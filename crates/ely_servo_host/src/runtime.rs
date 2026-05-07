use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use dpi::PhysicalSize;
use ely_domain::{ProfileId, TabId, WebViewId};
use servo::{
    EventLoopWaker, LoadStatus, RenderingContext, Servo, ServoBuilder, WebView, WebViewBuilder,
    WebViewDelegate,
};
use url::Url;

use crate::{
    NavigationRequest, PermissionDecision, PermissionRequest, ServoHost, ServoHostError,
    WebViewSnapshot, WebViewState,
};

static SERVO_RUNTIME_STARTED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServoSurfaceSize {
    width: u32,
    height: u32,
}

impl ServoSurfaceSize {
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self { width: width.max(1), height: height.max(1) }
    }

    fn physical(self) -> PhysicalSize<u32> {
        PhysicalSize { width: self.width, height: self.height }
    }
}

pub struct SoftwareServoHost {
    servo: Servo,
    rendering_context: Rc<dyn RenderingContext>,
    webviews: HashMap<WebViewId, HostWebView>,
    permissions: PermissionStore,
    wake_requested: Arc<AtomicBool>,
}

impl SoftwareServoHost {
    pub fn new(size: ServoSurfaceSize) -> Result<Self, ServoHostError> {
        if SERVO_RUNTIME_STARTED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(ServoHostError::RuntimeAlreadyStarted);
        }

        let host = Self::new_started(size);
        if host.is_err() {
            SERVO_RUNTIME_STARTED.store(false, Ordering::Release);
        }
        host
    }

    fn new_started(size: ServoSurfaceSize) -> Result<Self, ServoHostError> {
        let rendering_context = Rc::new(
            servo::SoftwareRenderingContext::new(size.physical())
                .map_err(|_| ServoHostError::RenderingContextUnavailable)?,
        );
        rendering_context.make_current().map_err(|_| ServoHostError::RenderingContextNotCurrent)?;

        let wake_requested = Arc::new(AtomicBool::new(false));
        let servo = ServoBuilder::default()
            .event_loop_waker(Box::new(ServoWakeFlag::new(wake_requested.clone())))
            .build();

        Ok(Self {
            servo,
            rendering_context,
            webviews: HashMap::new(),
            permissions: Rc::new(RefCell::new(HashMap::new())),
            wake_requested,
        })
    }
}

impl ServoHost for SoftwareServoHost {
    fn create_webview(
        &mut self,
        tab_id: TabId,
        profile_id: ProfileId,
    ) -> Result<WebViewId, ServoHostError> {
        let webview_id = WebViewId::new();
        let delegate = Rc::new(HostWebViewDelegate::new(
            tab_id.clone(),
            profile_id.clone(),
            self.permissions.clone(),
        ));
        let webview = WebViewBuilder::new(&self.servo, self.rendering_context.clone())
            .delegate(delegate.clone())
            .build();

        self.webviews.insert(
            webview_id.clone(),
            HostWebView { tab_id, profile_id, webview, delegate, requested_url: None },
        );

        Ok(webview_id)
    }

    fn navigate(&mut self, request: NavigationRequest) -> Result<(), ServoHostError> {
        let url = Url::parse(request.url.as_str()).map_err(|_| {
            ServoHostError::InvalidNavigationUrl { value: request.url.as_str().to_string() }
        })?;
        self.servo.spin_event_loop();
        let servo = self.servo.clone();
        let rendering_context = self.rendering_context.clone();
        let webview = self
            .webviews
            .get_mut(&request.webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: request.webview_id.clone() })?;

        let requested_url = url.to_string();
        let has_loaded_page =
            matches!(webview.current_url().as_deref(), Some(value) if value != "about:blank");
        let should_create_initial_document = webview.requested_url.is_none() && !has_loaded_page;

        webview.delegate.set_state(WebViewState::Loading);
        if should_create_initial_document {
            webview.webview = WebViewBuilder::new(&servo, rendering_context)
                .delegate(webview.delegate.clone())
                .url(url)
                .build();
        } else {
            webview.webview.load(url);
        }
        webview.requested_url = Some(requested_url);
        Ok(())
    }

    fn set_permission(
        &mut self,
        request: PermissionRequest,
        decision: PermissionDecision,
    ) -> Result<(), ServoHostError> {
        self.webviews
            .get(&request.webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: request.webview_id.clone() })?;

        self.permissions.borrow_mut().insert(
            PermissionKey::new(request.profile_id, request.tab_id, request.feature),
            decision,
        );
        Ok(())
    }

    fn state(&self, webview_id: &WebViewId) -> Result<WebViewState, ServoHostError> {
        Ok(self.webview(webview_id)?.state())
    }

    fn snapshot(&self, webview_id: &WebViewId) -> Result<WebViewSnapshot, ServoHostError> {
        self.webview(webview_id).map(|webview| webview.snapshot(webview_id))
    }

    fn tick(&mut self) -> bool {
        let requested = self.wake_requested.swap(false, Ordering::AcqRel);
        self.servo.spin_event_loop();
        requested
    }

    fn paint(&mut self, webview_id: &WebViewId) -> Result<(), ServoHostError> {
        let webview = self.webview(webview_id)?;
        webview.webview.paint();
        self.rendering_context.present();
        webview.delegate.mark_frame_presented();
        Ok(())
    }
}

impl SoftwareServoHost {
    fn webview(&self, webview_id: &WebViewId) -> Result<&HostWebView, ServoHostError> {
        self.webviews
            .get(webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: webview_id.clone() })
    }
}

struct HostWebView {
    tab_id: TabId,
    profile_id: ProfileId,
    webview: WebView,
    delegate: Rc<HostWebViewDelegate>,
    requested_url: Option<String>,
}

impl HostWebView {
    fn snapshot(&self, webview_id: &WebViewId) -> WebViewSnapshot {
        WebViewSnapshot::new(
            webview_id.clone(),
            self.tab_id.clone(),
            self.profile_id.clone(),
            self.state(),
            self.current_url(),
            self.current_title(),
            self.delegate.has_pending_frame(),
        )
    }

    fn state(&self) -> WebViewState {
        let state = self.delegate.state();
        if matches!(state, WebViewState::Crashed | WebViewState::Sleeping) {
            return state;
        }

        if let Some(requested_url) = &self.requested_url
            && self.current_url().as_deref() != Some(requested_url.as_str())
        {
            return WebViewState::Loading;
        }

        state
    }

    fn current_url(&self) -> Option<String> {
        self.webview.url().map(|url| url.to_string()).or_else(|| self.delegate.url())
    }

    fn current_title(&self) -> Option<String> {
        self.webview.page_title().or_else(|| self.delegate.title())
    }
}

struct HostWebViewDelegate {
    tab_id: TabId,
    profile_id: ProfileId,
    permissions: PermissionStore,
    state: RefCell<WebViewState>,
    url: RefCell<Option<String>>,
    title: RefCell<Option<String>>,
    has_pending_frame: Cell<bool>,
}

impl HostWebViewDelegate {
    fn new(tab_id: TabId, profile_id: ProfileId, permissions: PermissionStore) -> Self {
        Self {
            tab_id,
            profile_id,
            permissions,
            state: RefCell::new(WebViewState::Created),
            url: RefCell::new(None),
            title: RefCell::new(None),
            has_pending_frame: Cell::new(false),
        }
    }

    fn set_state(&self, state: WebViewState) {
        self.state.replace(state);
    }

    fn state(&self) -> WebViewState {
        self.state.borrow().clone()
    }

    fn url(&self) -> Option<String> {
        self.url.borrow().clone()
    }

    fn title(&self) -> Option<String> {
        self.title.borrow().clone()
    }

    fn has_pending_frame(&self) -> bool {
        self.has_pending_frame.get()
    }

    fn mark_frame_presented(&self) {
        self.has_pending_frame.set(false);
    }

    fn permission_decision(&self, feature: String) -> Option<PermissionDecision> {
        let key = PermissionKey::new(self.profile_id.clone(), self.tab_id.clone(), feature);
        let mut permissions = self.permissions.borrow_mut();
        match permissions.get(&key).cloned() {
            Some(PermissionDecision::AllowOnce) => permissions.remove(&key),
            decision => decision,
        }
    }
}

impl WebViewDelegate for HostWebViewDelegate {
    fn notify_url_changed(&self, _webview: WebView, url: Url) {
        self.url.replace(Some(url.to_string()));
    }

    fn notify_page_title_changed(&self, _webview: WebView, title: Option<String>) {
        self.title.replace(title);
    }

    fn notify_load_status_changed(&self, _webview: WebView, status: LoadStatus) {
        let state = match status {
            LoadStatus::Started | LoadStatus::HeadParsed => WebViewState::Loading,
            LoadStatus::Complete => WebViewState::Complete,
        };
        self.set_state(state);
    }

    fn notify_new_frame_ready(&self, _webview: WebView) {
        self.has_pending_frame.set(true);
    }

    fn notify_crashed(&self, _webview: WebView, _reason: String, _backtrace: Option<String>) {
        self.set_state(WebViewState::Crashed);
    }

    fn request_navigation(&self, _webview: WebView, navigation_request: servo::NavigationRequest) {
        navigation_request.allow();
    }

    fn request_permission(&self, _webview: WebView, permission_request: servo::PermissionRequest) {
        let feature = format!("{:?}", permission_request.feature());
        match self.permission_decision(feature) {
            Some(PermissionDecision::AllowOnce | PermissionDecision::AllowAlways) => {
                permission_request.allow();
            }
            Some(PermissionDecision::DenyAlways) | None => {
                permission_request.deny();
            }
        }
    }
}

type PermissionStore = Rc<RefCell<HashMap<PermissionKey, PermissionDecision>>>;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PermissionKey {
    profile_id: ProfileId,
    tab_id: TabId,
    feature: String,
}

impl PermissionKey {
    fn new(profile_id: ProfileId, tab_id: TabId, feature: String) -> Self {
        Self { profile_id, tab_id, feature }
    }
}

#[derive(Clone)]
struct ServoWakeFlag {
    requested: Arc<AtomicBool>,
}

impl ServoWakeFlag {
    fn new(requested: Arc<AtomicBool>) -> Self {
        Self { requested }
    }
}

impl EventLoopWaker for ServoWakeFlag {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }

    fn wake(&self) {
        self.requested.store(true, Ordering::Release);
    }
}

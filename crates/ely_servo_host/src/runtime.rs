use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use dpi::PhysicalSize;
use ely_domain::{ProfileId, TabId, WebViewId};
use servo::{
    DeviceIntPoint, DeviceIntRect, DeviceIntSize, DevicePoint, DeviceVector2D, Opts,
    RenderingContext, Scroll, Servo, ServoBuilder, WebViewBuilder, WebViewPoint, WebViewVector,
};
use url::Url;

use crate::{
    KeyboardTextRequest, MouseClickRequest, MouseDragRequest, NavigationRequest, PageZoomRequest,
    PermissionDecision, PermissionRequest, RenderedFrame, ResizeRequest, ScreenshotRequest,
    ScrollRequest, ServoHost, ServoHostError, TouchTapRequest, WebViewSnapshot, WebViewState,
    runtime_input::{send_keyboard_text, send_mouse_click, send_mouse_drag, send_touch_tap},
    runtime_permissions::{PermissionStore, set_permission_decision},
    runtime_waker::ServoWakeFlag,
    runtime_webview::{HostWebView, HostWebViewDelegate},
};

static SERVO_RUNTIME_STARTED: AtomicBool = AtomicBool::new(false);
const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(20);
const SCREENSHOT_POLL_INTERVAL: Duration = Duration::from_millis(2);

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
    last_rendered_frame: Option<RenderedFrame>,
}

impl SoftwareServoHost {
    pub fn new(size: ServoSurfaceSize) -> Result<Self, ServoHostError> {
        Self::new_with_config_dir(size, None)
    }

    pub fn new_with_config_dir(
        size: ServoSurfaceSize,
        config_dir: Option<PathBuf>,
    ) -> Result<Self, ServoHostError> {
        if SERVO_RUNTIME_STARTED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(ServoHostError::RuntimeAlreadyStarted);
        }

        let host = Self::new_started(size, config_dir);
        if host.is_err() {
            SERVO_RUNTIME_STARTED.store(false, Ordering::Release);
        }
        host
    }

    fn new_started(
        size: ServoSurfaceSize,
        config_dir: Option<PathBuf>,
    ) -> Result<Self, ServoHostError> {
        let rendering_context = Rc::new(
            servo::SoftwareRenderingContext::new(size.physical())
                .map_err(|_| ServoHostError::RenderingContextUnavailable)?,
        );
        rendering_context.make_current().map_err(|_| ServoHostError::RenderingContextNotCurrent)?;

        let wake_requested = Arc::new(AtomicBool::new(false));
        let mut builder = ServoBuilder::default()
            .event_loop_waker(Box::new(ServoWakeFlag::new(wake_requested.clone())));
        if let Some(config_dir) = config_dir {
            builder = builder.opts(Opts { config_dir: Some(config_dir), ..Opts::default() });
        }
        let servo = builder.build();

        Ok(Self {
            servo,
            rendering_context,
            webviews: HashMap::new(),
            permissions: Rc::new(RefCell::new(HashMap::new())),
            wake_requested,
            last_rendered_frame: None,
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
        let delegate =
            Rc::new(HostWebViewDelegate::new(profile_id.clone(), self.permissions.clone()));
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

    fn scroll(&mut self, request: ScrollRequest) -> Result<(), ServoHostError> {
        let webview = self
            .webviews
            .get(&request.webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: request.webview_id.clone() })?;

        if request.delta_x == 0 && request.delta_y == 0 {
            return Ok(());
        }

        webview.webview.notify_scroll_event(
            Scroll::Delta(WebViewVector::Device(DeviceVector2D::new(
                request.delta_x as f32,
                request.delta_y as f32,
            ))),
            WebViewPoint::Device(DevicePoint::zero()),
        );
        Ok(())
    }

    fn resize(&mut self, request: ResizeRequest) -> Result<(), ServoHostError> {
        let webview = self
            .webviews
            .get(&request.webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: request.webview_id.clone() })?;

        webview.webview.resize(PhysicalSize::new(request.width, request.height));
        Ok(())
    }

    fn set_page_zoom(&mut self, request: PageZoomRequest) -> Result<(), ServoHostError> {
        let webview = self
            .webviews
            .get(&request.webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: request.webview_id.clone() })?;

        webview.webview.set_page_zoom(request.zoom_factor);
        Ok(())
    }

    fn click(&mut self, request: MouseClickRequest) -> Result<(), ServoHostError> {
        let webview = self
            .webviews
            .get(&request.webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: request.webview_id.clone() })?;

        send_mouse_click(&webview.webview, request.x, request.y);
        Ok(())
    }

    fn drag(&mut self, request: MouseDragRequest) -> Result<(), ServoHostError> {
        let webview = self
            .webviews
            .get(&request.webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: request.webview_id.clone() })?;

        send_mouse_drag(
            &webview.webview,
            request.from_x,
            request.from_y,
            request.to_x,
            request.to_y,
        );
        Ok(())
    }

    fn touch_tap(&mut self, request: TouchTapRequest) -> Result<(), ServoHostError> {
        let webview = self
            .webviews
            .get(&request.webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: request.webview_id.clone() })?;

        send_touch_tap(&webview.webview, request.x, request.y);
        Ok(())
    }

    fn type_text(&mut self, request: KeyboardTextRequest) -> Result<(), ServoHostError> {
        let webview = self
            .webviews
            .get(&request.webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: request.webview_id.clone() })?;

        send_keyboard_text(&webview.webview, &request.text);
        Ok(())
    }

    fn capture_screenshot(
        &mut self,
        request: ScreenshotRequest,
    ) -> Result<RenderedFrame, ServoHostError> {
        let webview = self.webview(&request.webview_id)?.webview.clone();
        let captured_image = Rc::new(RefCell::new(None));
        let callback_image = captured_image.clone();
        webview.take_screenshot(None, move |result| {
            callback_image.replace(Some(result));
        });

        let started_at = Instant::now();
        while captured_image.borrow().is_none() {
            if started_at.elapsed() >= SCREENSHOT_TIMEOUT {
                return Err(ServoHostError::ScreenshotTimedOut { id: request.webview_id.clone() });
            }

            self.tick();
            if self.snapshot(&request.webview_id)?.has_pending_frame() {
                self.paint(&request.webview_id)?;
            }
            thread::sleep(SCREENSHOT_POLL_INTERVAL);
        }

        let Some(result) = captured_image.borrow_mut().take() else {
            return Err(ServoHostError::RenderedFrameUnavailable);
        };
        let image = result.map_err(|error| ServoHostError::ScreenshotUnavailable {
            reason: format!("{error:?}"),
        })?;
        let frame = RenderedFrame::from_rgba_bytes(image.width(), image.height(), image.into_raw());
        self.last_rendered_frame = Some(frame.clone());
        Ok(frame)
    }

    fn set_permission(
        &mut self,
        request: PermissionRequest,
        decision: PermissionDecision,
    ) -> Result<(), ServoHostError> {
        let webview = self
            .webviews
            .get(&request.webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: request.webview_id.clone() })?;
        if webview.profile_id != request.profile_id {
            return Err(ServoHostError::PermissionProfileMismatch {
                webview_id: request.webview_id,
                expected: webview.profile_id.clone(),
                actual: request.profile_id,
            });
        }

        set_permission_decision(&self.permissions, request, decision);
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
        self.rendering_context
            .make_current()
            .map_err(|_| ServoHostError::RenderingContextNotCurrent)?;
        self.rendering_context.prepare_for_rendering();
        {
            let webview = self.webview(webview_id)?;
            webview.webview.paint();
        }
        let rendered_frame = self.read_rendered_frame()?;
        self.rendering_context.present();
        self.webview(webview_id)?.delegate.mark_frame_presented();
        self.last_rendered_frame = Some(rendered_frame);
        Ok(())
    }

    fn last_rendered_frame(&self) -> Result<RenderedFrame, ServoHostError> {
        self.last_rendered_frame.clone().ok_or(ServoHostError::RenderedFrameUnavailable)
    }
}

impl SoftwareServoHost {
    fn webview(&self, webview_id: &WebViewId) -> Result<&HostWebView, ServoHostError> {
        self.webviews
            .get(webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: webview_id.clone() })
    }

    fn read_rendered_frame(&self) -> Result<RenderedFrame, ServoHostError> {
        let size = self.rendering_context.size();
        let width =
            i32::try_from(size.width).map_err(|_| ServoHostError::RenderedFrameUnavailable)?;
        let height =
            i32::try_from(size.height).map_err(|_| ServoHostError::RenderedFrameUnavailable)?;
        let frame_rect = DeviceIntRect::from_origin_and_size(
            DeviceIntPoint::new(0, 0),
            DeviceIntSize::new(width, height),
        );
        let image = self
            .rendering_context
            .read_to_image(frame_rect)
            .ok_or(ServoHostError::RenderedFrameUnavailable)?;

        Ok(RenderedFrame::from_rgba_bytes(size.width, size.height, image.into_raw()))
    }
}

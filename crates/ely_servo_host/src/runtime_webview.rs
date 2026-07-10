use std::{cell::Cell, cell::RefCell, rc::Rc};

use ely_domain::{ProfileId, TabId, WebViewId};
use servo::{LoadStatus, RenderingContext, WebView, WebViewDelegate};
use url::Url;

use crate::{
    PermissionDecision, WebViewSnapshot, WebViewSnapshotPending, WebViewState,
    runtime_permissions::{PermissionStore, permission_decision_for_webview},
};

pub(super) struct HostWebView {
    pub(super) tab_id: TabId,
    pub(super) profile_id: ProfileId,
    pub(super) rendering_context: Rc<dyn RenderingContext>,
    #[cfg(feature = "hardware-render")]
    pub(super) hardware_context: Option<Rc<crate::HardwareOffscreenContext>>,
    pub(super) webview: WebView,
    pub(super) delegate: Rc<HostWebViewDelegate>,
    pub(super) requested_url: Option<String>,
}

impl HostWebView {
    pub(super) fn snapshot(&self, webview_id: &WebViewId) -> WebViewSnapshot {
        WebViewSnapshot::new(
            webview_id.clone(),
            self.tab_id.clone(),
            self.profile_id.clone(),
            self.state(),
            self.current_url(),
            self.current_title(),
            WebViewSnapshotPending::new(
                self.delegate.has_pending_frame(),
                self.delegate.has_pending_metadata(),
            ),
        )
    }

    pub(super) fn state(&self) -> WebViewState {
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

    pub(super) fn current_url(&self) -> Option<String> {
        self.webview.url().map(|url| url.to_string()).or_else(|| self.delegate.url())
    }

    fn current_title(&self) -> Option<String> {
        self.webview.page_title().or_else(|| self.delegate.title())
    }
}

pub(super) struct HostWebViewDelegate {
    profile_id: ProfileId,
    permissions: PermissionStore,
    state: RefCell<WebViewState>,
    url: RefCell<Option<String>>,
    title: RefCell<Option<String>>,
    has_pending_frame: Cell<bool>,
    has_pending_metadata: Cell<bool>,
}

impl HostWebViewDelegate {
    pub(super) fn new(profile_id: ProfileId, permissions: PermissionStore) -> Self {
        Self {
            profile_id,
            permissions,
            state: RefCell::new(WebViewState::Created),
            url: RefCell::new(None),
            title: RefCell::new(None),
            has_pending_frame: Cell::new(false),
            has_pending_metadata: Cell::new(false),
        }
    }

    pub(super) fn set_state(&self, state: WebViewState) {
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

    fn record_url_change(&self, url: String) {
        self.url.replace(Some(url));
        self.has_pending_metadata.set(true);
    }

    fn record_title_change(&self, title: Option<String>) {
        self.title.replace(title);
        self.has_pending_metadata.set(true);
    }

    fn record_load_status(&self, status: LoadStatus) {
        let state = match status {
            LoadStatus::Started | LoadStatus::HeadParsed => WebViewState::Loading,
            LoadStatus::Complete => WebViewState::Complete,
        };
        self.set_state(state);
        self.has_pending_metadata.set(true);
    }

    pub(super) fn has_pending_frame(&self) -> bool {
        self.has_pending_frame.get()
    }

    pub(super) fn has_pending_metadata(&self) -> bool {
        self.has_pending_metadata.get()
    }

    pub(super) fn mark_frame_presented(&self) {
        self.has_pending_frame.set(false);
    }

    pub(super) fn mark_frame_ready(&self) {
        self.has_pending_frame.set(true);
    }

    pub(super) fn mark_metadata_observed(&self) {
        self.has_pending_metadata.set(false);
    }
}

impl WebViewDelegate for HostWebViewDelegate {
    fn notify_url_changed(&self, _webview: WebView, url: Url) {
        self.record_url_change(url.to_string());
    }

    fn notify_page_title_changed(&self, _webview: WebView, title: Option<String>) {
        self.record_title_change(title);
    }

    fn notify_load_status_changed(&self, _webview: WebView, status: LoadStatus) {
        self.record_load_status(status);
    }

    fn notify_new_frame_ready(&self, _webview: WebView) {
        self.mark_frame_ready();
    }

    fn notify_crashed(&self, _webview: WebView, _reason: String, _backtrace: Option<String>) {
        self.set_state(WebViewState::Crashed);
    }

    fn request_navigation(&self, _webview: WebView, navigation_request: servo::NavigationRequest) {
        navigation_request.allow();
    }

    fn request_permission(&self, webview: WebView, permission_request: servo::PermissionRequest) {
        match permission_decision_for_webview(
            &self.permissions,
            &self.profile_id,
            &webview,
            self.url(),
            permission_request.feature(),
        ) {
            Some(PermissionDecision::AllowOnce | PermissionDecision::AllowAlways) => {
                permission_request.allow();
            }
            Some(PermissionDecision::DenyAlways) | None => {
                permission_request.deny();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    use ely_domain::ProfileId;

    use super::HostWebViewDelegate;

    #[test]
    fn metadata_changes_are_separate_from_pending_frame() {
        let delegate =
            HostWebViewDelegate::new(ProfileId::new(), Rc::new(RefCell::new(HashMap::new())));

        assert!(!delegate.has_pending_frame());
        assert!(!delegate.has_pending_metadata());

        delegate.record_title_change(Some("Example Domain".to_string()));
        assert_eq!(delegate.title().as_deref(), Some("Example Domain"));
        assert!(!delegate.has_pending_frame());
        assert!(delegate.has_pending_metadata());

        delegate.mark_metadata_observed();
        assert!(!delegate.has_pending_frame());
        assert!(!delegate.has_pending_metadata());

        delegate.record_url_change("https://example.com/".to_string());
        assert_eq!(delegate.url().as_deref(), Some("https://example.com/"));
        assert!(!delegate.has_pending_frame());
        assert!(delegate.has_pending_metadata());
    }
}

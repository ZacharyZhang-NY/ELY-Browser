use ely_domain::{ProfileId, TabId, UrlText, WebViewId};

use crate::ServoHostError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WebViewState {
    Created,
    Loading,
    Complete,
    Sleeping,
    Crashed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebViewSnapshot {
    webview_id: WebViewId,
    tab_id: TabId,
    profile_id: ProfileId,
    state: WebViewState,
    url: Option<String>,
    title: Option<String>,
    has_pending_frame: bool,
}

impl WebViewSnapshot {
    #[must_use]
    pub fn new(
        webview_id: WebViewId,
        tab_id: TabId,
        profile_id: ProfileId,
        state: WebViewState,
        url: Option<String>,
        title: Option<String>,
        has_pending_frame: bool,
    ) -> Self {
        Self { webview_id, tab_id, profile_id, state, url, title, has_pending_frame }
    }

    #[must_use]
    pub fn webview_id(&self) -> &WebViewId {
        &self.webview_id
    }

    #[must_use]
    pub fn tab_id(&self) -> &TabId {
        &self.tab_id
    }

    #[must_use]
    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    #[must_use]
    pub fn state(&self) -> &WebViewState {
        &self.state
    }

    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub fn has_pending_frame(&self) -> bool {
        self.has_pending_frame
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationRequest {
    pub webview_id: WebViewId,
    pub tab_id: TabId,
    pub url: UrlText,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionRequest {
    pub webview_id: WebViewId,
    pub tab_id: TabId,
    pub profile_id: ProfileId,
    pub feature: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PermissionDecision {
    AllowOnce,
    AllowAlways,
    DenyAlways,
}

pub trait ServoHost {
    fn create_webview(
        &mut self,
        tab_id: TabId,
        profile_id: ProfileId,
    ) -> Result<WebViewId, ServoHostError>;

    fn navigate(&mut self, request: NavigationRequest) -> Result<(), ServoHostError>;

    fn set_permission(
        &mut self,
        request: PermissionRequest,
        decision: PermissionDecision,
    ) -> Result<(), ServoHostError>;

    fn state(&self, webview_id: &WebViewId) -> Result<WebViewState, ServoHostError>;

    fn snapshot(&self, webview_id: &WebViewId) -> Result<WebViewSnapshot, ServoHostError>;

    fn tick(&mut self) -> bool;

    fn paint(&mut self, webview_id: &WebViewId) -> Result<(), ServoHostError>;
}

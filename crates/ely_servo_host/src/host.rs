use ely_domain::{ProfileId, TabId, UrlText, WebViewId};

use crate::ServoHostError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WebViewState {
    Created,
    Attached,
    Sleeping,
    Crashed,
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
}

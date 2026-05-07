use ely_domain::WebViewId;
use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ServoHostError {
    #[error("webview not found: {id}")]
    WebViewNotFound { id: WebViewId },

    #[error("permission request missing profile context")]
    MissingProfileContext,
}

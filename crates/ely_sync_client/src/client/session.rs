use serde::Deserialize;

use super::{SyncApiClient, read_json_from_response};
use crate::error::SyncClientError;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionLogoutDocument {
    version: u32,
    signed_out: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthErrorDocument {
    error: String,
}

impl SyncApiClient {
    pub fn sign_out(&self) -> Result<(), SyncClientError> {
        let endpoint = self.endpoint("/api/session/logout");
        let response = self
            .agent
            .post(&endpoint)
            .set("Authorization", &format!("Bearer {}", self.bearer.as_str()))
            .call();
        match response {
            Ok(response) => {
                let document =
                    read_json_from_response::<SessionLogoutDocument>(&endpoint, response)?;
                if document.version != 1 || !document.signed_out {
                    return Err(SyncClientError::SessionLogoutInvalid);
                }
                Ok(())
            }
            Err(ureq::Error::Status(status, response)) => {
                let body = response.into_string().unwrap_or_default();
                if response_ends_session(status, &body) {
                    return Ok(());
                }
                Err(SyncClientError::HttpStatus { endpoint, status, body })
            }
            Err(source) => Err(SyncClientError::Http { endpoint, source: Box::new(source) }),
        }
    }
}

pub(super) fn response_ends_session(status: u16, body: &str) -> bool {
    status == 401
        && serde_json::from_str::<AuthErrorDocument>(body).is_ok_and(|document| {
            matches!(document.error.as_str(), "session_not_found" | "session_expired")
        })
}

#[cfg(test)]
mod tests {
    use super::response_ends_session;

    #[test]
    fn terminal_session_parser_is_exact() {
        for body in [r#"{"error":"session_not_found"}"#, r#"{"error":"session_expired"}"#] {
            assert!(response_ends_session(401, body));
        }
        for (status, body) in [
            (401, r#"{"error":"authorization_missing"}"#),
            (401, r#"{"error":"authorization_invalid"}"#),
            (401, r#"{"error":"unknown"}"#),
            (401, r#"{"error":"session_not_found","extra":true}"#),
            (401, "{"),
            (403, r#"{"error":"session_not_found"}"#),
            (500, r#"{"error":"session_expired"}"#),
        ] {
            assert!(!response_ends_session(status, body));
        }
    }
}

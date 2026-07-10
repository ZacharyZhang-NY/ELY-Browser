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
                if status == 401 && logout_is_already_complete(&body) {
                    return Ok(());
                }
                Err(SyncClientError::HttpStatus { endpoint, status, body })
            }
            Err(source) => Err(SyncClientError::Http { endpoint, source: Box::new(source) }),
        }
    }
}

fn logout_is_already_complete(body: &str) -> bool {
    serde_json::from_str::<AuthErrorDocument>(body).is_ok_and(|document| {
        matches!(document.error.as_str(), "session_not_found" | "session_expired")
    })
}

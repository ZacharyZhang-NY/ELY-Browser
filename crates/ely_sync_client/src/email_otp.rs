//! Email + OTP sign-in flow against the Better Auth `email-otp` plugin
//! exposed by the Cloudflare worker.
//!
//! Two endpoints are involved, both unauthenticated and rate-limited
//! by the worker:
//!
//! - `POST /api/auth/email-otp/send-verification-otp` triggers the
//!   email send. Body: `{ email, type: "sign-in" }`. Returns 200 on
//!   success.
//! - `POST /api/auth/sign-in/email-otp` exchanges the typed code for
//!   a Better Auth session. Body: `{ email, otp }`. Returns 200 with
//!   `{ token, user, … }` and a `Set-Cookie: better-auth.session_token`
//!   header. We accept either delivery channel — the JSON body's
//!   `token` field wins, with the cookie as the canonical fallback.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use ureq::{Agent, AgentBuilder};
use zeroize::Zeroizing;

use crate::{auth::BearerToken, client::ApiClientConfig, error::SyncClientError};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const USER_AGENT: &str = concat!("ELY Browser/", env!("CARGO_PKG_VERSION"));

#[derive(Serialize)]
struct SendOtpRequest<'a> {
    email: &'a str,
    #[serde(rename = "type")]
    purpose: &'static str,
}

#[derive(Serialize)]
struct VerifyOtpRequest<'a> {
    email: &'a str,
    otp: &'a str,
}

#[derive(Deserialize)]
struct VerifyOtpResponse {
    #[serde(default)]
    token: Option<String>,
}

/// Ask the worker to email a one-time code to `email`. The actual
/// delivery happens through the Cloudflare `SEND_EMAIL` binding
/// (sender `browser@elydora.com`). Returns when the worker has
/// accepted the request — it does not wait for the recipient's MTA.
pub fn send_email_otp(config: &ApiClientConfig, email: &str) -> Result<(), SyncClientError> {
    let endpoint = format!(
        "{}/api/auth/email-otp/send-verification-otp",
        config.base_url().trim_end_matches('/')
    );
    let agent = build_agent();
    let response = agent
        .post(&endpoint)
        .set("Content-Type", "application/json")
        .send_json(serde_json::to_value(SendOtpRequest { email, purpose: "sign-in" }).map_err(
            |error| SyncClientError::Json { endpoint: endpoint.clone(), source: error },
        )?);
    match response {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(status, raw)) => {
            let body = raw.into_string().unwrap_or_default();
            Err(SyncClientError::HttpStatus { endpoint, status, body })
        }
        Err(other) => Err(SyncClientError::Http { endpoint, source: Box::new(other) }),
    }
}

/// Verify the user's OTP and return the freshly-issued Better Auth
/// session token wrapped as a [`BearerToken`].
pub fn verify_email_otp(
    config: &ApiClientConfig,
    email: &str,
    otp: &str,
) -> Result<BearerToken, SyncClientError> {
    let endpoint =
        format!("{}/api/auth/sign-in/email-otp", config.base_url().trim_end_matches('/'));
    let agent = build_agent();
    let response = agent
        .post(&endpoint)
        .set("Content-Type", "application/json")
        .send_json(serde_json::to_value(VerifyOtpRequest { email, otp }).map_err(|error| {
            SyncClientError::Json { endpoint: endpoint.clone(), source: error }
        })?);
    match response {
        Ok(ok) => {
            let cookie_token = better_auth_cookie_token(ok.header("set-cookie"));
            let body =
                Zeroizing::new(ok.into_string().map_err(|error| SyncClientError::HttpStatus {
                    endpoint: endpoint.clone(),
                    status: 200,
                    body: error.to_string(),
                })?);
            let json = serde_json::from_str::<VerifyOtpResponse>(&body).map_err(|error| {
                SyncClientError::Json { endpoint: endpoint.clone(), source: error }
            })?;
            let token = json.token.map(Zeroizing::new).or(cookie_token).ok_or_else(|| {
                SyncClientError::TokenStorage(
                    "sign-in response did not include a session token".to_string(),
                )
            })?;
            BearerToken::new(token.as_str())
        }
        Err(ureq::Error::Status(status, raw)) => {
            let body = raw.into_string().unwrap_or_default();
            Err(SyncClientError::HttpStatus { endpoint, status, body })
        }
        Err(other) => Err(SyncClientError::Http { endpoint, source: Box::new(other) }),
    }
}

fn build_agent() -> Agent {
    AgentBuilder::new().timeout(REQUEST_TIMEOUT).user_agent(USER_AGENT).build()
}

/// Better Auth ships the session through a `Set-Cookie:
/// better-auth.session_token=<token>; …` header. Strip the cookie's
/// attributes and return just the value. Multi-cookie responses are
/// concatenated by `ureq` into a single header line per spec.
fn better_auth_cookie_token(set_cookie: Option<&str>) -> Option<Zeroizing<String>> {
    let header = set_cookie?;
    for cookie in header.split(',') {
        let trimmed = cookie.trim();
        if let Some(rest) = trimmed.strip_prefix("better-auth.session_token=") {
            let token = rest.split(';').next()?.trim();
            if !token.is_empty() {
                return Some(Zeroizing::new(token.to_string()));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::better_auth_cookie_token;

    #[test]
    fn picks_session_token_out_of_set_cookie_header() {
        let header =
            "better-auth.session_token=abc.def.ghi; Path=/; HttpOnly; Secure; SameSite=Lax";
        assert_eq!(
            better_auth_cookie_token(Some(header)).as_deref().map(String::as_str),
            Some("abc.def.ghi"),
        );
    }

    #[test]
    fn ignores_unrelated_cookies() {
        let header = "csrf=value; Path=/, other=cookie";
        assert_eq!(better_auth_cookie_token(Some(header)), None);
    }

    #[test]
    fn returns_none_when_header_is_absent() {
        assert_eq!(better_auth_cookie_token(None), None);
    }
}

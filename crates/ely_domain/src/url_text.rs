use std::fmt;

use url::Url;

use crate::DomainError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UrlText {
    value: String,
}

impl UrlText {
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField { field: "url" });
        }

        Url::parse(trimmed).map_err(|_| DomainError::InvalidUrl { value: trimmed.to_string() })?;

        Ok(Self { value: trimmed.to_string() })
    }

    pub fn from_address_text(value: &str) -> Result<Self, DomainError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField { field: "url" });
        }

        if Url::parse(trimmed).is_ok() {
            return Self::parse(trimmed);
        }

        if trimmed.contains('.') && !trimmed.contains(char::is_whitespace) {
            return Self::parse(format!("https://{trimmed}"));
        }

        Err(DomainError::InvalidUrl { value: trimmed.to_string() })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub fn display_host(&self) -> String {
        Url::parse(&self.value)
            .ok()
            .and_then(|url| url.host_str().map(str::to_string))
            .unwrap_or_else(|| self.value.clone())
    }

    #[must_use]
    pub fn host(&self) -> Option<String> {
        Url::parse(&self.value)
            .ok()
            .and_then(|url| url.host_str().map(|host| host.to_ascii_lowercase()))
    }

    #[must_use]
    pub fn display_url(&self) -> String {
        let Ok(url) = Url::parse(&self.value) else {
            return self.value.clone();
        };

        if url.scheme() == "ely" {
            return self.value.clone();
        }

        url.host_str().map(str::to_string).unwrap_or_else(|| self.value.clone())
    }

    /// Resolve a favicon URL for an HTTP(S) page. Returns `None` for
    /// non-web schemes (`ely://`, `file://`, …) and for URLs missing
    /// an authority — those tabs render the URL-derived glyph instead.
    ///
    /// We deliberately do NOT point the renderer at the site's own
    /// `/favicon.ico` because a) many sites only ship that icon as
    /// a multi-image `image/x-icon` blob the renderer's PNG/WebP
    /// decoder can't read, and b) the URL frequently 404s or
    /// redirects across origins (notion.com → notion.so etc.) which
    /// the GPUI image fetcher surfaces as a noisy `ERROR` log on
    /// every tab. Instead we route through Google's `s2/favicons`
    /// endpoint: it normalises the response to PNG, resolves
    /// redirects on Google's side, and serves a `_/` globe glyph
    /// when the target site has no favicon at all. Same URL shape
    /// every browser dev-tools panel already shows for "favicon".
    #[must_use]
    pub fn favicon_url(&self) -> Option<String> {
        let url = Url::parse(&self.value).ok()?;
        if !matches!(url.scheme(), "http" | "https") {
            return None;
        }
        let host = url.host_str()?;
        Some(format!("https://www.google.com/s2/favicons?domain={host}&sz=64"))
    }
}

impl fmt::Display for UrlText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}

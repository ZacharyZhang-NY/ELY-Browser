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
}

impl fmt::Display for UrlText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}

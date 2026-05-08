use url::Url;

use crate::{DomainError, UrlText};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SearchEngine {
    #[default]
    DuckDuckGo,
    Google,
    Brave,
}

impl SearchEngine {
    pub const ALL: &[Self] = &[Self::DuckDuckGo, Self::Google, Self::Brave];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::DuckDuckGo => "DuckDuckGo",
            Self::Google => "Google",
            Self::Brave => "Brave Search",
        }
    }

    #[must_use]
    pub fn host(self) -> &'static str {
        match self {
            Self::DuckDuckGo => "duckduckgo.com",
            Self::Google => "www.google.com",
            Self::Brave => "search.brave.com",
        }
    }

    pub fn search_url(self, query: &str) -> Result<UrlText, DomainError> {
        let mut url = Url::parse(self.base_url())
            .map_err(|_| DomainError::InvalidUrl { value: self.base_url().to_string() })?;
        url.query_pairs_mut().append_pair(self.query_parameter(), query);
        UrlText::parse(url.to_string())
    }

    fn base_url(self) -> &'static str {
        match self {
            Self::DuckDuckGo => "https://duckduckgo.com/",
            Self::Google => "https://www.google.com/search",
            Self::Brave => "https://search.brave.com/search",
        }
    }

    fn query_parameter(self) -> &'static str {
        "q"
    }
}

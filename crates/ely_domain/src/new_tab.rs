use crate::{DomainError, UrlText};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NewTabDestination {
    #[default]
    ElyNewTab,
    Bookmarks,
    ReadingList,
}

impl NewTabDestination {
    pub const ALL: &[Self] = &[Self::ElyNewTab, Self::Bookmarks, Self::ReadingList];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::ElyNewTab => "ELY New Tab",
            Self::Bookmarks => "Bookmarks",
            Self::ReadingList => "Reading List",
        }
    }

    #[must_use]
    pub fn detail(self) -> &'static str {
        match self {
            Self::ElyNewTab => "Open the quiet browser start surface.",
            Self::Bookmarks => "Open saved pages first.",
            Self::ReadingList => "Open the saved reading queue first.",
        }
    }

    #[must_use]
    pub fn route(self) -> &'static str {
        match self {
            Self::ElyNewTab => "ely://new-tab",
            Self::Bookmarks => "ely://bookmarks",
            Self::ReadingList => "ely://reading-list",
        }
    }

    pub fn url(self) -> Result<UrlText, DomainError> {
        UrlText::parse(self.route())
    }
}

use std::time::SystemTime;

use crate::BrowserTab;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveSource {
    ManualClose,
    AutoArchive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchivedTab {
    tab: BrowserTab,
    archived_at: SystemTime,
    source: ArchiveSource,
}

impl ArchivedTab {
    #[must_use]
    pub fn new(tab: BrowserTab, source: ArchiveSource) -> Self {
        Self { tab, archived_at: SystemTime::now(), source }
    }

    #[must_use]
    pub fn tab(&self) -> &BrowserTab {
        &self.tab
    }

    #[must_use]
    pub fn archived_at(&self) -> SystemTime {
        self.archived_at
    }

    #[must_use]
    pub fn source(&self) -> &ArchiveSource {
        &self.source
    }

    #[must_use]
    pub fn into_tab(self) -> BrowserTab {
        self.tab
    }
}

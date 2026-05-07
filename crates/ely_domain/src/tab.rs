use crate::{ProfileId, SpaceId, SplitId, TabId, UrlText};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TabState {
    Loading,
    Ready,
    Crashed,
    Discarded,
    Archived,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TabFlags {
    pub pinned: bool,
    pub favorite: bool,
    pub muted: bool,
    pub unread: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserTab {
    id: TabId,
    space_id: SpaceId,
    profile_id: ProfileId,
    title: String,
    url: UrlText,
    state: TabState,
    flags: TabFlags,
    split_id: Option<SplitId>,
}

impl BrowserTab {
    #[must_use]
    pub fn new(
        id: TabId,
        space_id: SpaceId,
        profile_id: ProfileId,
        title: impl Into<String>,
        url: UrlText,
    ) -> Self {
        Self {
            id,
            space_id,
            profile_id,
            title: title.into(),
            url,
            state: TabState::Ready,
            flags: TabFlags::default(),
            split_id: None,
        }
    }

    #[must_use]
    pub fn id(&self) -> &TabId {
        &self.id
    }

    #[must_use]
    pub fn space_id(&self) -> &SpaceId {
        &self.space_id
    }

    #[must_use]
    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn url(&self) -> &UrlText {
        &self.url
    }

    #[must_use]
    pub fn display_url(&self) -> String {
        self.url.display_url()
    }

    #[must_use]
    pub fn state(&self) -> &TabState {
        &self.state
    }

    #[must_use]
    pub fn flags(&self) -> &TabFlags {
        &self.flags
    }

    pub fn set_favorite(&mut self, favorite: bool) {
        self.flags.favorite = favorite;
    }

    pub fn set_pinned(&mut self, pinned: bool) {
        self.flags.pinned = pinned;
    }

    #[must_use]
    pub fn split_id(&self) -> Option<&SplitId> {
        self.split_id.as_ref()
    }
}

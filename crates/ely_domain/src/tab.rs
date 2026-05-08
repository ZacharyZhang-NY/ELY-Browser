use std::time::SystemTime;

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
    parent_tab_id: Option<TabId>,
    state: TabState,
    flags: TabFlags,
    split_id: Option<SplitId>,
    created_at: SystemTime,
    last_active_at: SystemTime,
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
        let created_at = SystemTime::now();
        Self {
            id,
            space_id,
            profile_id,
            title: title.into(),
            url,
            parent_tab_id: None,
            state: TabState::Ready,
            flags: TabFlags::default(),
            split_id: None,
            created_at,
            last_active_at: created_at,
        }
    }

    #[must_use]
    pub fn with_parent_tab_id(mut self, parent_tab_id: TabId) -> Self {
        self.parent_tab_id = Some(parent_tab_id);
        self
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
    pub fn parent_tab_id(&self) -> Option<&TabId> {
        self.parent_tab_id.as_ref()
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
    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    #[must_use]
    pub fn last_active_at(&self) -> SystemTime {
        self.last_active_at
    }

    pub fn record_activity(&mut self, active_at: SystemTime) {
        self.last_active_at = active_at;
    }

    pub fn mark_archived(&mut self) {
        self.state = TabState::Archived;
    }

    pub fn mark_ready(&mut self) {
        self.state = TabState::Ready;
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

    pub fn move_to_space(&mut self, space_id: SpaceId) {
        self.space_id = space_id;
    }

    #[must_use]
    pub fn split_id(&self) -> Option<&SplitId> {
        self.split_id.as_ref()
    }

    pub fn set_split_id(&mut self, split_id: SplitId) {
        self.split_id = Some(split_id);
    }

    pub fn clear_split_id(&mut self) {
        self.split_id = None;
    }
}

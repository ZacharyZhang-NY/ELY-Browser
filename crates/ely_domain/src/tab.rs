use std::time::SystemTime;

use crate::{DomainError, ProfileId, SpaceId, SplitId, TabGroupId, TabId, UrlText};

pub const DEFAULT_ZOOM_PERCENT: u16 = 100;
pub const MIN_ZOOM_PERCENT: u16 = 25;
pub const MAX_ZOOM_PERCENT: u16 = 500;
pub const ZOOM_PERCENT_STEP: u16 = 10;

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
    favicon_key: Option<String>,
    parent_tab_id: Option<TabId>,
    state: TabState,
    flags: TabFlags,
    group_id: Option<TabGroupId>,
    split_id: Option<SplitId>,
    sort_key: u64,
    sync_enabled: bool,
    zoom_percent: u16,
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
            favicon_key: None,
            parent_tab_id: None,
            state: TabState::Ready,
            flags: TabFlags::default(),
            group_id: None,
            split_id: None,
            sort_key: 0,
            sync_enabled: true,
            zoom_percent: DEFAULT_ZOOM_PERCENT,
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
    pub fn with_sort_key(mut self, sort_key: u64) -> Self {
        self.sort_key = sort_key;
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
    pub fn unread_count(&self) -> u32 {
        parse_title_unread_count(&self.title)
    }

    #[must_use]
    pub fn url(&self) -> &UrlText {
        &self.url
    }

    #[must_use]
    pub fn favicon_key(&self) -> Option<&str> {
        self.favicon_key.as_deref()
    }

    pub fn set_favicon_key(&mut self, favicon_key: impl Into<String>) -> Result<(), DomainError> {
        let favicon_key = favicon_key.into();
        let favicon_key = favicon_key.trim();
        if favicon_key.is_empty() {
            return Err(DomainError::EmptyField { field: "favicon_key" });
        }

        self.favicon_key = Some(favicon_key.to_string());
        Ok(())
    }

    pub fn clear_favicon_key(&mut self) {
        self.favicon_key = None;
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

    pub fn mark_crashed(&mut self) {
        self.state = TabState::Crashed;
    }

    pub fn mark_discarded(&mut self) {
        self.state = TabState::Discarded;
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
    pub fn group_id(&self) -> Option<&TabGroupId> {
        self.group_id.as_ref()
    }

    pub fn set_group_id(&mut self, group_id: TabGroupId) {
        self.group_id = Some(group_id);
    }

    pub fn clear_group_id(&mut self) {
        self.group_id = None;
    }

    #[must_use]
    pub fn split_id(&self) -> Option<&SplitId> {
        self.split_id.as_ref()
    }

    #[must_use]
    pub fn sync_enabled(&self) -> bool {
        self.sync_enabled
    }

    #[must_use]
    pub fn zoom_percent(&self) -> u16 {
        self.zoom_percent
    }

    #[must_use]
    pub fn zoom_factor(&self) -> f32 {
        f32::from(self.zoom_percent) / f32::from(DEFAULT_ZOOM_PERCENT)
    }

    pub fn set_zoom_percent(&mut self, zoom_percent: u16) -> Result<(), DomainError> {
        self.zoom_percent = validate_zoom_percent(zoom_percent)?;
        Ok(())
    }

    pub fn zoom_in(&mut self) {
        self.zoom_percent =
            self.zoom_percent.saturating_add(ZOOM_PERCENT_STEP).min(MAX_ZOOM_PERCENT);
    }

    pub fn zoom_out(&mut self) {
        self.zoom_percent =
            self.zoom_percent.saturating_sub(ZOOM_PERCENT_STEP).max(MIN_ZOOM_PERCENT);
    }

    pub fn reset_zoom(&mut self) {
        self.zoom_percent = DEFAULT_ZOOM_PERCENT;
    }

    pub fn set_split_id(&mut self, split_id: SplitId) {
        self.split_id = Some(split_id);
    }

    pub fn clear_split_id(&mut self) {
        self.split_id = None;
    }

    #[must_use]
    pub fn sort_key(&self) -> u64 {
        self.sort_key
    }

    pub fn set_sort_key(&mut self, sort_key: u64) {
        self.sort_key = sort_key;
    }

    pub fn set_sync_enabled(&mut self, sync_enabled: bool) {
        self.sync_enabled = sync_enabled;
    }
}

pub fn validate_zoom_percent(value: u16) -> Result<u16, DomainError> {
    if (MIN_ZOOM_PERCENT..=MAX_ZOOM_PERCENT).contains(&value) {
        return Ok(value);
    }

    Err(DomainError::InvalidZoomPercent { value, min: MIN_ZOOM_PERCENT, max: MAX_ZOOM_PERCENT })
}

#[must_use]
pub fn parse_title_unread_count(title: &str) -> u32 {
    let trimmed = title.trim_start();
    let Some(rest) = trimmed.strip_prefix('(') else {
        return 0;
    };
    let Some(close) = rest.find(')') else {
        return 0;
    };
    rest[..close].parse::<u32>().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::parse_title_unread_count;

    #[test]
    fn parses_leading_parenthesised_count() {
        assert_eq!(parse_title_unread_count("(12) Slack"), 12);
        assert_eq!(parse_title_unread_count("(3) Inbox — Linear"), 3);
        assert_eq!(parse_title_unread_count("(0) Quiet"), 0);
    }

    #[test]
    fn ignores_non_prefixed_titles() {
        assert_eq!(parse_title_unread_count("Slack"), 0);
        assert_eq!(parse_title_unread_count("Inbox (12)"), 0);
        assert_eq!(parse_title_unread_count(""), 0);
    }

    #[test]
    fn rejects_non_numeric_or_overflow_counts() {
        assert_eq!(parse_title_unread_count("(99+) Gmail"), 0);
        assert_eq!(parse_title_unread_count("(abc) Mail"), 0);
        assert_eq!(parse_title_unread_count("(99999999999999999999) huge"), 0);
    }

    #[test]
    fn tolerates_leading_whitespace() {
        assert_eq!(parse_title_unread_count("   (7) Slack"), 7);
    }
}

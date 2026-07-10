use std::time::{Duration, SystemTime};

use crate::{ProfileId, SpaceId};

pub const DEFAULT_SIDEBAR_WIDTH_PX: u16 = 280;
pub const COLLAPSED_SIDEBAR_WIDTH_PX: u16 = 56;
pub const HIDDEN_SIDEBAR_WIDTH_PX: u16 = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchivePolicy {
    Manual,
    IdleDays(u16),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Space {
    id: SpaceId,
    name: String,
    icon: String,
    accent_hex: u32,
    default_profile_id: ProfileId,
    archive_policy: ArchivePolicy,
    sidebar_width_px: u16,
    sort_key: u64,
    created_at: SystemTime,
    updated_at: SystemTime,
}

impl Space {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        icon: impl Into<String>,
        accent_hex: u32,
        default_profile_id: ProfileId,
        sort_key: u64,
    ) -> Self {
        let created_at = SystemTime::now();
        Self {
            id: SpaceId::new(),
            name: name.into(),
            icon: icon.into(),
            accent_hex,
            default_profile_id,
            archive_policy: ArchivePolicy::Manual,
            sidebar_width_px: DEFAULT_SIDEBAR_WIDTH_PX,
            sort_key,
            created_at,
            updated_at: created_at,
        }
    }

    #[must_use]
    pub fn id(&self) -> &SpaceId {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn icon(&self) -> &str {
        &self.icon
    }

    #[must_use]
    pub fn accent_hex(&self) -> u32 {
        self.accent_hex
    }

    #[must_use]
    pub fn default_profile_id(&self) -> &ProfileId {
        &self.default_profile_id
    }

    pub fn set_default_profile_id(&mut self, profile_id: ProfileId) {
        self.default_profile_id = profile_id;
        self.record_update();
    }

    pub fn set_accent_hex(&mut self, accent_hex: u32) {
        self.accent_hex = accent_hex;
        self.record_update();
    }

    pub fn set_presentation(
        &mut self,
        name: impl Into<String>,
        icon: impl Into<String>,
        accent_hex: u32,
    ) {
        self.name = name.into();
        self.icon = icon.into();
        self.accent_hex = accent_hex;
        self.record_update();
    }

    #[must_use]
    pub fn archive_policy(&self) -> &ArchivePolicy {
        &self.archive_policy
    }

    pub fn set_archive_policy(&mut self, archive_policy: ArchivePolicy) {
        self.archive_policy = archive_policy;
        self.record_update();
    }

    #[must_use]
    pub fn sidebar_width_px(&self) -> u16 {
        self.sidebar_width_px
    }

    pub fn set_sidebar_width_px(&mut self, sidebar_width_px: u16) {
        self.sidebar_width_px = sidebar_width_px;
        self.record_update();
    }

    #[must_use]
    pub fn sort_key(&self) -> u64 {
        self.sort_key
    }

    pub fn set_sort_key(&mut self, sort_key: u64) {
        self.sort_key = sort_key;
        self.record_update();
    }

    #[must_use]
    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    #[must_use]
    pub fn updated_at(&self) -> SystemTime {
        self.updated_at
    }

    fn record_update(&mut self) {
        let now = SystemTime::now();
        self.updated_at =
            if now > self.updated_at { now } else { self.updated_at + Duration::from_nanos(1) };
    }
}

use crate::{ProfileId, SpaceId};

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
}

impl Space {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        icon: impl Into<String>,
        accent_hex: u32,
        default_profile_id: ProfileId,
    ) -> Self {
        Self {
            id: SpaceId::new(),
            name: name.into(),
            icon: icon.into(),
            accent_hex,
            default_profile_id,
            archive_policy: ArchivePolicy::Manual,
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
    }

    #[must_use]
    pub fn archive_policy(&self) -> &ArchivePolicy {
        &self.archive_policy
    }

    pub fn set_archive_policy(&mut self, archive_policy: ArchivePolicy) {
        self.archive_policy = archive_policy;
    }
}

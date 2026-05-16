use crate::{DownloadPolicy, ProfileId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileKind {
    Standard,
    Private,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ProfileSyncPolicy {
    #[default]
    Enabled,
    Paused,
}

impl ProfileSyncPolicy {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Enabled => "Sync on",
            Self::Paused => "Sync paused",
        }
    }

    #[must_use]
    pub fn action_label(self) -> &'static str {
        match self {
            Self::Enabled => "Pause Sync",
            Self::Paused => "Resume Sync",
        }
    }

    #[must_use]
    pub fn toggled(self) -> Self {
        match self {
            Self::Enabled => Self::Paused,
            Self::Paused => Self::Enabled,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Profile {
    id: ProfileId,
    name: String,
    color_hex: u32,
    kind: ProfileKind,
    download_policy: DownloadPolicy,
    sync_policy: ProfileSyncPolicy,
}

impl Profile {
    #[must_use]
    pub fn new(name: impl Into<String>, color_hex: u32, kind: ProfileKind) -> Self {
        let sync_policy = match kind {
            ProfileKind::Standard => ProfileSyncPolicy::Enabled,
            ProfileKind::Private => ProfileSyncPolicy::Paused,
        };

        Self {
            id: ProfileId::new(),
            name: name.into(),
            color_hex,
            kind,
            download_policy: DownloadPolicy::ask_every_time(),
            sync_policy,
        }
    }

    #[must_use]
    pub fn restore(id: ProfileId, mut profile: Self) -> Self {
        profile.id = id;
        profile
    }

    #[must_use]
    pub fn id(&self) -> &ProfileId {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn color_hex(&self) -> u32 {
        self.color_hex
    }

    #[must_use]
    pub fn kind(&self) -> &ProfileKind {
        &self.kind
    }

    #[must_use]
    pub fn download_policy(&self) -> &DownloadPolicy {
        &self.download_policy
    }

    #[must_use]
    pub fn sync_policy(&self) -> ProfileSyncPolicy {
        self.sync_policy
    }

    #[must_use]
    pub fn allows_sync(&self) -> bool {
        self.kind == ProfileKind::Standard
    }

    pub fn set_download_policy(&mut self, download_policy: DownloadPolicy) {
        self.download_policy = download_policy;
    }

    pub fn set_presentation(&mut self, name: impl Into<String>, color_hex: u32) {
        self.name = name.into();
        self.color_hex = color_hex;
    }

    pub fn set_sync_policy(&mut self, sync_policy: ProfileSyncPolicy) {
        self.sync_policy = match self.kind {
            ProfileKind::Standard => sync_policy,
            ProfileKind::Private => ProfileSyncPolicy::Paused,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::{Profile, ProfileKind, ProfileSyncPolicy};
    use crate::ProfileId;

    #[test]
    fn restores_existing_profile_identity() {
        let id = ProfileId::new();
        let profile = Profile::new("Research", 0x9fc9a2, ProfileKind::Standard);
        let profile = Profile::restore(id.clone(), profile);

        assert_eq!(profile.id(), &id);
        assert_eq!(profile.name(), "Research");
        assert_eq!(profile.color_hex(), 0x9fc9a2);
    }

    #[test]
    fn updates_profile_presentation() {
        let mut profile = Profile::new("Old", 0x111111, ProfileKind::Standard);

        profile.set_presentation("New", 0x222222);

        assert_eq!(profile.name(), "New");
        assert_eq!(profile.color_hex(), 0x222222);
    }

    #[test]
    fn private_profile_sync_policy_stays_paused() {
        let mut profile = Profile::new("Private", 0x807d72, ProfileKind::Private);

        profile.set_sync_policy(ProfileSyncPolicy::Enabled);

        assert_eq!(profile.sync_policy(), ProfileSyncPolicy::Paused);
    }
}

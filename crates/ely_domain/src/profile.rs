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

    pub fn set_download_policy(&mut self, download_policy: DownloadPolicy) {
        self.download_policy = download_policy;
    }

    pub fn set_sync_policy(&mut self, sync_policy: ProfileSyncPolicy) {
        self.sync_policy = sync_policy;
    }
}

use crate::{DownloadPolicy, ProfileId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileKind {
    Standard,
    Private,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Profile {
    id: ProfileId,
    name: String,
    color_hex: u32,
    kind: ProfileKind,
    download_policy: DownloadPolicy,
}

impl Profile {
    #[must_use]
    pub fn new(name: impl Into<String>, color_hex: u32, kind: ProfileKind) -> Self {
        Self {
            id: ProfileId::new(),
            name: name.into(),
            color_hex,
            kind,
            download_policy: DownloadPolicy::ask_every_time(),
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

    pub fn set_download_policy(&mut self, download_policy: DownloadPolicy) {
        self.download_policy = download_policy;
    }
}

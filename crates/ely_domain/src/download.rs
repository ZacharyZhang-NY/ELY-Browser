use std::time::SystemTime;

use crate::{DomainError, DownloadId, ProfileId, UrlText};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DownloadState {
    InProgress,
    Paused,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadEntry {
    id: DownloadId,
    profile_id: ProfileId,
    source_url: UrlText,
    file_name: String,
    state: DownloadState,
    received_bytes: u64,
    total_bytes: Option<u64>,
    started_at: SystemTime,
}

impl DownloadEntry {
    pub fn started(
        profile_id: ProfileId,
        source_url: UrlText,
        file_name: impl Into<String>,
        total_bytes: Option<u64>,
        started_at: SystemTime,
    ) -> Result<Self, DomainError> {
        let file_name = file_name.into();
        let file_name = file_name.trim();
        if file_name.is_empty() {
            return Err(DomainError::EmptyField { field: "file_name" });
        }

        Ok(Self {
            id: DownloadId::new(),
            profile_id,
            source_url,
            file_name: file_name.to_string(),
            state: DownloadState::InProgress,
            received_bytes: 0,
            total_bytes,
            started_at,
        })
    }

    #[must_use]
    pub fn id(&self) -> &DownloadId {
        &self.id
    }

    #[must_use]
    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    #[must_use]
    pub fn source_url(&self) -> &UrlText {
        &self.source_url
    }

    #[must_use]
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    #[must_use]
    pub fn state(&self) -> &DownloadState {
        &self.state
    }

    #[must_use]
    pub fn received_bytes(&self) -> u64 {
        self.received_bytes
    }

    #[must_use]
    pub fn total_bytes(&self) -> Option<u64> {
        self.total_bytes
    }

    #[must_use]
    pub fn started_at(&self) -> SystemTime {
        self.started_at
    }
}

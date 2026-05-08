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

    pub fn pause(&mut self) -> Result<(), DomainError> {
        self.require_state("pause", &[DownloadState::InProgress])?;
        self.state = DownloadState::Paused;
        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), DomainError> {
        self.require_state("resume", &[DownloadState::Paused])?;
        self.state = DownloadState::InProgress;
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<(), DomainError> {
        self.require_state("cancel", &[DownloadState::InProgress, DownloadState::Paused])?;
        self.state = DownloadState::Cancelled;
        Ok(())
    }

    pub fn retry(&mut self) -> Result<(), DomainError> {
        self.require_state("retry", &[DownloadState::Cancelled, DownloadState::Failed])?;
        self.state = DownloadState::InProgress;
        self.received_bytes = 0;
        Ok(())
    }

    pub fn update_progress(&mut self, received_bytes: u64) -> Result<(), DomainError> {
        self.require_state("update progress", &[DownloadState::InProgress])?;
        self.validate_received_bytes(received_bytes)?;
        self.received_bytes = received_bytes;
        Ok(())
    }

    pub fn complete(&mut self, received_bytes: u64) -> Result<(), DomainError> {
        self.require_state("complete", &[DownloadState::InProgress])?;
        self.validate_received_bytes(received_bytes)?;
        self.received_bytes = received_bytes;
        self.state = DownloadState::Completed;
        Ok(())
    }

    pub fn fail(&mut self) -> Result<(), DomainError> {
        self.require_state("fail", &[DownloadState::InProgress, DownloadState::Paused])?;
        self.state = DownloadState::Failed;
        Ok(())
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

    fn require_state(
        &self,
        action: &'static str,
        allowed_states: &[DownloadState],
    ) -> Result<(), DomainError> {
        if allowed_states.iter().any(|state| state == &self.state) {
            return Ok(());
        }

        Err(DomainError::InvalidDownloadTransition { action, state: self.state.as_str() })
    }

    fn validate_received_bytes(&self, received_bytes: u64) -> Result<(), DomainError> {
        if let Some(total_bytes) = self.total_bytes
            && received_bytes > total_bytes
        {
            return Err(DomainError::InvalidDownloadProgress { received_bytes, total_bytes });
        }

        Ok(())
    }
}

impl DownloadState {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

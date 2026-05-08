use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

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
pub enum DownloadDestination {
    AskEveryTime,
    FixedDirectory(PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadPolicy {
    destination: DownloadDestination,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DownloadSecurity {
    Standard,
    DangerousExtension,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadEntry {
    id: DownloadId,
    profile_id: ProfileId,
    source_url: UrlText,
    file_name: String,
    destination: DownloadDestination,
    target_file_path: Option<PathBuf>,
    security: DownloadSecurity,
    state: DownloadState,
    received_bytes: u64,
    total_bytes: Option<u64>,
    started_at: SystemTime,
}

const DANGEROUS_DOWNLOAD_EXTENSIONS: &[&str] = &[
    "app",
    "applescript",
    "bat",
    "bash",
    "cmd",
    "com",
    "command",
    "dmg",
    "exe",
    "fish",
    "jar",
    "js",
    "jse",
    "msi",
    "msp",
    "pkg",
    "ps1",
    "psm1",
    "reg",
    "scr",
    "scpt",
    "sh",
    "terminal",
    "vbe",
    "vbs",
    "workflow",
    "zsh",
];

impl DownloadDestination {
    pub fn fixed_directory(path: impl Into<PathBuf>) -> Result<Self, DomainError> {
        let path = path.into();
        if path.as_os_str().is_empty() {
            return Err(DomainError::EmptyField { field: "download_directory" });
        }
        if !path.is_absolute() {
            return Err(DomainError::InvalidDownloadDirectory { path: path.display().to_string() });
        }

        Ok(Self::FixedDirectory(path))
    }

    #[must_use]
    pub fn as_path(&self) -> Option<&Path> {
        match self {
            Self::AskEveryTime => None,
            Self::FixedDirectory(path) => Some(path.as_path()),
        }
    }

    pub fn target_file_path(&self, file_name: &str) -> Result<Option<PathBuf>, DomainError> {
        validate_file_name(file_name)?;
        Ok(match self {
            Self::AskEveryTime => None,
            Self::FixedDirectory(path) => Some(path.join(file_name)),
        })
    }
}

impl DownloadPolicy {
    #[must_use]
    pub fn ask_every_time() -> Self {
        Self { destination: DownloadDestination::AskEveryTime }
    }

    pub fn fixed_directory(path: impl Into<PathBuf>) -> Result<Self, DomainError> {
        Ok(Self { destination: DownloadDestination::fixed_directory(path)? })
    }

    #[must_use]
    pub fn destination(&self) -> &DownloadDestination {
        &self.destination
    }
}

impl DownloadSecurity {
    #[must_use]
    pub fn for_file_name(file_name: &str) -> Self {
        match download_extension(file_name) {
            Some(extension) if is_dangerous_extension(extension) => Self::DangerousExtension,
            _ => Self::Standard,
        }
    }

    #[must_use]
    pub fn requires_prompt(&self) -> bool {
        matches!(self, Self::DangerousExtension)
    }
}

impl DownloadEntry {
    pub fn started(
        profile_id: ProfileId,
        source_url: UrlText,
        file_name: impl Into<String>,
        destination: DownloadDestination,
        total_bytes: Option<u64>,
        started_at: SystemTime,
    ) -> Result<Self, DomainError> {
        let file_name = file_name.into();
        let file_name = file_name.trim();
        if file_name.is_empty() {
            return Err(DomainError::EmptyField { field: "file_name" });
        }
        validate_file_name(file_name)?;
        let target_file_path = destination.target_file_path(file_name)?;

        Ok(Self {
            id: DownloadId::new(),
            profile_id,
            source_url,
            file_name: file_name.to_string(),
            destination,
            target_file_path,
            security: DownloadSecurity::for_file_name(file_name),
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
    pub fn destination(&self) -> &DownloadDestination {
        &self.destination
    }

    #[must_use]
    pub fn target_file_path(&self) -> Option<&Path> {
        self.target_file_path.as_deref()
    }

    #[must_use]
    pub fn security(&self) -> &DownloadSecurity {
        &self.security
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

fn validate_file_name(file_name: &str) -> Result<(), DomainError> {
    let has_path_separator = file_name.chars().any(|ch| ch == '/' || ch == '\\');
    let is_parent_reference = file_name == "." || file_name == "..";
    let has_control_character = file_name.chars().any(char::is_control);

    if has_path_separator || is_parent_reference || has_control_character {
        return Err(DomainError::InvalidFileName { value: file_name.to_string() });
    }

    Ok(())
}

fn download_extension(file_name: &str) -> Option<&str> {
    let (_, extension) = file_name.rsplit_once('.')?;
    if extension.is_empty() {
        return None;
    }

    Some(extension)
}

fn is_dangerous_extension(extension: &str) -> bool {
    DANGEROUS_DOWNLOAD_EXTENSIONS.iter().any(|candidate| extension.eq_ignore_ascii_case(candidate))
}

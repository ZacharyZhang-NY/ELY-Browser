use std::{path::PathBuf, time::SystemTime};

use ely_domain::{DownloadEntry, DownloadId, UrlText};

use crate::CoreError;

use super::BrowserCore;

impl BrowserCore {
    pub fn record_download_started(
        &mut self,
        source_url: UrlText,
        file_name: impl Into<String>,
        total_bytes: Option<u64>,
    ) -> Result<DownloadId, CoreError> {
        let destination = self.active_profile()?.download_policy().destination().clone();
        let entry = DownloadEntry::started(
            self.active_profile_id.clone(),
            source_url,
            file_name,
            destination,
            total_bytes,
            SystemTime::now(),
        )?;
        let download_id = entry.id().clone();
        self.download_entries.push(entry);
        Ok(download_id)
    }

    pub fn pause_download(&mut self, download_id: &DownloadId) -> Result<(), CoreError> {
        self.download_entry_mut(download_id)?.pause()?;
        Ok(())
    }

    pub fn resume_download(&mut self, download_id: &DownloadId) -> Result<(), CoreError> {
        self.download_entry_mut(download_id)?.resume()?;
        Ok(())
    }

    pub fn cancel_download(&mut self, download_id: &DownloadId) -> Result<(), CoreError> {
        self.download_entry_mut(download_id)?.cancel()?;
        Ok(())
    }

    pub fn retry_download(&mut self, download_id: &DownloadId) -> Result<(), CoreError> {
        self.download_entry_mut(download_id)?.retry()?;
        Ok(())
    }

    pub fn update_download_progress(
        &mut self,
        download_id: &DownloadId,
        received_bytes: u64,
    ) -> Result<(), CoreError> {
        self.download_entry_mut(download_id)?.update_progress(received_bytes)?;
        Ok(())
    }

    pub fn complete_download(
        &mut self,
        download_id: &DownloadId,
        received_bytes: u64,
    ) -> Result<(), CoreError> {
        self.download_entry_mut(download_id)?.complete(received_bytes)?;
        Ok(())
    }

    pub fn fail_download(&mut self, download_id: &DownloadId) -> Result<(), CoreError> {
        self.download_entry_mut(download_id)?.fail()?;
        Ok(())
    }

    pub fn download_target_file_path(
        &self,
        download_id: &DownloadId,
    ) -> Result<PathBuf, CoreError> {
        let entry = self.visible_download_entry(download_id)?;
        entry
            .target_file_path()
            .map(PathBuf::from)
            .ok_or_else(|| CoreError::DownloadTargetPathUnavailable { id: download_id.clone() })
    }

    pub fn clear_downloads_for_active_profile(&mut self) -> usize {
        let active_profile_id = self.active_profile_id.clone();
        let before_count = self.download_entries.len();
        self.download_entries.retain(|entry| entry.profile_id() != &active_profile_id);
        before_count - self.download_entries.len()
    }

    pub(super) fn visible_downloads(&self) -> Vec<DownloadEntry> {
        self.download_entries
            .iter()
            .filter(|entry| entry.profile_id() == &self.active_profile_id)
            .cloned()
            .collect()
    }

    fn visible_download_entry(
        &self,
        download_id: &DownloadId,
    ) -> Result<&DownloadEntry, CoreError> {
        self.download_entries
            .iter()
            .filter(|entry| entry.profile_id() == &self.active_profile_id)
            .find(|entry| entry.id() == download_id)
            .ok_or_else(|| CoreError::DownloadNotFound { id: download_id.clone() })
    }

    fn download_entry_mut(
        &mut self,
        download_id: &DownloadId,
    ) -> Result<&mut DownloadEntry, CoreError> {
        self.download_entries
            .iter_mut()
            .find(|entry| entry.id() == download_id)
            .ok_or_else(|| CoreError::DownloadNotFound { id: download_id.clone() })
    }
}

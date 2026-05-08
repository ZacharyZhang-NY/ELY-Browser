use std::time::SystemTime;

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
        let entry = DownloadEntry::started(
            self.active_profile_id.clone(),
            source_url,
            file_name,
            total_bytes,
            SystemTime::now(),
        )?;
        let download_id = entry.id().clone();
        self.download_entries.push(entry);
        Ok(download_id)
    }

    pub(super) fn visible_downloads(&self) -> Vec<DownloadEntry> {
        self.download_entries
            .iter()
            .filter(|entry| entry.profile_id() == &self.active_profile_id)
            .cloned()
            .collect()
    }
}

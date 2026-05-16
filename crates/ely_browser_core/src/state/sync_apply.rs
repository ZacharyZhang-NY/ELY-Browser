use ely_sync_client::SyncClientError;

use super::{BrowserCore, sync_context::SyncSnapshotApplyContext};
use crate::{sync_engine::SyncSnapshotApplySummary, sync_records::SyncSnapshotBody};

impl BrowserCore {
    pub(crate) fn apply_sync_snapshot_body(
        &mut self,
        body: SyncSnapshotBody,
    ) -> Result<SyncSnapshotApplySummary, SyncClientError> {
        let mut summary = SyncSnapshotApplySummary::default();
        let mut context = SyncSnapshotApplyContext::default();
        for record in body.profiles {
            self.apply_profile_sync_record(record, &mut summary, &mut context)?;
        }
        for record in body.spaces {
            self.apply_space_sync_record(record, &mut summary, &context)?;
        }
        for record in body.tabs {
            self.apply_tab_sync_record(record, &mut summary, &context)?;
        }
        for record in body.bookmarks {
            self.apply_bookmark_sync_record(record, &mut summary, &context)?;
        }
        for record in body.notes {
            self.apply_note_sync_record(record, &mut summary, &context)?;
        }
        for record in body.reading_list {
            self.apply_reading_list_sync_record(record, &mut summary, &context)?;
        }
        for record in body.site_permissions {
            self.apply_site_permission_sync_record(record, &mut summary, &context)?;
        }
        for record in body.history {
            self.apply_history_sync_record(record, &mut summary, &context)?;
        }
        for record in body.plugin_settings {
            self.apply_plugin_settings_sync_record(record, &mut summary)?;
        }
        Ok(summary)
    }
}

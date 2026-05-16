use ely_domain::{PluginId, SyncObjectKind, SyncObjectPolicy};
use ely_sync_client::SyncClientError;

use super::{BrowserCore, InstalledPlugin, sync::snapshot_schema_error};
use crate::{sync_engine::SyncSnapshotApplySummary, sync_records::PluginSettingsSyncRecord};

impl BrowserCore {
    pub(crate) fn visible_plugin_settings_for_sync(&self) -> Vec<&InstalledPlugin> {
        if self.sync_object_policy(SyncObjectKind::PluginSettings) == SyncObjectPolicy::Paused {
            return Vec::new();
        }
        self.installed_plugins.iter().collect()
    }

    pub(super) fn apply_plugin_settings_sync_record(
        &mut self,
        record: PluginSettingsSyncRecord,
        summary: &mut SyncSnapshotApplySummary,
    ) -> Result<(), SyncClientError> {
        let plugin_id = PluginId::parse(&record.plugin_id).map_err(snapshot_schema_error)?;
        let Some(plugin) =
            self.installed_plugins.iter_mut().find(|plugin| plugin.id() == &plugin_id)
        else {
            summary.record_skipped();
            return Ok(());
        };
        if plugin.manifest().checksum() != record.checksum {
            summary.record_skipped();
            return Ok(());
        }
        if plugin.apply_synced_settings(record.enabled, record.private_window_allowed) {
            summary.record_updated();
        } else {
            summary.record_skipped();
        }
        Ok(())
    }
}

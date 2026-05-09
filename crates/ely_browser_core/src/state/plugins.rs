use std::time::SystemTime;

use ely_domain::{PluginId, PluginManifest, ProfileKind};

use crate::CoreError;

use super::BrowserCore;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledPlugin {
    manifest: PluginManifest,
    enabled: bool,
    private_window_allowed: bool,
    high_risk_confirmed: bool,
    installed_at: SystemTime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginAuditAction {
    Installed,
    Enabled,
    Disabled,
    PrivateWindowAllowed,
    PrivateWindowBlocked,
    Uninstalled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginAuditEvent {
    plugin_id: PluginId,
    action: PluginAuditAction,
    created_at: SystemTime,
}

impl InstalledPlugin {
    fn new(manifest: PluginManifest, high_risk_confirmed: bool, installed_at: SystemTime) -> Self {
        Self {
            manifest,
            enabled: true,
            private_window_allowed: false,
            high_risk_confirmed,
            installed_at,
        }
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn set_private_window_allowed(&mut self, allowed: bool) {
        self.private_window_allowed = allowed;
    }

    #[must_use]
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    #[must_use]
    pub fn id(&self) -> &PluginId {
        self.manifest.id()
    }

    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn private_window_allowed(&self) -> bool {
        self.private_window_allowed
    }

    #[must_use]
    pub fn enabled_for_profile(&self, profile_kind: &ProfileKind) -> bool {
        self.enabled
            && match profile_kind {
                ProfileKind::Standard => true,
                ProfileKind::Private => self.private_window_allowed,
            }
    }

    #[must_use]
    pub fn high_risk_confirmed(&self) -> bool {
        self.high_risk_confirmed
    }

    #[must_use]
    pub fn installed_at(&self) -> SystemTime {
        self.installed_at
    }
}

impl PluginAuditEvent {
    fn record(plugin_id: PluginId, action: PluginAuditAction, created_at: SystemTime) -> Self {
        Self { plugin_id, action, created_at }
    }

    #[must_use]
    pub fn plugin_id(&self) -> &PluginId {
        &self.plugin_id
    }

    #[must_use]
    pub fn action(&self) -> &PluginAuditAction {
        &self.action
    }

    #[must_use]
    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }
}

impl BrowserCore {
    pub fn install_plugin(
        &mut self,
        manifest: PluginManifest,
        high_risk_confirmed: bool,
    ) -> Result<PluginId, CoreError> {
        let plugin_id = manifest.id().clone();
        if self.plugin_installed(&plugin_id) {
            return Err(CoreError::PluginAlreadyInstalled { id: plugin_id });
        }
        if manifest.high_risk_permissions().next().is_some() && !high_risk_confirmed {
            return Err(CoreError::PluginHighRiskConfirmationRequired { id: plugin_id });
        }

        self.installed_plugins.push(InstalledPlugin::new(
            manifest,
            high_risk_confirmed,
            SystemTime::now(),
        ));
        self.record_plugin_audit_event(plugin_id.clone(), PluginAuditAction::Installed);
        Ok(plugin_id)
    }

    pub fn enable_plugin(&mut self, plugin_id: &PluginId) -> Result<(), CoreError> {
        self.plugin_mut(plugin_id)?.set_enabled(true);
        self.record_plugin_audit_event(plugin_id.clone(), PluginAuditAction::Enabled);
        Ok(())
    }

    pub fn disable_plugin(&mut self, plugin_id: &PluginId) -> Result<(), CoreError> {
        self.plugin_mut(plugin_id)?.set_enabled(false);
        self.record_plugin_audit_event(plugin_id.clone(), PluginAuditAction::Disabled);
        Ok(())
    }

    pub fn set_plugin_private_window_allowed(
        &mut self,
        plugin_id: &PluginId,
        allowed: bool,
    ) -> Result<(), CoreError> {
        self.plugin_mut(plugin_id)?.set_private_window_allowed(allowed);
        let action = if allowed {
            PluginAuditAction::PrivateWindowAllowed
        } else {
            PluginAuditAction::PrivateWindowBlocked
        };
        self.record_plugin_audit_event(plugin_id.clone(), action);
        Ok(())
    }

    pub fn enabled_plugins_for_active_profile(&self) -> Result<Vec<InstalledPlugin>, CoreError> {
        let profile_kind = self.active_profile()?.kind();
        Ok(self
            .installed_plugins
            .iter()
            .filter(|plugin| plugin.enabled_for_profile(profile_kind))
            .cloned()
            .collect())
    }

    pub fn uninstall_plugin(&mut self, plugin_id: &PluginId) -> Result<(), CoreError> {
        let plugin_index = self
            .installed_plugins
            .iter()
            .position(|plugin| plugin.id() == plugin_id)
            .ok_or_else(|| CoreError::PluginNotFound { id: plugin_id.clone() })?;

        self.installed_plugins.remove(plugin_index);
        self.record_plugin_audit_event(plugin_id.clone(), PluginAuditAction::Uninstalled);
        Ok(())
    }

    fn record_plugin_audit_event(&mut self, plugin_id: PluginId, action: PluginAuditAction) {
        self.plugin_audit_events.push(PluginAuditEvent::record(
            plugin_id,
            action,
            SystemTime::now(),
        ));
    }

    fn plugin_installed(&self, plugin_id: &PluginId) -> bool {
        self.installed_plugins.iter().any(|plugin| plugin.id() == plugin_id)
    }

    fn plugin_mut(&mut self, plugin_id: &PluginId) -> Result<&mut InstalledPlugin, CoreError> {
        self.installed_plugins
            .iter_mut()
            .find(|plugin| plugin.id() == plugin_id)
            .ok_or_else(|| CoreError::PluginNotFound { id: plugin_id.clone() })
    }
}

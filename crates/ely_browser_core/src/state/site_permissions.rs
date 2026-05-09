use std::time::SystemTime;

use ely_domain::{
    ProfileId, SiteOrigin, SitePermissionAuditAction, SitePermissionAuditEvent,
    SitePermissionDecision, SitePermissionEntry, SitePermissionFeature, UrlText,
};

use crate::CoreError;

use super::BrowserCore;

impl BrowserCore {
    pub fn set_site_permission(
        &mut self,
        origin: SiteOrigin,
        feature: SitePermissionFeature,
        decision: SitePermissionDecision,
    ) -> Result<(), CoreError> {
        let profile_id = self.active_profile_id.clone();
        self.set_site_permission_for_profile(&profile_id, origin, feature, decision)
    }

    pub fn revoke_site_permission(
        &mut self,
        origin: &SiteOrigin,
        feature: SitePermissionFeature,
    ) -> Result<(), CoreError> {
        let profile_id = self.active_profile_id.clone();
        self.revoke_site_permission_for_profile(&profile_id, origin, feature)
    }

    pub fn clear_active_profile_site_permissions(&mut self) -> Result<usize, CoreError> {
        let profile_id = self.active_profile_id.clone();
        self.clear_site_permissions_for_profile(&profile_id)
    }

    pub fn active_tab_site_settings_url(&self) -> Result<Option<UrlText>, CoreError> {
        let active_tab = self.active_tab()?;
        let Some(origin) = SiteOrigin::from_url(active_tab.url())? else {
            return Ok(None);
        };
        origin.site_settings_url().map(Some).map_err(CoreError::from)
    }

    pub(super) fn visible_site_permissions(&self) -> Vec<SitePermissionEntry> {
        self.site_permissions
            .iter()
            .filter(|entry| entry.profile_id() == &self.active_profile_id)
            .cloned()
            .collect()
    }

    pub(super) fn visible_site_permission_audit_events(&self) -> Vec<SitePermissionAuditEvent> {
        self.site_permission_audit_events
            .iter()
            .filter(|event| event.profile_id() == &self.active_profile_id)
            .cloned()
            .collect()
    }

    fn set_site_permission_for_profile(
        &mut self,
        profile_id: &ProfileId,
        origin: SiteOrigin,
        feature: SitePermissionFeature,
        decision: SitePermissionDecision,
    ) -> Result<(), CoreError> {
        self.require_profile(profile_id)?;

        if let Some(entry) = self.site_permission_entry_mut(profile_id, &origin, feature) {
            if entry.decision() == decision {
                return Ok(());
            }
            entry.set_decision(decision);
        } else {
            self.site_permissions.push(SitePermissionEntry::new(
                profile_id.clone(),
                origin.clone(),
                feature,
                decision,
            ));
        }

        self.record_site_permission_audit_event(
            profile_id.clone(),
            origin,
            feature,
            SitePermissionAuditAction::Set(decision),
        );
        Ok(())
    }

    fn revoke_site_permission_for_profile(
        &mut self,
        profile_id: &ProfileId,
        origin: &SiteOrigin,
        feature: SitePermissionFeature,
    ) -> Result<(), CoreError> {
        self.require_profile(profile_id)?;
        let Some(index) = self.site_permission_entry_index(profile_id, origin, feature) else {
            return Ok(());
        };

        let entry = self.site_permissions.remove(index);
        self.record_site_permission_audit_event(
            profile_id.clone(),
            entry.origin().clone(),
            feature,
            SitePermissionAuditAction::Revoked,
        );
        Ok(())
    }

    fn clear_site_permissions_for_profile(
        &mut self,
        profile_id: &ProfileId,
    ) -> Result<usize, CoreError> {
        self.require_profile(profile_id)?;
        let mut revoked_permissions = Vec::new();

        self.site_permissions.retain(|entry| {
            if entry.profile_id() == profile_id {
                revoked_permissions.push((entry.origin().clone(), entry.feature()));
                false
            } else {
                true
            }
        });

        let revoked_count = revoked_permissions.len();
        for (origin, feature) in revoked_permissions {
            self.record_site_permission_audit_event(
                profile_id.clone(),
                origin,
                feature,
                SitePermissionAuditAction::Revoked,
            );
        }

        Ok(revoked_count)
    }

    fn require_profile(&self, profile_id: &ProfileId) -> Result<(), CoreError> {
        if self.profiles.iter().any(|profile| profile.id() == profile_id) {
            return Ok(());
        }

        Err(CoreError::ProfileNotFound { id: profile_id.clone() })
    }

    fn site_permission_entry_mut(
        &mut self,
        profile_id: &ProfileId,
        origin: &SiteOrigin,
        feature: SitePermissionFeature,
    ) -> Option<&mut SitePermissionEntry> {
        self.site_permissions.iter_mut().find(|entry| {
            entry.profile_id() == profile_id
                && entry.origin() == origin
                && entry.feature() == feature
        })
    }

    fn site_permission_entry_index(
        &self,
        profile_id: &ProfileId,
        origin: &SiteOrigin,
        feature: SitePermissionFeature,
    ) -> Option<usize> {
        self.site_permissions.iter().position(|entry| {
            entry.profile_id() == profile_id
                && entry.origin() == origin
                && entry.feature() == feature
        })
    }

    pub(super) fn record_site_permission_audit_event(
        &mut self,
        profile_id: ProfileId,
        origin: SiteOrigin,
        feature: SitePermissionFeature,
        action: SitePermissionAuditAction,
    ) {
        self.site_permission_audit_events.push(SitePermissionAuditEvent::new(
            profile_id,
            origin,
            feature,
            action,
            SystemTime::now(),
        ));
    }
}

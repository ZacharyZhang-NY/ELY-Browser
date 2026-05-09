use ely_domain::{HistoryEntry, ProfileId, SiteOrigin, SitePermissionAuditAction};

use crate::CoreError;

use super::BrowserCore;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SiteDataClearance {
    history_entries: usize,
    site_permissions: usize,
}

impl SiteDataClearance {
    #[must_use]
    pub fn new(history_entries: usize, site_permissions: usize) -> Self {
        Self { history_entries, site_permissions }
    }

    #[must_use]
    pub fn history_entries(self) -> usize {
        self.history_entries
    }

    #[must_use]
    pub fn site_permissions(self) -> usize {
        self.site_permissions
    }

    #[must_use]
    pub fn total_items(self) -> usize {
        self.history_entries + self.site_permissions
    }
}

impl BrowserCore {
    pub fn clear_active_profile_site_data(
        &mut self,
    ) -> Result<Option<SiteDataClearance>, CoreError> {
        let Some(origin) = SiteOrigin::from_url(self.active_tab()?.url())? else {
            return Ok(None);
        };
        let profile_id = self.active_profile_id.clone();
        let history_entries = self.clear_profile_history_for_origin(&profile_id, &origin)?;
        let site_permissions = self.clear_site_permissions_for_origin(&profile_id, &origin)?;

        Ok(Some(SiteDataClearance::new(history_entries, site_permissions)))
    }

    fn clear_profile_history_for_origin(
        &mut self,
        profile_id: &ProfileId,
        origin: &SiteOrigin,
    ) -> Result<usize, CoreError> {
        self.require_site_data_profile(profile_id)?;
        let original_count = self.history_entries.len();
        self.history_entries.retain(|entry| {
            entry.profile_id() != profile_id || !history_entry_matches_origin(entry, origin)
        });
        Ok(original_count - self.history_entries.len())
    }

    fn clear_site_permissions_for_origin(
        &mut self,
        profile_id: &ProfileId,
        origin: &SiteOrigin,
    ) -> Result<usize, CoreError> {
        self.require_site_data_profile(profile_id)?;
        let mut revoked_permissions = Vec::new();

        self.site_permissions.retain(|entry| {
            if entry.profile_id() == profile_id && entry.origin() == origin {
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

    fn require_site_data_profile(&self, profile_id: &ProfileId) -> Result<(), CoreError> {
        if self.profiles.iter().any(|profile| profile.id() == profile_id) {
            return Ok(());
        }

        Err(CoreError::ProfileNotFound { id: profile_id.clone() })
    }
}

fn history_entry_matches_origin(entry: &HistoryEntry, origin: &SiteOrigin) -> bool {
    SiteOrigin::from_url(entry.url()).ok().flatten().as_ref() == Some(origin)
}

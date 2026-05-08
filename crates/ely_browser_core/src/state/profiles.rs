use ely_domain::{DownloadPolicy, Profile, ProfileId, ProfileKind, TabId};

use crate::CoreError;

use super::BrowserCore;

impl BrowserCore {
    pub fn create_profile(
        &mut self,
        name: impl Into<String>,
        color_hex: u32,
        kind: ProfileKind,
    ) -> Result<ProfileId, CoreError> {
        let profile = Profile::new(name, color_hex, kind);
        let profile_id = profile.id().clone();
        self.profiles.push(profile);
        self.select_profile(&profile_id)?;
        Ok(profile_id)
    }

    pub fn select_profile(&mut self, profile_id: &ProfileId) -> Result<TabId, CoreError> {
        if !self.profiles.iter().any(|profile| profile.id() == profile_id) {
            return Err(CoreError::ProfileNotFound { id: profile_id.clone() });
        }

        let active_key = (self.active_space_id.clone(), profile_id.clone());
        if let Some(tab_id) = self
            .active_tabs_by_space_profile
            .get(&active_key)
            .filter(|tab_id| {
                self.tabs.iter().any(|tab| {
                    tab.id() == *tab_id
                        && tab.space_id() == &active_key.0
                        && tab.profile_id() == &active_key.1
                })
            })
            .cloned()
        {
            self.select_tab(&tab_id)?;
            return Ok(tab_id);
        }

        if let Some(tab_id) = self
            .tabs
            .iter()
            .rfind(|tab| tab.space_id() == &active_key.0 && tab.profile_id() == &active_key.1)
            .map(|tab| tab.id().clone())
        {
            self.select_tab(&tab_id)?;
            return Ok(tab_id);
        }

        let tab = self.build_tab_for(
            self.active_space_id.clone(),
            profile_id.clone(),
            self.new_tab_url()?,
        );
        let tab_id = tab.id().clone();
        let insert_index = self
            .tabs
            .iter()
            .position(|existing| existing.id() == &self.active_tab_id)
            .map_or(self.tabs.len(), |index| index + 1);

        self.tabs.insert(insert_index, tab);
        self.select_tab(&tab_id)?;
        Ok(tab_id)
    }

    pub fn set_profile_download_policy(
        &mut self,
        profile_id: &ProfileId,
        download_policy: DownloadPolicy,
    ) -> Result<(), CoreError> {
        let profile = self
            .profiles
            .iter_mut()
            .find(|profile| profile.id() == profile_id)
            .ok_or_else(|| CoreError::ProfileNotFound { id: profile_id.clone() })?;

        profile.set_download_policy(download_policy);
        Ok(())
    }
}

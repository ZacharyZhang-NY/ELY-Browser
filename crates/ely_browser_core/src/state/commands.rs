use ely_domain::{CommandIntent, CommandScope, ProfileId, ProfileKind, SpaceId};

use crate::{
    CoreError,
    navigation::{
        downloads_url, history_url, move_tab_space_name, new_profile_name, new_space_name,
        search_url, settings_page_url, settings_url, space_icon, switch_profile_name, sync_url,
    },
};

use super::BrowserCore;

impl BrowserCore {
    pub fn submit_command(&mut self) -> Result<Option<CommandIntent>, CoreError> {
        let query = self.command_query.trim();
        if query.is_empty() {
            return Ok(None);
        }

        let intent = CommandIntent::parse(query)?;
        match &intent {
            CommandIntent::Navigate(url) => {
                self.open_tab(url.clone());
                self.command_query.clear();
            }
            CommandIntent::Search(query) => {
                let url = search_url(query)?;
                self.open_tab(url);
                self.command_query.clear();
            }
            CommandIntent::Command(command) if self.submit_named_command(command)? => {
                self.command_query.clear();
            }
            CommandIntent::Command(_) => {}
            CommandIntent::ScopedSearch { scope: CommandScope::Tabs, query } => {
                if let Some(tab_id) = self.find_tab_match(query) {
                    self.select_tab(&tab_id)?;
                    self.command_query.clear();
                }
            }
            CommandIntent::ScopedSearch { scope: CommandScope::Spaces, query } => {
                if let Some(space_id) = self.find_space_match(query) {
                    self.select_space(&space_id)?;
                    self.command_query.clear();
                }
            }
            CommandIntent::ScopedSearch { scope: CommandScope::History, query } => {
                if let Some(url) = self.find_history_match(query) {
                    self.open_tab(url);
                    self.command_query.clear();
                }
            }
            CommandIntent::ScopedSearch { scope: CommandScope::Settings, query } => {
                if let Some(url) = settings_page_url(query)? {
                    self.open_tab(url);
                    self.command_query.clear();
                }
            }
            CommandIntent::ScopedSearch { scope: CommandScope::Archive, query }
                if self.restore_archived_tab_match(query)?.is_some() =>
            {
                self.command_query.clear();
            }
            _ => {}
        }

        Ok(Some(intent))
    }

    fn submit_named_command(&mut self, command: &str) -> Result<bool, CoreError> {
        let command = command.trim();
        if let Some(name) = new_space_name(command) {
            self.create_space(name.to_string(), space_icon(name), 0xf54e00)?;
            return Ok(true);
        }
        if let Some(name) = new_profile_name(command) {
            self.create_profile(name.to_string(), 0xf54e00, ProfileKind::Standard)?;
            return Ok(true);
        }
        if let Some(name) = move_tab_space_name(command) {
            let Some(space_id) = self.find_space_match(name) else {
                return Ok(false);
            };
            self.move_active_tab_to_space(&space_id)?;
            return Ok(true);
        }
        if let Some(name) = switch_profile_name(command) {
            let Some(profile_id) = self.find_profile_match(name) else {
                return Ok(false);
            };
            self.select_profile(&profile_id)?;
            return Ok(true);
        }

        match command.to_ascii_lowercase().as_str() {
            "new-tab" => {
                self.open_tab(self.new_tab_url.clone());
                Ok(true)
            }
            "downloads" | "open-downloads" | "open downloads" => {
                self.open_tab(downloads_url()?);
                Ok(true)
            }
            "history" | "open-history" | "open history" => {
                self.open_tab(history_url()?);
                Ok(true)
            }
            "settings" | "open-settings" | "open settings" => {
                self.open_tab(settings_url()?);
                Ok(true)
            }
            "sync" | "open-sync-status" | "open sync status" => {
                self.open_tab(sync_url()?);
                Ok(true)
            }
            "close-tab" => {
                self.close_active_tab()?;
                Ok(true)
            }
            "favorite" | "toggle-favorite" => {
                self.toggle_active_tab_favorite()?;
                Ok(true)
            }
            "pin" | "pin-tab" | "toggle-pin" => {
                self.toggle_active_tab_pinned()?;
                Ok(true)
            }
            "restore-tab" | "reopen-tab" => {
                self.restore_last_archived_tab()?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn find_space_match(&self, query: &str) -> Option<SpaceId> {
        let query = query.trim().to_lowercase();
        self.spaces.iter().find_map(|space| {
            (space.name().to_lowercase().contains(&query)
                || space.icon().to_lowercase().contains(&query))
            .then(|| space.id().clone())
        })
    }

    fn find_profile_match(&self, query: &str) -> Option<ProfileId> {
        let query = query.trim().to_lowercase();
        self.profiles
            .iter()
            .find(|profile| profile.name().to_lowercase().contains(&query))
            .map(|profile| profile.id().clone())
    }
}

use std::time::SystemTime;

use ely_domain::{
    ArchivePolicy, CommandIntent, CommandScope, ProfileId, ProfileKind, SpaceId, SplitAxis,
};

use crate::{
    CoreError,
    navigation::{
        about_url, archive_idle_days, archive_url, bookmarks_url, downloads_url, history_url,
        move_tab_space_name, new_private_profile_name, new_profile_name, new_space_name, note_body,
        notes_url, plugin_detail_url, plugins_url, reading_list_url, reading_progress_percent,
        search_url, settings_page_url, settings_url, shortcut_settings_url, space_icon,
        split_group_name, switch_profile_name, sync_status_url, tab_group_name, tab_note_body,
        task_manager_url,
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
                let url = search_url(query, self.search_engine)?;
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
            CommandIntent::ScopedSearch { scope: CommandScope::Bookmarks, query } => {
                if let Some(url) = self.find_bookmark_match(query) {
                    self.open_tab(url);
                    self.command_query.clear();
                }
            }
            CommandIntent::ScopedSearch { scope: CommandScope::Notes, query } => {
                if let Some(url) = self.find_note_match(query) {
                    self.open_tab(url);
                    self.command_query.clear();
                }
            }
            CommandIntent::ScopedSearch { scope: CommandScope::ReadingList, query } => {
                if let Some(url) = self.find_reading_list_match(query) {
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
            CommandIntent::ScopedSearch { scope: CommandScope::Plugins, query } => {
                if let Some(url) = self.find_plugin_match(query)? {
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
        if let Some(name) = new_private_profile_name(command) {
            self.create_profile(name.to_string(), 0x807d72, ProfileKind::Private)?;
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
        if let Some(idle_days) = archive_idle_days(command) {
            self.set_active_space_archive_policy(ArchivePolicy::IdleDays(idle_days))?;
            self.archive_idle_tabs(SystemTime::now())?;
            return Ok(true);
        }
        if let Some(percent) = reading_progress_percent(command)? {
            self.set_active_tab_reading_progress(percent)?;
            return Ok(true);
        }
        if command.eq_ignore_ascii_case("tab group to split") {
            return Ok(self.split_active_tab_group()?.is_some());
        }
        if let Some(name) = split_group_name(command) {
            return Ok(self.group_active_split_view(Some(name))?.is_some());
        }
        if let Some(name) = tab_group_name(command) {
            self.group_active_tab(name)?;
            return Ok(true);
        }
        if let Some(body) = tab_note_body(command) {
            self.save_active_tab_note(body)?;
            return Ok(true);
        }
        if let Some(body) = note_body(command) {
            self.save_active_url_note(body)?;
            return Ok(true);
        }

        match command.to_ascii_lowercase().as_str() {
            "new-tab" => {
                self.open_new_tab()?;
                Ok(true)
            }
            "split-right" | "split right" => {
                self.split_active_tab_right()?;
                Ok(true)
            }
            "split-horizontal" | "split horizontal" => {
                Ok(self.set_active_split_axis(SplitAxis::Horizontal)?.is_some())
            }
            "split-vertical" | "split vertical" => {
                Ok(self.set_active_split_axis(SplitAxis::Vertical)?.is_some())
            }
            "split-grid" | "split grid" => {
                Ok(self.set_active_split_axis(SplitAxis::Grid)?.is_some())
            }
            "detach-split-pane" | "detach split pane" | "detach-pane" | "detach pane" => {
                self.detach_active_split_pane()
            }
            "duplicate-split-pane"
            | "duplicate split pane"
            | "duplicate-pane"
            | "duplicate pane" => Ok(self.duplicate_active_split_pane()?.is_some()),
            "save-split-view" | "save split view" => Ok(self.save_active_split_view()?.is_some()),
            "split-tab-group" | "split tab group" | "tab-group-to-split" | "tab group to split" => {
                Ok(self.split_active_tab_group()?.is_some())
            }
            "group-split-view"
            | "group split view"
            | "split-view-to-group"
            | "split view to group" => Ok(self.group_active_split_view(None)?.is_some()),
            "toggle-tab-group" | "toggle tab group" => {
                Ok(self.toggle_active_tab_group_collapsed()?.is_some())
            }
            "collapse-tab-group" | "collapse tab group" => {
                Ok(self.set_active_tab_group_collapsed(true)?.is_some())
            }
            "expand-tab-group" | "expand tab group" => {
                Ok(self.set_active_tab_group_collapsed(false)?.is_some())
            }
            "ungroup-tab" | "ungroup tab" => self.ungroup_active_tab(),
            "downloads" | "open-downloads" | "open downloads" => {
                self.open_tab(downloads_url()?);
                Ok(true)
            }
            "bookmarks" | "open-bookmarks" | "open bookmarks" => {
                self.open_tab(bookmarks_url()?);
                Ok(true)
            }
            "reading-list" | "open-reading-list" | "open reading list" => {
                self.open_tab(reading_list_url()?);
                Ok(true)
            }
            "notes" | "open-notes" | "open notes" => {
                self.open_tab(notes_url()?);
                Ok(true)
            }
            "history" | "open-history" | "open history" => {
                self.open_tab(history_url()?);
                Ok(true)
            }
            "archive" | "open-archive" | "open archive" => {
                self.open_tab(archive_url()?);
                Ok(true)
            }
            "task-manager" | "tasks" | "open-task-manager" | "open task manager" => {
                self.open_tab(task_manager_url()?);
                Ok(true)
            }
            "plugins"
            | "open-plugins"
            | "open plugins"
            | "plugin-marketplace"
            | "open-plugin-marketplace"
            | "open plugin marketplace" => {
                self.open_tab(plugins_url()?);
                Ok(true)
            }
            "site-settings" | "open-site-settings" | "open site settings" => {
                let Some(url) = self.active_tab_site_settings_url()? else {
                    return Ok(false);
                };
                self.open_tab(url);
                Ok(true)
            }
            "about" | "open-about" | "open about" => {
                self.open_tab(about_url()?);
                Ok(true)
            }
            "settings" | "open-settings" | "open settings" => {
                self.open_tab(settings_url()?);
                Ok(true)
            }
            "shortcuts" | "open-shortcuts" | "open shortcuts" => {
                self.open_tab(shortcut_settings_url()?);
                Ok(true)
            }
            "sync" | "open-sync-status" | "open sync status" => {
                self.open_tab(sync_status_url()?);
                Ok(true)
            }
            "close-tab" => {
                self.close_active_tab()?;
                Ok(true)
            }
            "close-split-view" | "close split view" => {
                Ok(self.close_active_saved_split_view()?.is_some())
            }
            "archive-idle-tabs" | "archive idle tabs" => {
                self.archive_idle_tabs(SystemTime::now())?;
                Ok(true)
            }
            "favorite" | "toggle-favorite" => {
                self.toggle_active_tab_favorite()?;
                Ok(true)
            }
            "bookmark" | "add-bookmark" | "add bookmark" => {
                self.bookmark_active_tab()?;
                Ok(true)
            }
            "save-reading-list" | "save reading list" | "read-later" | "read later" => {
                self.save_active_tab_to_reading_list()?;
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

    fn find_plugin_match(&self, query: &str) -> Result<Option<ely_domain::UrlText>, CoreError> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Ok(None);
        }

        self.installed_plugins
            .iter()
            .find(|plugin| {
                plugin.id().as_str().to_lowercase().contains(&query)
                    || plugin.manifest().name().to_lowercase().contains(&query)
                    || plugin.manifest().description().to_lowercase().contains(&query)
            })
            .map(|plugin| plugin_detail_url(plugin.id()))
            .transpose()
    }
}

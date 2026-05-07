use ely_domain::{
    BrowserTab, CommandIntent, CommandScope, DomainError, Profile, ProfileId, ProfileKind, Space,
    SpaceId, TabId, UrlText,
};
use url::Url;

use crate::CoreError;

const DEFAULT_SEARCH_URL: &str = "https://duckduckgo.com/";
const DEFAULT_FAVORITE_LIMIT: usize = 12;

#[derive(Clone, Debug)]
pub struct InitialBrowserConfig {
    pub space_name: String,
    pub space_icon: String,
    pub profile_name: String,
    pub initial_url: UrlText,
}

impl InitialBrowserConfig {
    pub fn ely_defaults() -> Result<Self, DomainError> {
        Ok(Self {
            space_name: "Work".to_string(),
            space_icon: "W".to_string(),
            profile_name: "Default".to_string(),
            initial_url: UrlText::parse("ely://new-tab")?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct BrowserSnapshot {
    pub tabs: Vec<BrowserTab>,
    pub favorites: Vec<BrowserTab>,
    pub active_tab_id: TabId,
    pub active_space_name: String,
    pub active_profile_name: String,
    pub command_query: String,
}

#[derive(Debug)]
pub struct BrowserCore {
    spaces: Vec<Space>,
    profiles: Vec<Profile>,
    tabs: Vec<BrowserTab>,
    active_space_id: SpaceId,
    active_profile_id: ProfileId,
    active_tab_id: TabId,
    command_query: String,
    new_tab_url: UrlText,
}

impl BrowserCore {
    pub fn new(config: InitialBrowserConfig) -> Result<Self, CoreError> {
        let space = Space::new(config.space_name, config.space_icon, 0xf54e00);
        let profile = Profile::new(config.profile_name, 0x26251e, ProfileKind::Standard);
        let new_tab_url = config.initial_url;
        let tab = BrowserTab::new(
            TabId::new(),
            space.id().clone(),
            profile.id().clone(),
            "New Tab",
            new_tab_url.clone(),
        );

        Ok(Self {
            active_space_id: space.id().clone(),
            active_profile_id: profile.id().clone(),
            active_tab_id: tab.id().clone(),
            spaces: vec![space],
            profiles: vec![profile],
            tabs: vec![tab],
            command_query: String::new(),
            new_tab_url,
        })
    }

    pub fn open_tab(&mut self, url: UrlText) -> TabId {
        let tab = self.build_tab(url);
        let tab_id = tab.id().clone();
        let insert_index = self
            .tabs
            .iter()
            .position(|existing| existing.id() == &self.active_tab_id)
            .map_or(self.tabs.len(), |index| index + 1);
        self.tabs.insert(insert_index, tab);
        self.active_tab_id = tab_id.clone();
        tab_id
    }

    pub fn close_active_tab(&mut self) -> Result<TabId, CoreError> {
        let tab_id = self.active_tab_id.clone();
        self.close_tab(&tab_id)
    }

    pub fn close_tab(&mut self, tab_id: &TabId) -> Result<TabId, CoreError> {
        let close_index = self
            .tabs
            .iter()
            .position(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        let was_active = &self.active_tab_id == tab_id;

        self.tabs.remove(close_index);

        if self.tabs.is_empty() {
            let tab = self.build_tab(self.new_tab_url.clone());
            let replacement_id = tab.id().clone();
            self.tabs.push(tab);
            self.active_tab_id = replacement_id.clone();
            return Ok(replacement_id);
        }

        if was_active {
            let next_index = close_index.min(self.tabs.len() - 1);
            let next_tab_id = self.tabs[next_index].id().clone();
            self.select_tab(&next_tab_id)?;
        }

        Ok(self.active_tab_id.clone())
    }

    pub fn select_tab(&mut self, tab_id: &TabId) -> Result<(), CoreError> {
        let tab = self
            .tabs
            .iter()
            .find(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;

        self.active_tab_id = tab.id().clone();
        self.active_space_id = tab.space_id().clone();
        self.active_profile_id = tab.profile_id().clone();
        Ok(())
    }

    pub fn select_next_tab(&mut self) -> Result<TabId, CoreError> {
        self.select_tab_by_offset(1)
    }

    pub fn select_previous_tab(&mut self) -> Result<TabId, CoreError> {
        self.select_tab_by_offset(-1)
    }

    pub fn toggle_active_tab_favorite(&mut self) -> Result<bool, CoreError> {
        let active_index = self.active_tab_index()?;
        let favorite_count = self.tabs.iter().filter(|tab| tab.flags().favorite).count();
        let active_tab = self.tabs.get_mut(active_index).ok_or(CoreError::MissingActiveTab)?;
        let next_favorite = !active_tab.flags().favorite;

        if next_favorite && favorite_count >= DEFAULT_FAVORITE_LIMIT {
            return Err(CoreError::FavoriteLimitReached { limit: DEFAULT_FAVORITE_LIMIT });
        }

        active_tab.set_favorite(next_favorite);
        Ok(next_favorite)
    }

    pub fn set_command_query(&mut self, query: impl Into<String>) {
        self.command_query = query.into();
    }

    #[must_use]
    pub fn command_query(&self) -> &str {
        &self.command_query
    }

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
            CommandIntent::ScopedSearch { scope: CommandScope::Tabs, query } => {
                if let Some(tab_id) = self.find_tab_match(query) {
                    self.select_tab(&tab_id)?;
                    self.command_query.clear();
                }
            }
            _ => {}
        }

        Ok(Some(intent))
    }

    pub fn snapshot(&self) -> Result<BrowserSnapshot, CoreError> {
        let active_space = self
            .spaces
            .iter()
            .find(|space| space.id() == &self.active_space_id)
            .ok_or(CoreError::MissingActiveTab)?;
        let active_profile = self
            .profiles
            .iter()
            .find(|profile| profile.id() == &self.active_profile_id)
            .ok_or(CoreError::MissingActiveTab)?;

        Ok(BrowserSnapshot {
            favorites: self.favorites(),
            tabs: self.tabs.clone(),
            active_tab_id: self.active_tab_id.clone(),
            active_space_name: active_space.name().to_string(),
            active_profile_name: active_profile.name().to_string(),
            command_query: self.command_query.clone(),
        })
    }

    pub fn active_tab(&self) -> Result<&BrowserTab, CoreError> {
        self.tabs
            .iter()
            .find(|tab| tab.id() == &self.active_tab_id)
            .ok_or(CoreError::MissingActiveTab)
    }

    fn select_tab_by_offset(&mut self, offset: isize) -> Result<TabId, CoreError> {
        let active_index = self.active_tab_index()?;
        let tab_count = self.tabs.len() as isize;
        let next_index = (active_index as isize + offset).rem_euclid(tab_count) as usize;
        let next_tab_id = self.tabs[next_index].id().clone();
        self.select_tab(&next_tab_id)?;
        Ok(next_tab_id)
    }

    fn active_tab_index(&self) -> Result<usize, CoreError> {
        self.tabs
            .iter()
            .position(|tab| tab.id() == &self.active_tab_id)
            .ok_or(CoreError::MissingActiveTab)
    }

    fn favorites(&self) -> Vec<BrowserTab> {
        self.tabs.iter().filter(|tab| tab.flags().favorite).cloned().collect()
    }

    fn find_tab_match(&self, query: &str) -> Option<TabId> {
        let normalized_query = query.trim().to_lowercase();
        self.tabs
            .iter()
            .find(|tab| tab_matches_query(tab, &normalized_query))
            .map(|tab| tab.id().clone())
    }

    fn build_tab(&self, url: UrlText) -> BrowserTab {
        let title = tab_title(&url);
        BrowserTab::new(
            TabId::new(),
            self.active_space_id.clone(),
            self.active_profile_id.clone(),
            title,
            url,
        )
    }
}

fn tab_title(url: &UrlText) -> String {
    if url.as_str() == "ely://new-tab" {
        return "New Tab".to_string();
    }

    url.display_host()
}

fn tab_matches_query(tab: &BrowserTab, normalized_query: &str) -> bool {
    tab.title().to_lowercase().contains(normalized_query)
        || tab.url().as_str().to_lowercase().contains(normalized_query)
        || tab.display_url().to_lowercase().contains(normalized_query)
}

fn search_url(query: &str) -> Result<UrlText, CoreError> {
    let mut url = Url::parse(DEFAULT_SEARCH_URL)
        .map_err(|_| DomainError::InvalidUrl { value: DEFAULT_SEARCH_URL.to_string() })?;
    url.query_pairs_mut().append_pair("q", query);
    UrlText::parse(url.to_string()).map_err(CoreError::from)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use ely_domain::{CommandIntent, CommandScope, UrlText};

    use super::{BrowserCore, InitialBrowserConfig};
    use crate::CoreError;

    #[test]
    fn opens_new_tab_below_active_tab() -> Result<(), Box<dyn Error>> {
        let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
        let first_tab_id = core.active_tab()?.id().clone();
        let second_tab_id = core.open_tab(UrlText::parse("https://example.com")?);

        core.select_tab(&first_tab_id)?;
        let third_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);

        let snapshot = core.snapshot()?;
        let ordered_ids = snapshot.tabs.iter().map(|tab| tab.id().clone()).collect::<Vec<_>>();

        assert_eq!(ordered_ids, vec![first_tab_id, third_tab_id.clone(), second_tab_id]);
        assert_eq!(snapshot.active_tab_id, third_tab_id);
        Ok(())
    }

    #[test]
    fn closes_active_tab_and_selects_next_neighbor() -> Result<(), Box<dyn Error>> {
        let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
        let first_tab_id = core.active_tab()?.id().clone();
        let second_tab_id = core.open_tab(UrlText::parse("https://example.com")?);
        let third_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);

        core.select_tab(&second_tab_id)?;
        let active_tab_id = core.close_active_tab()?;

        let snapshot = core.snapshot()?;
        let ordered_ids = snapshot.tabs.iter().map(|tab| tab.id().clone()).collect::<Vec<_>>();

        assert_eq!(active_tab_id, third_tab_id);
        assert_eq!(ordered_ids, vec![first_tab_id, active_tab_id.clone()]);
        assert_eq!(snapshot.active_tab_id, active_tab_id);
        Ok(())
    }

    #[test]
    fn closing_last_tab_replaces_it_with_new_tab() -> Result<(), Box<dyn Error>> {
        let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
        let closed_tab_id = core.active_tab()?.id().clone();

        let active_tab_id = core.close_tab(&closed_tab_id)?;

        let snapshot = core.snapshot()?;
        assert_eq!(snapshot.tabs.len(), 1);
        assert_eq!(snapshot.active_tab_id, active_tab_id);
        let replacement_tab = snapshot.tabs.first().ok_or(CoreError::MissingActiveTab)?;
        assert_eq!(replacement_tab.url().as_str(), "ely://new-tab");
        Ok(())
    }

    #[test]
    fn selects_next_tab_with_wraparound() -> Result<(), Box<dyn Error>> {
        let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
        let first_tab_id = core.active_tab()?.id().clone();
        let second_tab_id = core.open_tab(UrlText::parse("https://example.com")?);
        let third_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);

        core.select_tab(&first_tab_id)?;
        let next_tab_id = core.select_next_tab()?;
        assert_eq!(next_tab_id, second_tab_id);

        core.select_tab(&third_tab_id)?;
        let wrapped_tab_id = core.select_next_tab()?;
        assert_eq!(wrapped_tab_id, first_tab_id);
        Ok(())
    }

    #[test]
    fn selects_previous_tab_with_wraparound() -> Result<(), Box<dyn Error>> {
        let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
        let first_tab_id = core.active_tab()?.id().clone();
        let second_tab_id = core.open_tab(UrlText::parse("https://example.com")?);
        let third_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);

        let previous_tab_id = core.select_previous_tab()?;
        assert_eq!(previous_tab_id, second_tab_id);

        core.select_tab(&first_tab_id)?;
        let wrapped_tab_id = core.select_previous_tab()?;
        assert_eq!(wrapped_tab_id, third_tab_id);
        Ok(())
    }

    #[test]
    fn tab_scoped_search_selects_matching_open_tab() -> Result<(), Box<dyn Error>> {
        let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
        let first_tab_id = core.active_tab()?.id().clone();
        core.open_tab(UrlText::parse("https://example.com")?);
        let servo_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);

        core.select_tab(&first_tab_id)?;
        core.set_command_query("@tabs servo");
        let intent = core.submit_command()?;

        let snapshot = core.snapshot()?;
        assert!(matches!(
            intent,
            Some(CommandIntent::ScopedSearch { scope: CommandScope::Tabs, query }) if query == "servo"
        ));
        assert_eq!(snapshot.active_tab_id, servo_tab_id);
        assert_eq!(snapshot.command_query, "");
        Ok(())
    }

    #[test]
    fn tab_scoped_search_preserves_query_without_match() -> Result<(), Box<dyn Error>> {
        let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
        let active_tab_id = core.active_tab()?.id().clone();

        core.set_command_query("@tabs absent");
        core.submit_command()?;

        let snapshot = core.snapshot()?;
        assert_eq!(snapshot.active_tab_id, active_tab_id);
        assert_eq!(snapshot.command_query, "@tabs absent");
        Ok(())
    }

    #[test]
    fn search_command_opens_default_search_url() -> Result<(), Box<dyn Error>> {
        let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

        core.set_command_query("? rust async book");
        let intent = core.submit_command()?;

        let active_tab = core.active_tab()?;
        assert_eq!(intent, Some(CommandIntent::Search("rust async book".to_string())));
        assert_eq!(active_tab.url().as_str(), "https://duckduckgo.com/?q=rust+async+book");
        assert_eq!(core.command_query(), "");
        Ok(())
    }

    #[test]
    fn toggles_active_tab_favorite() -> Result<(), Box<dyn Error>> {
        let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

        let favorite = core.toggle_active_tab_favorite()?;
        let snapshot = core.snapshot()?;

        assert!(favorite);
        assert_eq!(snapshot.favorites.len(), 1);
        assert_eq!(snapshot.favorites[0].id(), &snapshot.active_tab_id);

        let favorite = core.toggle_active_tab_favorite()?;
        let snapshot = core.snapshot()?;

        assert!(!favorite);
        assert!(snapshot.favorites.is_empty());
        Ok(())
    }

    #[test]
    fn enforces_default_favorite_limit() -> Result<(), Box<dyn Error>> {
        let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

        core.toggle_active_tab_favorite()?;
        for index in 1..12 {
            core.open_tab(UrlText::parse(format!("https://example.com/{index}"))?);
            core.toggle_active_tab_favorite()?;
        }

        core.open_tab(UrlText::parse("https://example.com/overflow")?);
        let error = match core.toggle_active_tab_favorite() {
            Err(error) => error,
            Ok(_) => return Err("favorite limit should apply".into()),
        };

        assert_eq!(error, CoreError::FavoriteLimitReached { limit: 12 });
        assert_eq!(core.snapshot()?.favorites.len(), 12);
        Ok(())
    }
}

use ely_domain::{
    BrowserTab, CommandIntent, DomainError, Profile, ProfileId, ProfileKind, Space, SpaceId, TabId,
    UrlText,
};

use crate::CoreError;

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
}

impl BrowserCore {
    pub fn new(config: InitialBrowserConfig) -> Result<Self, CoreError> {
        let space = Space::new(config.space_name, config.space_icon, 0xf54e00);
        let profile = Profile::new(config.profile_name, 0x26251e, ProfileKind::Standard);
        let tab = BrowserTab::new(
            TabId::new(),
            space.id().clone(),
            profile.id().clone(),
            "New Tab",
            config.initial_url,
        );

        Ok(Self {
            active_space_id: space.id().clone(),
            active_profile_id: profile.id().clone(),
            active_tab_id: tab.id().clone(),
            spaces: vec![space],
            profiles: vec![profile],
            tabs: vec![tab],
            command_query: String::new(),
        })
    }

    pub fn open_tab(&mut self, url: UrlText) -> TabId {
        let title = tab_title(&url);
        let tab = BrowserTab::new(
            TabId::new(),
            self.active_space_id.clone(),
            self.active_profile_id.clone(),
            title,
            url,
        );
        let tab_id = tab.id().clone();
        self.tabs.push(tab);
        self.active_tab_id = tab_id.clone();
        tab_id
    }

    pub fn select_tab(&mut self, tab_id: &TabId) -> Result<(), CoreError> {
        if self.tabs.iter().any(|tab| tab.id() == tab_id) {
            self.active_tab_id = tab_id.clone();
            return Ok(());
        }

        Err(CoreError::TabNotFound { id: tab_id.clone() })
    }

    pub fn set_command_query(&mut self, query: impl Into<String>) {
        self.command_query = query.into();
    }

    pub fn submit_command(&mut self) -> Result<Option<CommandIntent>, CoreError> {
        let query = self.command_query.trim();
        if query.is_empty() {
            return Ok(None);
        }

        let intent = CommandIntent::parse(query)?;
        if let CommandIntent::Navigate(url) = &intent {
            self.open_tab(url.clone());
            self.command_query.clear();
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
}

fn tab_title(url: &UrlText) -> String {
    if url.as_str() == "ely://new-tab" {
        return "New Tab".to_string();
    }

    url.display_host()
}

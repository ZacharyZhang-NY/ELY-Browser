use ely_domain::{
    AppearanceSettings, ArchivePolicy, DEFAULT_SIDEBAR_WIDTH_PX, FavoriteLimit, NewTabDestination,
    SearchEngine, ThemeMode, WallpaperTheme,
};

use super::BrowserCore;
use crate::CoreError;

impl BrowserCore {
    pub fn set_search_engine(&mut self, search_engine: SearchEngine) {
        self.search_engine = search_engine;
    }

    pub fn reset_search_settings(&mut self) {
        self.set_search_engine(SearchEngine::default());
    }

    #[must_use]
    pub fn search_engine(&self) -> SearchEngine {
        self.search_engine
    }

    pub fn set_new_tab_destination(&mut self, destination: NewTabDestination) {
        self.new_tab_destination = destination;
    }

    pub fn reset_general_settings(&mut self) {
        self.set_new_tab_destination(NewTabDestination::default());
    }

    #[must_use]
    pub fn new_tab_destination(&self) -> NewTabDestination {
        self.new_tab_destination
    }

    pub fn set_favorite_limit(&mut self, favorite_limit: FavoriteLimit) {
        self.favorite_limit = favorite_limit;
    }

    pub fn reset_sidebar_tabs_settings(&mut self) -> Result<(), CoreError> {
        let active_space_id = self.active_space_id.clone();
        self.set_space_archive_policy(&active_space_id, ArchivePolicy::Manual)?;
        self.set_space_sidebar_width(&active_space_id, DEFAULT_SIDEBAR_WIDTH_PX)?;
        self.set_favorite_limit(FavoriteLimit::default());
        Ok(())
    }

    #[must_use]
    pub fn favorite_limit(&self) -> FavoriteLimit {
        self.favorite_limit
    }

    #[must_use]
    pub fn appearance(&self) -> AppearanceSettings {
        self.appearance
    }

    pub fn set_wallpaper_theme(&mut self, wallpaper: WallpaperTheme) {
        self.appearance.set_wallpaper(wallpaper);
    }

    pub fn set_theme_mode(&mut self, theme_mode: ThemeMode) {
        self.appearance.set_theme_mode(theme_mode);
    }

    pub fn set_reduce_motion(&mut self, reduce_motion: bool) {
        self.appearance.set_reduce_motion(reduce_motion);
    }

    pub fn set_translucency_pct(&mut self, value: u8) {
        self.appearance.set_translucency_pct(value);
    }

    pub fn reset_appearance(&mut self) {
        self.appearance = AppearanceSettings::default();
    }

    pub fn set_command_query(&mut self, query: impl Into<String>) {
        self.command_query = query.into();
    }

    #[must_use]
    pub fn command_query(&self) -> &str {
        &self.command_query
    }
}

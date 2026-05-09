use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WallpaperTheme {
    #[default]
    Dawn,
    Violet,
    Mint,
    Slate,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AppearanceSettings {
    wallpaper: WallpaperTheme,
    theme_mode: ThemeMode,
    reduce_motion: bool,
}

impl AppearanceSettings {
    pub fn wallpaper(&self) -> WallpaperTheme {
        self.wallpaper
    }

    pub fn theme_mode(&self) -> ThemeMode {
        self.theme_mode
    }

    pub fn reduce_motion(&self) -> bool {
        self.reduce_motion
    }

    pub fn set_wallpaper(&mut self, wallpaper: WallpaperTheme) {
        self.wallpaper = wallpaper;
    }

    pub fn set_theme_mode(&mut self, theme_mode: ThemeMode) {
        self.theme_mode = theme_mode;
    }

    pub fn set_reduce_motion(&mut self, reduce_motion: bool) {
        self.reduce_motion = reduce_motion;
    }
}

#[cfg(test)]
mod tests {
    use super::{AppearanceSettings, ThemeMode, WallpaperTheme};

    #[test]
    fn default_settings_use_dawn_and_system_mode() {
        let settings = AppearanceSettings::default();
        assert_eq!(settings.wallpaper(), WallpaperTheme::Dawn);
        assert_eq!(settings.theme_mode(), ThemeMode::System);
        assert!(!settings.reduce_motion());
    }

    #[test]
    fn setters_update_each_field_independently() {
        let mut settings = AppearanceSettings::default();

        settings.set_wallpaper(WallpaperTheme::Violet);
        settings.set_theme_mode(ThemeMode::Dark);
        settings.set_reduce_motion(true);

        assert_eq!(settings.wallpaper(), WallpaperTheme::Violet);
        assert_eq!(settings.theme_mode(), ThemeMode::Dark);
        assert!(settings.reduce_motion());
    }

    #[test]
    fn json_round_trip_preserves_kebab_case_variants() {
        let settings = AppearanceSettings {
            wallpaper: WallpaperTheme::Mint,
            theme_mode: ThemeMode::Light,
            reduce_motion: true,
        };

        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("\"wallpaper\":\"mint\""));
        assert!(json.contains("\"theme_mode\":\"light\""));

        let restored: AppearanceSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, settings);
    }
}

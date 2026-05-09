use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{ThemeMode, WallpaperTheme};

#[test]
fn snapshot_starts_with_default_appearance() -> Result<(), Box<dyn Error>> {
    let core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let snapshot = core.snapshot()?;

    assert_eq!(snapshot.appearance.wallpaper(), WallpaperTheme::Dawn);
    assert_eq!(snapshot.appearance.theme_mode(), ThemeMode::System);
    assert!(!snapshot.appearance.reduce_motion());
    Ok(())
}

#[test]
fn setters_persist_into_subsequent_snapshots() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_wallpaper_theme(WallpaperTheme::Mint);
    core.set_theme_mode(ThemeMode::Dark);
    core.set_reduce_motion(true);

    let snapshot = core.snapshot()?;
    assert_eq!(snapshot.appearance.wallpaper(), WallpaperTheme::Mint);
    assert_eq!(snapshot.appearance.theme_mode(), ThemeMode::Dark);
    assert!(snapshot.appearance.reduce_motion());
    Ok(())
}

#[test]
fn reset_appearance_restores_defaults() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_wallpaper_theme(WallpaperTheme::Slate);
    core.set_theme_mode(ThemeMode::Light);
    core.set_reduce_motion(true);
    core.reset_appearance();

    let snapshot = core.snapshot()?;
    assert_eq!(snapshot.appearance.wallpaper(), WallpaperTheme::Dawn);
    assert_eq!(snapshot.appearance.theme_mode(), ThemeMode::System);
    assert!(!snapshot.appearance.reduce_motion());
    Ok(())
}

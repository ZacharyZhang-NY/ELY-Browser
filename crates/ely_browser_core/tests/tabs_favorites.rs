use std::error::Error;

use ely_browser_core::{BrowserCore, CoreError, InitialBrowserConfig};
use ely_domain::{FavoriteLimit, UrlText};

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
fn toggles_active_tab_pinned() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    let pinned = core.toggle_active_tab_pinned()?;
    let snapshot = core.snapshot()?;

    assert!(pinned);
    assert_eq!(snapshot.pinned_tabs.len(), 1);
    assert_eq!(snapshot.pinned_tabs[0].id(), &snapshot.active_tab_id);

    let pinned = core.toggle_active_tab_pinned()?;
    let snapshot = core.snapshot()?;

    assert!(!pinned);
    assert!(snapshot.pinned_tabs.is_empty());
    Ok(())
}

#[test]
fn favorite_tabs_are_omitted_from_pinned_section() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.toggle_active_tab_pinned()?;
    core.toggle_active_tab_favorite()?;

    let snapshot = core.snapshot()?;
    assert_eq!(snapshot.favorites.len(), 1);
    assert!(snapshot.pinned_tabs.is_empty());
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

#[test]
fn enforces_configured_favorite_limit() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.set_favorite_limit(FavoriteLimit::Six);

    core.toggle_active_tab_favorite()?;
    for index in 1..6 {
        core.open_tab(UrlText::parse(format!("https://example.com/{index}"))?);
        core.toggle_active_tab_favorite()?;
    }

    core.open_tab(UrlText::parse("https://example.com/overflow")?);
    let error = match core.toggle_active_tab_favorite() {
        Err(error) => error,
        Ok(_) => return Err("configured favorite limit should apply".into()),
    };

    let snapshot = core.snapshot()?;
    assert_eq!(error, CoreError::FavoriteLimitReached { limit: 6 });
    assert_eq!(snapshot.favorite_limit, FavoriteLimit::Six);
    assert_eq!(snapshot.favorites.len(), 6);
    Ok(())
}

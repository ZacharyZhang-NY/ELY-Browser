use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::UrlText;

#[test]
fn active_tab_tracks_back_and_forward_navigation() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.navigate_active_tab(UrlText::parse("https://example.com/a")?)?;
    core.navigate_active_tab(UrlText::parse("https://example.com/b")?)?;
    assert!(core.active_tab()?.can_navigate_back());
    assert!(!core.active_tab()?.can_navigate_forward());

    assert!(core.navigate_active_tab_back()?);
    assert_eq!(core.active_tab()?.url().as_str(), "https://example.com/a");
    assert!(core.active_tab()?.can_navigate_back());
    assert!(core.active_tab()?.can_navigate_forward());

    assert!(core.navigate_active_tab_forward()?);
    assert_eq!(core.active_tab()?.url().as_str(), "https://example.com/b");
    assert!(core.active_tab()?.can_navigate_back());
    assert!(!core.active_tab()?.can_navigate_forward());
    Ok(())
}

#[test]
fn new_navigation_clears_forward_stack() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.navigate_active_tab(UrlText::parse("https://example.com/a")?)?;
    core.navigate_active_tab(UrlText::parse("https://example.com/b")?)?;
    assert!(core.navigate_active_tab_back()?);

    core.navigate_active_tab(UrlText::parse("https://example.com/c")?)?;

    assert_eq!(core.active_tab()?.url().as_str(), "https://example.com/c");
    assert!(core.active_tab()?.can_navigate_back());
    assert!(!core.active_tab()?.can_navigate_forward());
    Ok(())
}

#[test]
fn observed_loaded_url_replaces_active_url_without_back_stack() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.active_tab()?.id().clone();

    assert!(core.replace_tab_loaded_url(&tab_id, UrlText::parse("https://example.com/")?)?);

    assert_eq!(core.active_tab()?.url().as_str(), "https://example.com/");
    assert!(!core.active_tab()?.can_navigate_back());
    assert!(!core.active_tab()?.can_navigate_forward());
    Ok(())
}

#[test]
fn navigation_replaces_new_tab_metadata_with_url_metadata() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.navigate_active_tab(UrlText::parse("https://example.com/research")?)?;

    let active_tab = core.active_tab()?;
    assert_eq!(active_tab.title(), "example.com");
    assert_eq!(active_tab.favicon_key(), Some("ely-favicon://example.com"));
    Ok(())
}

#[test]
fn internal_navigation_clears_previous_web_favicon() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.navigate_active_tab(UrlText::parse("https://example.com/research")?)?;
    core.navigate_active_tab(UrlText::parse("ely://history")?)?;

    let active_tab = core.active_tab()?;
    assert_eq!(active_tab.title(), "History");
    assert_eq!(active_tab.favicon_key(), None);
    Ok(())
}

#[test]
fn history_navigation_refreshes_url_metadata() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.navigate_active_tab(UrlText::parse("https://example.com/a")?)?;
    let tab_id = core.active_tab()?.id().clone();
    core.set_tab_title(&tab_id, "Live Example")?;
    core.navigate_active_tab(UrlText::parse("https://servo.org/b")?)?;

    assert!(core.navigate_active_tab_back()?);

    let active_tab = core.active_tab()?;
    assert_eq!(active_tab.url().as_str(), "https://example.com/a");
    assert_eq!(active_tab.title(), "example.com");
    assert_eq!(active_tab.favicon_key(), Some("ely-favicon://example.com"));
    Ok(())
}

#[test]
fn user_loaded_url_enters_active_back_stack() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.navigate_active_tab(UrlText::parse("https://example.com/a")?)?;
    let tab_id = core.active_tab()?.id().clone();

    assert!(core.navigate_tab_to_loaded_url(&tab_id, UrlText::parse("https://example.com/b")?)?);

    assert_eq!(core.active_tab()?.url().as_str(), "https://example.com/b");
    assert!(core.active_tab()?.can_navigate_back());
    assert!(core.navigate_active_tab_back()?);
    assert_eq!(core.active_tab()?.url().as_str(), "https://example.com/a");
    Ok(())
}

#[test]
fn empty_history_navigation_keeps_active_url() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let url = core.active_tab()?.url().clone();

    assert!(!core.navigate_active_tab_back()?);
    assert!(!core.navigate_active_tab_forward()?);

    assert_eq!(core.active_tab()?.url(), &url);
    Ok(())
}

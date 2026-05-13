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
fn empty_history_navigation_keeps_active_url() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let url = core.active_tab()?.url().clone();

    assert!(!core.navigate_active_tab_back()?);
    assert!(!core.navigate_active_tab_forward()?);

    assert_eq!(core.active_tab()?.url(), &url);
    Ok(())
}

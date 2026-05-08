use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{CommandIntent, TabState, UrlText};

#[test]
fn discard_active_tab_preserves_tab_metadata() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.open_tab(UrlText::parse("https://example.com/sleep")?);
    let title = core.active_tab()?.title().to_string();
    core.set_tab_favicon_key(&tab_id, "favicons/example.ico")?;

    core.discard_active_tab()?;
    let active_tab = core.active_tab()?;

    assert_eq!(active_tab.id(), &tab_id);
    assert_eq!(active_tab.state(), &TabState::Discarded);
    assert_eq!(active_tab.url().as_str(), "https://example.com/sleep");
    assert_eq!(active_tab.title(), title);
    assert_eq!(active_tab.favicon_key(), Some("favicons/example.ico"));
    Ok(())
}

#[test]
fn wake_discarded_tab_marks_tab_ready() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.open_tab(UrlText::parse("https://example.com/wake")?);

    core.discard_active_tab()?;
    core.wake_discarded_tab(&tab_id)?;
    let active_tab = core.active_tab()?;

    assert_eq!(active_tab.id(), &tab_id);
    assert_eq!(active_tab.state(), &TabState::Ready);
    assert_eq!(active_tab.url().as_str(), "https://example.com/wake");
    Ok(())
}

#[test]
fn sleep_tab_command_discards_active_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.open_tab(UrlText::parse("https://example.com/sleep-command")?);

    core.set_command_query(">sleep-tab");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("sleep-tab".to_string())));
    assert_eq!(snapshot.active_tab_id, tab_id);
    assert_eq!(active_tab.state(), &TabState::Discarded);
    assert_eq!(active_tab.url().as_str(), "https://example.com/sleep-command");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn wake_tab_command_restores_active_discarded_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.open_tab(UrlText::parse("https://example.com/wake-command")?);
    core.discard_active_tab()?;

    core.set_command_query(">wake-tab");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("wake-tab".to_string())));
    assert_eq!(snapshot.active_tab_id, tab_id);
    assert_eq!(active_tab.state(), &TabState::Ready);
    assert_eq!(active_tab.url().as_str(), "https://example.com/wake-command");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

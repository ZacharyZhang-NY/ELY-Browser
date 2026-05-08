use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{CommandIntent, TabState, UrlText};

#[test]
fn crash_active_tab_preserves_tab_metadata() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.open_tab(UrlText::parse("https://example.com/form")?);
    let title = core.active_tab()?.title().to_string();
    core.set_tab_favicon_key(&tab_id, "favicons/example.ico")?;

    core.crash_active_tab()?;
    let active_tab = core.active_tab()?;

    assert_eq!(active_tab.id(), &tab_id);
    assert_eq!(active_tab.state(), &TabState::Crashed);
    assert_eq!(active_tab.url().as_str(), "https://example.com/form");
    assert_eq!(active_tab.title(), title);
    assert_eq!(active_tab.favicon_key(), Some("favicons/example.ico"));
    Ok(())
}

#[test]
fn recover_crashed_tab_marks_tab_ready() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.open_tab(UrlText::parse("https://example.com/recover")?);

    core.crash_active_tab()?;
    core.recover_crashed_tab(&tab_id)?;
    let active_tab = core.active_tab()?;

    assert_eq!(active_tab.id(), &tab_id);
    assert_eq!(active_tab.state(), &TabState::Ready);
    assert_eq!(active_tab.url().as_str(), "https://example.com/recover");
    Ok(())
}

#[test]
fn crash_tab_command_marks_active_tab_crashed() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.open_tab(UrlText::parse("https://example.com/crash-loop")?);

    core.set_command_query(">crash-tab");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("crash-tab".to_string())));
    assert_eq!(snapshot.active_tab_id, tab_id);
    assert_eq!(active_tab.state(), &TabState::Crashed);
    assert_eq!(active_tab.url().as_str(), "https://example.com/crash-loop");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn recover_tab_command_restores_active_crashed_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.open_tab(UrlText::parse("https://example.com/recover-command")?);
    core.crash_active_tab()?;

    core.set_command_query(">recover-tab");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("recover-tab".to_string())));
    assert_eq!(snapshot.active_tab_id, tab_id);
    assert_eq!(active_tab.state(), &TabState::Ready);
    assert_eq!(active_tab.url().as_str(), "https://example.com/recover-command");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn crash_route_title_marks_internal_recovery_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.active_tab()?.id().clone();
    let recovery_url = UrlText::parse(format!("ely://crash/{}", tab_id.as_str()))?;

    core.open_tab(recovery_url);
    let active_tab = core.active_tab()?;

    assert_eq!(active_tab.title(), "Tab Recovery");
    assert_eq!(active_tab.url().as_str(), format!("ely://crash/{}", tab_id.as_str()));
    Ok(())
}

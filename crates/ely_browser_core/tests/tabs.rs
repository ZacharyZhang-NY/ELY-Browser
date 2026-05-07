use std::error::Error;

use ely_browser_core::{BrowserCore, CoreError, InitialBrowserConfig};
use ely_domain::{CommandIntent, CommandScope, UrlText};

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

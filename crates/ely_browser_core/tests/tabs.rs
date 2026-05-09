use std::error::Error;

use ely_browser_core::{BrowserCore, CoreError, InitialBrowserConfig};
use ely_domain::{
    CommandIntent, CommandScope, DEFAULT_ZOOM_PERCENT, DomainError, MAX_ZOOM_PERCENT,
    MIN_ZOOM_PERCENT, NewTabDestination, SearchEngine, TabState, UrlText,
};

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
fn opened_tabs_receive_visible_sort_keys() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let first_tab_id = core.active_tab()?.id().clone();
    let second_tab_id = core.open_tab(UrlText::parse("https://example.com")?);

    core.select_tab(&first_tab_id)?;
    let third_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);
    let snapshot = core.snapshot()?;
    let ordered =
        snapshot.tabs.iter().map(|tab| (tab.id().clone(), tab.sort_key())).collect::<Vec<_>>();

    assert_eq!(ordered, vec![(first_tab_id, 0), (third_tab_id, 1), (second_tab_id, 2)]);
    Ok(())
}

#[test]
fn snapshot_orders_tabs_by_sort_key() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let first_tab_id = core.active_tab()?.id().clone();
    let second_tab_id = core.open_tab(UrlText::parse("https://example.com")?);
    let third_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);

    core.set_tab_sort_key(&second_tab_id, 30)?;
    core.set_tab_sort_key(&third_tab_id, 10)?;
    core.set_tab_sort_key(&first_tab_id, 20)?;
    let snapshot = core.snapshot()?;
    let ordered_ids = snapshot.tabs.iter().map(|tab| tab.id().clone()).collect::<Vec<_>>();

    assert_eq!(ordered_ids, vec![third_tab_id, first_tab_id, second_tab_id]);
    Ok(())
}

#[test]
fn opened_tabs_record_active_tab_as_parent() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let first_tab_id = core.active_tab()?.id().clone();

    let second_tab_id = core.open_tab(UrlText::parse("https://example.com")?);
    let snapshot = core.snapshot()?;

    let opened_tab = snapshot
        .tabs
        .iter()
        .find(|tab| tab.id() == &second_tab_id)
        .ok_or(CoreError::MissingActiveTab)?;
    assert_eq!(opened_tab.parent_tab_id(), Some(&first_tab_id));
    Ok(())
}

#[test]
fn auth_callback_tabs_receive_internal_title() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    let tab_id = core.open_tab(UrlText::parse("ely://auth/callback?code=abc")?);
    let snapshot = core.snapshot()?;
    let tab =
        snapshot.tabs.iter().find(|tab| tab.id() == &tab_id).ok_or(CoreError::MissingActiveTab)?;

    assert_eq!(tab.title(), "Auth Callback");
    Ok(())
}

#[test]
fn replacement_tabs_have_no_parent_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();

    let replacement_tab_id = core.close_tab(&active_tab_id)?;
    let snapshot = core.snapshot()?;

    let replacement_tab = snapshot
        .tabs
        .iter()
        .find(|tab| tab.id() == &replacement_tab_id)
        .ok_or(CoreError::MissingActiveTab)?;
    assert_eq!(replacement_tab.parent_tab_id(), None);
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
    assert_eq!(snapshot.archived_tabs.len(), 1);
    assert_eq!(snapshot.archived_tabs[0].tab().id(), &second_tab_id);
    assert_eq!(snapshot.archived_tabs[0].tab().state(), &TabState::Archived);
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
    assert_eq!(snapshot.archived_tabs.len(), 1);
    let replacement_tab = snapshot.tabs.first().ok_or(CoreError::MissingActiveTab)?;
    assert_eq!(replacement_tab.url().as_str(), "ely://new-tab");
    Ok(())
}

#[test]
fn restores_last_archived_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let closed_tab_id = core.open_tab(UrlText::parse("https://example.com")?);
    core.close_active_tab()?;

    let restored_tab_id = core.restore_last_archived_tab()?;
    let snapshot = core.snapshot()?;

    assert_eq!(restored_tab_id, closed_tab_id);
    assert_eq!(snapshot.active_tab_id, closed_tab_id);
    assert!(snapshot.archived_tabs.is_empty());
    let restored_tab = snapshot
        .tabs
        .iter()
        .find(|tab| tab.id() == &closed_tab_id)
        .ok_or(CoreError::MissingActiveTab)?;
    assert_eq!(restored_tab.state(), &TabState::Ready);
    Ok(())
}

#[test]
fn restores_archived_tab_by_id() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let example_tab_id = core.open_tab(UrlText::parse("https://example.com")?);
    let servo_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);

    core.close_active_tab()?;
    core.select_tab(&example_tab_id)?;
    core.close_active_tab()?;

    let restored_tab_id = core.restore_archived_tab(&servo_tab_id)?;
    let snapshot = core.snapshot()?;

    assert_eq!(restored_tab_id, servo_tab_id);
    assert_eq!(snapshot.active_tab_id, servo_tab_id);
    assert_eq!(snapshot.archived_tabs.len(), 1);
    assert_eq!(snapshot.archived_tabs[0].tab().id(), &example_tab_id);
    Ok(())
}

#[test]
fn restore_archived_tab_by_id_returns_error_for_open_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();

    let error = match core.restore_archived_tab(&active_tab_id) {
        Err(error) => error,
        Ok(_) => return Err("restore should require an archived tab id".into()),
    };

    assert_eq!(error, CoreError::TabNotFound { id: active_tab_id });
    Ok(())
}

#[test]
fn restores_matching_archived_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let example_tab_id = core.open_tab(UrlText::parse("https://example.com")?);
    let servo_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);

    core.close_active_tab()?;
    core.select_tab(&example_tab_id)?;
    core.close_active_tab()?;

    let restored_tab_id = core.restore_archived_tab_match("servo")?;
    let snapshot = core.snapshot()?;

    assert_eq!(restored_tab_id, Some(servo_tab_id.clone()));
    assert_eq!(snapshot.active_tab_id, servo_tab_id);
    assert_eq!(snapshot.archived_tabs.len(), 1);
    assert_eq!(snapshot.archived_tabs[0].tab().id(), &example_tab_id);
    Ok(())
}

#[test]
fn ignores_empty_archived_tab_match_query() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com")?);
    core.close_active_tab()?;
    let active_tab_id = core.active_tab()?.id().clone();

    let restored_tab_id = core.restore_archived_tab_match(" ")?;
    let snapshot = core.snapshot()?;

    assert_eq!(restored_tab_id, None);
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert_eq!(snapshot.archived_tabs.len(), 1);
    Ok(())
}

#[test]
fn restore_without_archived_tabs_returns_error() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    let error = match core.restore_last_archived_tab() {
        Err(error) => error,
        Ok(_) => return Err("restore should require an archived tab".into()),
    };

    assert_eq!(error, CoreError::NoArchivedTabs);
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
fn search_command_uses_selected_search_engine() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.set_search_engine(SearchEngine::Google);

    core.set_command_query("? rust async book");
    let intent = core.submit_command()?;

    let active_tab = core.active_tab()?;
    assert_eq!(intent, Some(CommandIntent::Search("rust async book".to_string())));
    assert_eq!(active_tab.url().as_str(), "https://www.google.com/search?q=rust+async+book");
    assert_eq!(core.snapshot()?.search_engine, SearchEngine::Google);
    assert_eq!(core.command_query(), "");
    Ok(())
}

#[test]
fn new_tab_command_uses_selected_destination() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.set_new_tab_destination(NewTabDestination::Bookmarks);

    core.set_command_query(">new-tab");
    let intent = core.submit_command()?;

    let active_tab = core.active_tab()?;
    assert_eq!(intent, Some(CommandIntent::Command("new-tab".to_string())));
    assert_eq!(active_tab.title(), "Bookmarks");
    assert_eq!(active_tab.url().as_str(), "ely://bookmarks");
    assert_eq!(core.snapshot()?.new_tab_destination, NewTabDestination::Bookmarks);
    assert_eq!(core.command_query(), "");
    Ok(())
}

#[test]
fn active_tab_zoom_updates_tab_state() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    assert_eq!(core.active_tab()?.zoom_percent(), DEFAULT_ZOOM_PERCENT);
    assert_eq!(core.zoom_active_tab_in()?, DEFAULT_ZOOM_PERCENT + 10);
    assert_eq!(core.zoom_active_tab_out()?, DEFAULT_ZOOM_PERCENT);
    assert_eq!(core.set_active_tab_zoom_percent(125)?, 125);
    assert_eq!(core.active_tab()?.zoom_factor(), 1.25);
    assert_eq!(core.reset_active_tab_zoom()?, DEFAULT_ZOOM_PERCENT);
    Ok(())
}

#[test]
fn active_tab_zoom_clamps_incremental_commands() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_active_tab_zoom_percent(MAX_ZOOM_PERCENT)?;
    assert_eq!(core.zoom_active_tab_in()?, MAX_ZOOM_PERCENT);

    core.set_active_tab_zoom_percent(MIN_ZOOM_PERCENT)?;
    assert_eq!(core.zoom_active_tab_out()?, MIN_ZOOM_PERCENT);
    Ok(())
}

#[test]
fn active_tab_zoom_rejects_out_of_range_percent() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    let error = match core.set_active_tab_zoom_percent(5) {
        Err(error) => error,
        Ok(_) => return Err("zoom should require an in-range percent".into()),
    };

    assert_eq!(
        error,
        CoreError::Domain(DomainError::InvalidZoomPercent {
            value: 5,
            min: MIN_ZOOM_PERCENT,
            max: MAX_ZOOM_PERCENT,
        })
    );
    Ok(())
}

#[test]
fn zoom_commands_update_active_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">zoom-in");
    let intent = core.submit_command()?;
    assert_eq!(intent, Some(CommandIntent::Command("zoom-in".to_string())));
    assert_eq!(core.active_tab()?.zoom_percent(), DEFAULT_ZOOM_PERCENT + 10);

    core.set_command_query(">zoom 125%");
    core.submit_command()?;
    assert_eq!(core.active_tab()?.zoom_percent(), 125);

    core.set_command_query(">actual-size");
    core.submit_command()?;
    assert_eq!(core.active_tab()?.zoom_percent(), DEFAULT_ZOOM_PERCENT);
    assert_eq!(core.command_query(), "");
    Ok(())
}

#[test]
fn switching_spaces_restores_each_space_active_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_snapshot = core.snapshot()?;
    let work_space_id = work_snapshot.active_space_id;
    let work_tab_id = work_snapshot.active_tab_id;

    let research_space_id = core.create_space("Research", "R", 0xf54e00)?;
    let research_new_tab_id = core.active_tab()?.id().clone();
    let servo_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);

    core.select_space(&work_space_id)?;
    let work_snapshot = core.snapshot()?;

    assert_eq!(work_snapshot.active_tab_id, work_tab_id);
    assert_eq!(work_snapshot.active_space_id, work_space_id);
    assert_eq!(work_snapshot.tabs.len(), 1);
    assert_eq!(work_snapshot.tabs[0].id(), &work_tab_id);

    core.select_space(&research_space_id)?;
    let research_snapshot = core.snapshot()?;
    let research_ids =
        research_snapshot.tabs.iter().map(|tab| tab.id().clone()).collect::<Vec<_>>();

    assert_eq!(research_snapshot.active_tab_id, servo_tab_id);
    assert_eq!(research_snapshot.active_space_id, research_space_id);
    assert_eq!(research_ids, vec![research_new_tab_id, servo_tab_id]);
    Ok(())
}

#[test]
fn closing_active_tab_selects_neighbor_in_same_space() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_tab_id = core.active_tab()?.id().clone();
    core.create_space("Research", "R", 0xf54e00)?;
    let research_tab_id = core.active_tab()?.id().clone();
    let servo_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);

    let active_tab_id = core.close_active_tab()?;
    let snapshot = core.snapshot()?;

    assert_eq!(active_tab_id, research_tab_id);
    assert_eq!(snapshot.active_tab_id, research_tab_id);
    assert_eq!(snapshot.tabs.len(), 1);
    assert_eq!(snapshot.tabs[0].id(), &research_tab_id);
    assert!(snapshot.tabs.iter().all(|tab| tab.id() != &work_tab_id));
    assert_eq!(snapshot.archived_tabs[0].tab().id(), &servo_tab_id);
    Ok(())
}

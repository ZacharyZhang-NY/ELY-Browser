use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{CommandIntent, CommandScope, ProfileKind, UrlText};

#[test]
fn archive_scoped_search_restores_tab_by_space_name() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let research_space_id = core.create_space("Research", "R", 0xf54e00)?;
    let archived_tab_id = core.open_tab(UrlText::parse("https://example.com/context")?);
    core.close_active_tab()?;
    core.select_space(&work_space_id)?;

    core.set_command_query("@archive Research");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Archive,
            query: "Research".to_string(),
        })
    );
    assert_eq!(snapshot.active_tab_id, archived_tab_id);
    assert_eq!(snapshot.active_space_id, research_space_id);
    assert!(snapshot.archived_tabs.is_empty());
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn archive_scoped_search_restores_tab_by_profile_name() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.snapshot()?.active_profile_id;
    let client_profile_id = core.create_profile("Client", 0xf54e00, ProfileKind::Standard)?;
    let archived_tab_id = core.open_tab(UrlText::parse("https://example.com/account")?);
    core.close_active_tab()?;
    core.select_profile(&default_profile_id)?;

    core.set_command_query("@archive Client");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Archive,
            query: "Client".to_string(),
        })
    );
    assert_eq!(snapshot.active_tab_id, archived_tab_id);
    assert_eq!(snapshot.active_profile_id, client_profile_id);
    assert!(snapshot.archived_tabs.is_empty());
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

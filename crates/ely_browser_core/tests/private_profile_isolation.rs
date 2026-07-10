use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::UrlText;

/// A private profile can be created inside the main window (`>new-private-profile`).
/// Its tabs and favorites must never appear once a standard profile is active —
/// the sidebar is scoped to the active profile like every other surface
/// (bookmarks, history, downloads, permissions). PRD §8.11.
#[test]
fn private_profile_tabs_do_not_leak_into_a_standard_profile() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.active_tab()?.profile_id().clone();
    core.open_tab(UrlText::parse("https://example.com/work")?);

    core.set_command_query(">new-private-profile Private");
    core.submit_command()?;
    let private_tab = core.open_tab(UrlText::parse("https://example.com/secret")?);
    core.toggle_active_tab_favorite()?;

    core.select_profile(&default_profile_id)?;
    let snapshot = core.snapshot()?;

    assert!(
        snapshot.tabs.iter().all(|tab| tab.id() != &private_tab),
        "private tab must not appear in the standard profile sidebar",
    );
    assert!(
        snapshot.tabs.iter().all(|tab| tab.profile_id() == &default_profile_id),
        "every visible tab must belong to the active profile",
    );
    assert!(
        snapshot.favorites.iter().all(|tab| tab.profile_id() == &default_profile_id),
        "private favorites must not appear in the standard profile",
    );
    assert!(
        snapshot.tabs.iter().any(|tab| tab.url().as_str() == "https://example.com/work"),
        "the standard profile's own tab must still be visible",
    );
    Ok(())
}

/// Switching back into the private profile still shows its own tabs — the fix
/// scopes visibility, it does not destroy the private session mid-window.
#[test]
fn private_profile_still_sees_its_own_tabs_when_active() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.active_tab()?.profile_id().clone();

    core.set_command_query(">new-private-profile Private");
    core.submit_command()?;
    let private_profile_id = core.active_tab()?.profile_id().clone();
    let private_tab = core.open_tab(UrlText::parse("https://example.com/secret")?);
    assert_ne!(private_profile_id, default_profile_id);

    core.select_profile(&default_profile_id)?;
    core.select_profile(&private_profile_id)?;
    let snapshot = core.snapshot()?;

    assert!(
        snapshot.tabs.iter().any(|tab| tab.id() == &private_tab),
        "the private profile must still see its own tab when active",
    );
    Ok(())
}

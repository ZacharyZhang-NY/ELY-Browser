use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{CommandIntent, ProfileKind, UrlText};

#[test]
fn new_private_profile_command_creates_private_profile() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">new-private-profile Private");
    let intent = core.submit_command()?;

    assert_eq!(intent, Some(CommandIntent::Command("new-private-profile Private".to_string())),);
    let snapshot = core.snapshot()?;
    let Some(profile) = snapshot.profiles.iter().find(|profile| profile.name() == "Private") else {
        return Err("missing private profile".into());
    };

    assert_eq!(profile.kind(), &ProfileKind::Private);
    assert_eq!(&snapshot.active_profile_id, profile.id());
    assert_eq!(snapshot.active_profile_name, "Private");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn private_profiles_do_not_record_history() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.snapshot()?.active_profile_id;
    let private_profile_id = core.create_profile("Private", 0x807d72, ProfileKind::Private)?;

    core.open_tab(UrlText::parse("https://example.com/private")?);
    let private_snapshot = core.snapshot()?;
    assert_eq!(private_snapshot.active_profile_id, private_profile_id);
    assert!(private_snapshot.history_entries.is_empty());

    core.select_profile(&default_profile_id)?;
    core.open_tab(UrlText::parse("https://example.com/standard")?);
    let default_snapshot = core.snapshot()?;
    assert_eq!(default_snapshot.active_profile_id, default_profile_id);
    assert_eq!(default_snapshot.history_entries.len(), 1);

    core.select_profile(&private_profile_id)?;
    let private_snapshot = core.snapshot()?;
    assert!(private_snapshot.history_entries.is_empty());
    Ok(())
}

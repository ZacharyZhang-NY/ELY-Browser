use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::CommandIntent;

#[test]
fn space_accent_command_updates_the_active_space() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">space-accent #123ABC");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_space = snapshot
        .spaces
        .iter()
        .find(|space| space.id() == &snapshot.active_space_id)
        .ok_or("missing active space")?;

    assert_eq!(intent, Some(CommandIntent::Command("space-accent #123ABC".to_string())));
    assert_eq!(snapshot.command_query, "");
    assert_eq!(active_space.accent_hex(), 0x123abc);
    Ok(())
}

#[test]
fn space_accent_command_preserves_query_for_invalid_hex() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let original_accent = core.snapshot()?.spaces[0].accent_hex();

    core.set_command_query(">space-accent #12");
    core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(snapshot.command_query, ">space-accent #12");
    assert_eq!(snapshot.spaces[0].accent_hex(), original_accent);
    Ok(())
}

#[test]
fn new_spaces_rotate_through_the_accent_palette() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">new-space Research");
    core.submit_command()?;
    core.set_command_query(">new-space Notes");
    core.submit_command()?;

    let snapshot = core.snapshot()?;
    let accents: Vec<u32> = snapshot.spaces.iter().map(ely_domain::Space::accent_hex).collect();
    assert_eq!(accents.len(), 3);
    assert_ne!(accents[1], accents[2], "consecutive spaces must not share one accent");
    Ok(())
}

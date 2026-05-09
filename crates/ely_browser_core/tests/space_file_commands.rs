use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::CommandIntent;

#[test]
fn export_space_command_opens_space_settings_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">export-space");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("export-space".to_string())));
    assert_eq!(active_tab.title(), "Space Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/spaces");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn import_space_command_opens_space_settings_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">import-space-with-profiles");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("import-space-with-profiles".to_string())));
    assert_eq!(active_tab.title(), "Space Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/spaces");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

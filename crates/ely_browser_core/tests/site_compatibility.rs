use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::CommandIntent;

#[test]
fn open_site_compatibility_command_opens_diagnostics_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">open-site-compatibility");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("open-site-compatibility".to_string())));
    assert_eq!(active_tab.title(), "Site Compatibility");
    assert_eq!(active_tab.url().as_str(), "ely://site-compatibility");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

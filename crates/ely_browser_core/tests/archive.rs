use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::CommandIntent;

#[test]
fn open_archive_command_opens_archive_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">open-archive");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("open-archive".to_string())));
    assert_eq!(active_tab.title(), "Archived Tabs");
    assert_eq!(active_tab.url().as_str(), "ely://archive");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

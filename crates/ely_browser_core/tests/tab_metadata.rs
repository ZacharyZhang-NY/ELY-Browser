use std::error::Error;

use ely_browser_core::{BrowserCore, CoreError, InitialBrowserConfig};
use ely_domain::DomainError;

#[test]
fn new_tabs_start_without_favicon_key() -> Result<(), Box<dyn Error>> {
    let core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    assert_eq!(core.active_tab()?.favicon_key(), None);
    Ok(())
}

#[test]
fn tab_favicon_key_can_be_set_and_cleared() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.active_tab()?.id().clone();

    core.set_tab_favicon_key(&tab_id, "favicons/example.ico")?;
    assert_eq!(core.active_tab()?.favicon_key(), Some("favicons/example.ico"));

    core.clear_tab_favicon_key(&tab_id)?;
    assert_eq!(core.active_tab()?.favicon_key(), None);
    Ok(())
}

#[test]
fn tab_favicon_key_requires_text() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.active_tab()?.id().clone();

    let error = match core.set_tab_favicon_key(&tab_id, " ") {
        Err(error) => error,
        Ok(_) => return Err("favicon key should require text".into()),
    };

    assert_eq!(error, CoreError::Domain(DomainError::EmptyField { field: "favicon_key" }));
    Ok(())
}

#[test]
fn archived_tabs_preserve_favicon_key() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.active_tab()?.id().clone();

    core.set_tab_favicon_key(&tab_id, "favicons/example.ico")?;
    core.close_active_tab()?;
    let snapshot = core.snapshot()?;

    assert_eq!(snapshot.archived_tabs[0].tab().favicon_key(), Some("favicons/example.ico"));
    Ok(())
}

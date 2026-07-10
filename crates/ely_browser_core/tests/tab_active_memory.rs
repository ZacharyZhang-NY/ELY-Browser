use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{ProfileKind, UrlText};

#[test]
fn closing_a_background_tab_keeps_the_remembered_active_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let first_profile_id = core.active_tab()?.profile_id().clone();
    let tab_a = core.open_tab(UrlText::parse("https://example.com/a")?);
    let _tab_b = core.open_tab(UrlText::parse("https://example.com/b")?);
    let tab_c = core.open_tab(UrlText::parse("https://example.com/c")?);
    core.select_tab(&tab_a)?;

    core.close_tab(&tab_c)?;

    core.create_profile("Second", 0xf54e00, ProfileKind::Standard)?;
    let restored = core.select_profile(&first_profile_id)?;

    assert_eq!(
        restored, tab_a,
        "closing a background tab must not clobber the remembered active tab"
    );
    Ok(())
}

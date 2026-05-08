use std::error::Error;

use ely_browser_core::{BrowserCore, CoreError, InitialBrowserConfig};
use ely_domain::{ProfileId, ProfileKind};

#[test]
fn created_space_binds_current_profile_as_default() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let research_profile_id = core.create_profile("Research", 0x9fc9a2, ProfileKind::Standard)?;

    let research_space_id = core.create_space("Research", "R", 0x9fc9a2)?;
    let snapshot = core.snapshot()?;
    let Some(research_space) =
        snapshot.spaces.iter().find(|space| space.id() == &research_space_id)
    else {
        return Err("missing research space".into());
    };

    assert_eq!(research_space.default_profile_id(), &research_profile_id);
    assert_eq!(snapshot.active_profile_id, research_profile_id);
    Ok(())
}

#[test]
fn space_default_profile_updates_with_profile_validation() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let research_profile_id = core.create_profile("Research", 0x9fc9a2, ProfileKind::Standard)?;

    core.set_space_default_profile(&work_space_id, &research_profile_id)?;
    let snapshot = core.snapshot()?;
    let Some(work_space) = snapshot.spaces.iter().find(|space| space.id() == &work_space_id) else {
        return Err("missing work space".into());
    };

    assert_eq!(work_space.default_profile_id(), &research_profile_id);

    let missing_profile_id = ProfileId::new();
    let error = match core.set_space_default_profile(&work_space_id, &missing_profile_id) {
        Err(error) => error,
        Ok(_) => return Err("space default profile should require an existing profile".into()),
    };

    assert_eq!(error, CoreError::ProfileNotFound { id: missing_profile_id });
    Ok(())
}

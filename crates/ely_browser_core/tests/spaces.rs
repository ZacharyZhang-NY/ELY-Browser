use std::error::Error;

use ely_browser_core::{BrowserCore, CoreError, InitialBrowserConfig};
use ely_domain::{ArchivePolicy, DEFAULT_SIDEBAR_WIDTH_PX, ProfileId, ProfileKind};

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
fn created_space_records_creation_timestamps() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let research_space_id = core.create_space("Research", "R", 0x9fc9a2)?;
    let snapshot = core.snapshot()?;
    let Some(research_space) =
        snapshot.spaces.iter().find(|space| space.id() == &research_space_id)
    else {
        return Err("missing research space".into());
    };

    assert_eq!(research_space.created_at(), research_space.updated_at());
    Ok(())
}

#[test]
fn created_space_uses_default_sidebar_width() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let research_space_id = core.create_space("Research", "R", 0x9fc9a2)?;
    let snapshot = core.snapshot()?;
    let Some(research_space) =
        snapshot.spaces.iter().find(|space| space.id() == &research_space_id)
    else {
        return Err("missing research space".into());
    };

    assert_eq!(research_space.sidebar_width_px(), DEFAULT_SIDEBAR_WIDTH_PX);
    Ok(())
}

#[test]
fn created_spaces_receive_incrementing_sort_keys() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let research_space_id = core.create_space("Research", "R", 0x9fc9a2)?;

    let snapshot = core.snapshot()?;
    let Some(work_space) = snapshot.spaces.iter().find(|space| space.id() == &work_space_id) else {
        return Err("missing work space".into());
    };
    let Some(research_space) =
        snapshot.spaces.iter().find(|space| space.id() == &research_space_id)
    else {
        return Err("missing research space".into());
    };

    assert_eq!(work_space.sort_key(), 0);
    assert_eq!(research_space.sort_key(), 1);
    Ok(())
}

#[test]
fn snapshot_orders_spaces_by_sort_key() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let research_space_id = core.create_space("Research", "R", 0x9fc9a2)?;

    core.set_space_sort_key(&work_space_id, 20)?;
    core.set_space_sort_key(&research_space_id, 10)?;
    let snapshot = core.snapshot()?;

    assert_eq!(snapshot.spaces[0].id(), &research_space_id);
    assert_eq!(snapshot.spaces[1].id(), &work_space_id);
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

#[test]
fn space_settings_refresh_updated_at() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let before_update = active_space_updated_at(&core, &work_space_id)?;

    core.set_space_archive_policy(&work_space_id, ArchivePolicy::IdleDays(7))?;
    let after_archive_update = active_space_updated_at(&core, &work_space_id)?;

    let research_profile_id = core.create_profile("Research", 0x9fc9a2, ProfileKind::Standard)?;
    core.set_space_default_profile(&work_space_id, &research_profile_id)?;
    let after_profile_update = active_space_updated_at(&core, &work_space_id)?;

    core.set_space_sidebar_width(&work_space_id, 320)?;
    let after_sidebar_update = active_space_updated_at(&core, &work_space_id)?;

    core.set_space_sort_key(&work_space_id, 8)?;
    let after_sort_update = active_space_updated_at(&core, &work_space_id)?;

    assert!(after_archive_update > before_update);
    assert!(after_profile_update > after_archive_update);
    assert!(after_sidebar_update > after_profile_update);
    assert!(after_sort_update > after_sidebar_update);
    Ok(())
}

#[test]
fn space_sidebar_width_updates_selected_space() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;

    core.set_space_sidebar_width(&work_space_id, 320)?;
    let snapshot = core.snapshot()?;
    let Some(work_space) = snapshot.spaces.iter().find(|space| space.id() == &work_space_id) else {
        return Err("missing work space".into());
    };

    assert_eq!(work_space.sidebar_width_px(), 320);
    Ok(())
}

fn active_space_updated_at(
    core: &BrowserCore,
    space_id: &ely_domain::SpaceId,
) -> Result<std::time::SystemTime, Box<dyn Error>> {
    let snapshot = core.snapshot()?;
    let Some(space) = snapshot.spaces.iter().find(|space| space.id() == space_id) else {
        return Err("missing space".into());
    };

    Ok(space.updated_at())
}

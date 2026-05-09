use std::{
    error::Error,
    time::{Duration, SystemTime},
};

use ely_browser_core::{BrowserCore, CoreError, InitialBrowserConfig};
use ely_domain::{
    ArchivePolicy, DEFAULT_SIDEBAR_WIDTH_PX, ProfileId, ProfileKind, SpaceId, UrlText,
};

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
fn space_creation_from_private_profile_uses_space_default_profile() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.snapshot()?.active_profile_id;

    core.create_profile("Private", 0x807d72, ProfileKind::Private)?;
    let research_space_id = core.create_space("Research", "R", 0x9fc9a2)?;
    let snapshot = core.snapshot()?;
    let Some(research_space) =
        snapshot.spaces.iter().find(|space| space.id() == &research_space_id)
    else {
        return Err("missing research space".into());
    };

    assert_eq!(research_space.default_profile_id(), &default_profile_id);
    assert_eq!(snapshot.active_profile_id, default_profile_id);
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
fn moving_spaces_updates_visible_order() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let research_space_id = core.create_space("Research", "R", 0x9fc9a2)?;
    let personal_space_id = core.create_space("Personal", "P", 0x8eb7d4)?;

    core.set_space_sort_key(&work_space_id, 20)?;
    core.set_space_sort_key(&research_space_id, 10)?;
    core.set_space_sort_key(&personal_space_id, 30)?;

    assert!(core.move_space_up(&work_space_id)?);
    assert_eq!(
        ordered_space_ids(&core)?,
        vec![work_space_id.clone(), research_space_id.clone(), personal_space_id.clone()]
    );

    assert!(core.move_space_down(&work_space_id)?);
    assert_eq!(
        ordered_space_ids(&core)?,
        vec![research_space_id, work_space_id, personal_space_id]
    );
    Ok(())
}

#[test]
fn moving_boundary_or_missing_space_is_safe() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;

    assert!(!core.move_space_up(&work_space_id)?);
    assert!(!core.move_space_down(&work_space_id)?);

    let missing_space_id = SpaceId::new();
    let error = match core.move_space_up(&missing_space_id) {
        Err(error) => error,
        Ok(_) => return Err("moving a missing space should fail".into()),
    };

    assert_eq!(error, CoreError::SpaceNotFound { id: missing_space_id });
    Ok(())
}

#[test]
fn trashing_space_removes_it_and_restores_with_tabs() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let research_space_id = core.create_space("Research", "R", 0x9fc9a2)?;
    let research_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);

    core.select_space(&work_space_id)?;
    assert!(core.trash_space(&research_space_id, SystemTime::UNIX_EPOCH)?);
    let snapshot = core.snapshot()?;
    assert_eq!(snapshot.active_space_id, work_space_id);
    assert!(snapshot.spaces.iter().all(|space| space.id() != &research_space_id));
    assert_eq!(snapshot.trashed_spaces[0].space().id(), &research_space_id);
    assert!(snapshot.trashed_spaces[0].tabs().iter().any(|tab| tab.id() == &research_tab_id));

    assert!(core.restore_trashed_space(&research_space_id)?);
    let snapshot = core.snapshot()?;
    assert_eq!(snapshot.active_space_id, research_space_id);
    assert!(snapshot.tabs.iter().any(|tab| tab.id() == &research_tab_id));
    assert!(snapshot.trashed_spaces.is_empty());
    Ok(())
}

#[test]
fn trashing_active_space_selects_remaining_space() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let research_space_id = core.create_space("Research", "R", 0x9fc9a2)?;

    assert!(core.trash_space(&research_space_id, SystemTime::UNIX_EPOCH)?);
    let snapshot = core.snapshot()?;

    assert_eq!(snapshot.active_space_id, work_space_id);
    assert_eq!(snapshot.active_space_name, "Work");
    Ok(())
}

#[test]
fn trashed_space_keeps_thirty_day_retention() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let research_space_id = core.create_space("Research", "R", 0x9fc9a2)?;
    let trashed_at = SystemTime::UNIX_EPOCH + Duration::from_secs(900);

    core.trash_space(&research_space_id, trashed_at)?;
    let snapshot = core.snapshot()?;
    let trashed_space = &snapshot.trashed_spaces[0];

    assert_eq!(trashed_space.trashed_at(), trashed_at);
    assert_eq!(trashed_space.purge_at(), trashed_at + Duration::from_secs(30 * 86_400));
    Ok(())
}

#[test]
fn trashing_last_or_missing_space_is_rejected() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let error = match core.trash_space(&work_space_id, SystemTime::UNIX_EPOCH) {
        Err(error) => error,
        Ok(_) => return Err("trashing the last space should fail".into()),
    };

    assert_eq!(error, CoreError::LastSpaceCannotBeTrashed);

    let missing_space_id = SpaceId::new();
    core.create_space("Research", "R", 0x9fc9a2)?;
    let error = match core.trash_space(&missing_space_id, SystemTime::UNIX_EPOCH) {
        Err(error) => error,
        Ok(_) => return Err("trashing a missing space should fail".into()),
    };

    assert_eq!(error, CoreError::SpaceNotFound { id: missing_space_id });
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
fn space_default_profile_rejects_private_profile() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let snapshot = core.snapshot()?;
    let work_space_id = snapshot.active_space_id;
    let default_profile_id = snapshot.active_profile_id;
    let private_profile_id = core.create_profile("Private", 0x807d72, ProfileKind::Private)?;

    let error = match core.set_space_default_profile(&work_space_id, &private_profile_id) {
        Err(error) => error,
        Ok(_) => return Err("space default profile accepted a private profile".into()),
    };

    assert_eq!(error, CoreError::PrivateProfileDefaultLocked { id: private_profile_id.clone() });
    let snapshot = core.snapshot()?;
    let Some(work_space) = snapshot.spaces.iter().find(|space| space.id() == &work_space_id) else {
        return Err("missing work space".into());
    };
    assert_eq!(work_space.default_profile_id(), &default_profile_id);
    Ok(())
}

#[test]
fn active_space_default_profile_updates_current_space() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let research_profile_id = core.create_profile("Research", 0x9fc9a2, ProfileKind::Standard)?;

    core.set_active_space_default_profile(&research_profile_id)?;
    let snapshot = core.snapshot()?;
    let Some(work_space) = snapshot.spaces.iter().find(|space| space.id() == &work_space_id) else {
        return Err("missing work space".into());
    };

    assert_eq!(work_space.default_profile_id(), &research_profile_id);
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

#[test]
fn selecting_adjacent_spaces_uses_sort_order_with_wraparound() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let research_space_id = core.create_space("Research", "R", 0x9fc9a2)?;
    let personal_space_id = core.create_space("Personal", "P", 0x8eb7d4)?;

    core.set_space_sort_key(&work_space_id, 20)?;
    core.set_space_sort_key(&research_space_id, 10)?;
    core.set_space_sort_key(&personal_space_id, 30)?;
    core.select_space(&work_space_id)?;

    core.select_next_space()?;
    assert_eq!(core.snapshot()?.active_space_id, personal_space_id);

    core.select_next_space()?;
    assert_eq!(core.snapshot()?.active_space_id, research_space_id);

    core.select_previous_space()?;
    assert_eq!(core.snapshot()?.active_space_id, personal_space_id);
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

fn ordered_space_ids(core: &BrowserCore) -> Result<Vec<SpaceId>, Box<dyn Error>> {
    Ok(core.snapshot()?.spaces.iter().map(|space| space.id().clone()).collect())
}

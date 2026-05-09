use std::error::Error;

use ely_browser_core::{
    BrowserCore, CoreError, ELYSPACE_FILE_EXTENSION, ELYSPACE_SCHEMA_VERSION, InitialBrowserConfig,
    SpaceImportProfileMapping,
};
use ely_domain::{ArchivePolicy, ProfileKind, UrlText};
use serde_json::Value;

#[test]
fn exports_space_package_json_with_settings_and_tabs() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let space_id = core.snapshot()?.active_space_id;
    let active_tab_id = core.snapshot()?.active_tab_id;

    core.set_space_archive_policy(&space_id, ArchivePolicy::IdleDays(14))?;
    core.set_space_sidebar_width(&space_id, 320)?;
    core.set_tab_favicon_key(&active_tab_id, "favicons/work.ico")?;
    core.toggle_active_tab_pinned()?;
    core.toggle_active_tab_favorite()?;
    core.open_tab(UrlText::parse("https://servo.org/")?);

    let package = core.export_space_package(&space_id)?;
    let package_json = core.export_space_package_json(&space_id)?;
    let value: Value = serde_json::from_str(&package_json)?;

    assert_eq!(ELYSPACE_FILE_EXTENSION, "elyspace");
    assert_eq!(package.version(), ELYSPACE_SCHEMA_VERSION);
    assert_eq!(package.space_name(), "Work");
    assert_eq!(package.tab_count(), 2);
    assert_eq!(json_u64(&value, "version")?, 1);
    assert_eq!(json_str(json_object(&value, "space")?, "name")?, "Work");
    assert_eq!(json_object_u64(json_object(&value, "space")?, "sidebar_width_px")?, 320);
    assert_eq!(json_array(&value, "tabs")?.len(), 2);
    Ok(())
}

#[test]
fn imports_space_package_with_active_profile_mapping() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_space_id = source.snapshot()?.active_space_id;
    source.set_space_archive_policy(&source_space_id, ArchivePolicy::IdleDays(7))?;
    source.set_space_sidebar_width(&source_space_id, 360)?;
    source.open_tab(UrlText::parse("https://example.com")?);
    let package_json = source.export_space_package_json(&source_space_id)?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let target_profile_id = target.snapshot()?.active_profile_id;
    let imported_space_id = target
        .import_space_package_json(&package_json, SpaceImportProfileMapping::UseActiveProfile)?;
    let snapshot = target.snapshot()?;
    let Some(imported_space) =
        snapshot.spaces.iter().find(|space| space.id() == &imported_space_id)
    else {
        return Err("missing imported space".into());
    };

    assert_eq!(imported_space.name(), "Work");
    assert_eq!(imported_space.archive_policy(), &ArchivePolicy::IdleDays(7));
    assert_eq!(imported_space.sidebar_width_px(), 360);
    assert_eq!(imported_space.default_profile_id(), &target_profile_id);
    assert!(snapshot.tabs.iter().all(|tab| tab.profile_id() == &target_profile_id));
    assert!(snapshot.tabs.iter().any(|tab| tab.url().as_str() == "https://example.com"));
    Ok(())
}

#[test]
fn imports_space_package_with_existing_profile_mapping() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let research_profile_id = core.create_profile("Research", 0x9fc9a2, ProfileKind::Standard)?;
    let research_space_id = core.create_space("Research", "R", 0x9fc9a2)?;
    core.open_tab(UrlText::parse("https://servo.org/")?);
    let package_json = core.export_space_package_json(&research_space_id)?;

    let imported_space_id =
        core.import_space_package_json(&package_json, SpaceImportProfileMapping::PreserveExisting)?;
    let snapshot = core.snapshot()?;
    let Some(imported_space) =
        snapshot.spaces.iter().find(|space| space.id() == &imported_space_id)
    else {
        return Err("missing imported space".into());
    };

    assert_eq!(imported_space.default_profile_id(), &research_profile_id);
    assert!(snapshot.tabs.iter().all(|tab| tab.profile_id() == &research_profile_id));
    Ok(())
}

#[test]
fn importing_space_package_rejects_missing_preserved_profile() -> Result<(), Box<dyn Error>> {
    let source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_profile_id = source.snapshot()?.active_profile_id;
    let source_space_id = source.snapshot()?.active_space_id;
    let package_json = source.export_space_package_json(&source_space_id)?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let error = match target
        .import_space_package_json(&package_json, SpaceImportProfileMapping::PreserveExisting)
    {
        Err(error) => error,
        Ok(_) => return Err("import should reject missing preserved profile".into()),
    };

    assert_eq!(error, CoreError::ProfileNotFound { id: source_profile_id });
    Ok(())
}

fn json_object<'a>(
    value: &'a Value,
    key: &str,
) -> Result<&'a serde_json::Map<String, Value>, Box<dyn Error>> {
    value.get(key).and_then(Value::as_object).ok_or_else(|| format!("missing object {key}").into())
}

fn json_array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, Box<dyn Error>> {
    value.get(key).and_then(Value::as_array).ok_or_else(|| format!("missing array {key}").into())
}

fn json_str<'a>(
    value: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str, Box<dyn Error>> {
    value.get(key).and_then(Value::as_str).ok_or_else(|| format!("missing string {key}").into())
}

fn json_object_u64(
    value: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<u64, Box<dyn Error>> {
    value.get(key).and_then(Value::as_u64).ok_or_else(|| format!("missing number {key}").into())
}

fn json_u64(value: &Value, key: &str) -> Result<u64, Box<dyn Error>> {
    value.get(key).and_then(Value::as_u64).ok_or_else(|| format!("missing number {key}").into())
}

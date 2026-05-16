use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{PluginManifest, SyncObjectKind, SyncObjectPolicy};

#[test]
fn sync_snapshot_updates_existing_plugin_settings() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let plugin_id =
        source.install_plugin(plugin_manifest("com.elydora.reader", DEFAULT_CHECKSUM)?, false)?;
    source.set_plugin_private_window_allowed(&plugin_id, true)?;
    source.disable_plugin(&plugin_id)?;
    pause_tab_sync(&mut source);
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let target_plugin_id =
        target.install_plugin(plugin_manifest("com.elydora.reader", DEFAULT_CHECKSUM)?, false)?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;
    let [plugin] = snapshot.installed_plugins.as_slice() else {
        return Err(format!(
            "expected 1 installed plugin, got {}",
            snapshot.installed_plugins.len()
        )
        .into());
    };

    assert_eq!(summary.imported(), 0);
    assert_eq!(summary.updated(), 1);
    assert_eq!(summary.skipped(), 0);
    assert_eq!(plugin.id(), &target_plugin_id);
    assert!(!plugin.enabled());
    assert!(plugin.private_window_allowed());
    assert_eq!(snapshot.plugin_audit_events.len(), 1);
    Ok(())
}

#[test]
fn sync_snapshot_omits_paused_plugin_settings() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let plugin_id =
        source.install_plugin(plugin_manifest("com.elydora.reader", DEFAULT_CHECKSUM)?, false)?;
    source.disable_plugin(&plugin_id)?;
    source.set_sync_object_policy(SyncObjectKind::PluginSettings, SyncObjectPolicy::Paused);
    pause_tab_sync(&mut source);
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    target.install_plugin(plugin_manifest("com.elydora.reader", DEFAULT_CHECKSUM)?, false)?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;
    let [plugin] = snapshot.installed_plugins.as_slice() else {
        return Err(format!(
            "expected 1 installed plugin, got {}",
            snapshot.installed_plugins.len()
        )
        .into());
    };

    assert_eq!(summary.imported(), 0);
    assert_eq!(summary.updated(), 0);
    assert_eq!(summary.skipped(), 0);
    assert!(plugin.enabled());
    assert!(!plugin.private_window_allowed());
    Ok(())
}

#[test]
fn sync_snapshot_skips_missing_plugin_package() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let plugin_id =
        source.install_plugin(plugin_manifest("com.elydora.reader", DEFAULT_CHECKSUM)?, false)?;
    source.disable_plugin(&plugin_id)?;
    pause_tab_sync(&mut source);
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;

    assert_eq!(summary.imported(), 0);
    assert_eq!(summary.updated(), 0);
    assert_eq!(summary.skipped(), 1);
    assert!(snapshot.installed_plugins.is_empty());
    Ok(())
}

#[test]
fn sync_snapshot_skips_checksum_mismatched_plugin_package() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let plugin_id =
        source.install_plugin(plugin_manifest("com.elydora.reader", DEFAULT_CHECKSUM)?, false)?;
    source.disable_plugin(&plugin_id)?;
    pause_tab_sync(&mut source);
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    target.install_plugin(plugin_manifest("com.elydora.reader", OTHER_CHECKSUM)?, false)?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;
    let [plugin] = snapshot.installed_plugins.as_slice() else {
        return Err(format!(
            "expected 1 installed plugin, got {}",
            snapshot.installed_plugins.len()
        )
        .into());
    };

    assert_eq!(summary.imported(), 0);
    assert_eq!(summary.updated(), 0);
    assert_eq!(summary.skipped(), 1);
    assert!(plugin.enabled());
    assert_eq!(plugin.manifest().checksum(), OTHER_CHECKSUM.to_ascii_lowercase());
    Ok(())
}

const DEFAULT_CHECKSUM: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const OTHER_CHECKSUM: &str = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

fn plugin_manifest(id: &str, checksum: &str) -> Result<PluginManifest, Box<dyn Error>> {
    PluginManifest::from_toml(plugin_manifest_toml(id, checksum).as_str()).map_err(Into::into)
}

fn pause_tab_sync(core: &mut BrowserCore) {
    core.set_sync_object_policy(SyncObjectKind::Tabs, SyncObjectPolicy::Paused);
}

fn plugin_manifest_toml(id: &str, checksum: &str) -> String {
    format!(
        r#"
id = "{id}"
name = "Reader Exporter"
description = "Exports the active reader view."
author = "Elydora"
homepage = "https://elydora.com/plugins/reader"
permissions = ["page:metadata", "ui:command"]
contributes = ["command-bar-command"]
min_ely_build = "0.1.0"
checksum = "{checksum}"

[signature]
algorithm = "ed25519"
key_id = "elydora-alpha-plugins"
public_key = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
value = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
"#
    )
}

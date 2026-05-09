use std::{error::Error, io};

use ely_browser_core::{BrowserCore, CoreError, InitialBrowserConfig, PluginAuditAction};
use ely_domain::{CommandIntent, CommandScope, PluginId, PluginManifest};

#[test]
fn install_plugin_from_file_command_opens_plugin_settings_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">install-plugin-from-file");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("install-plugin-from-file".to_string())));
    assert_eq!(active_tab.title(), "Plugin Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/plugins");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn installs_standard_plugin_and_records_audit_event() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let manifest = plugin_manifest("com.elydora.reader", &["page:metadata", "ui:command"])?;

    let plugin_id = core.install_plugin(manifest, false)?;
    let snapshot = core.snapshot()?;

    assert_eq!(plugin_id.as_str(), "com.elydora.reader");
    assert_eq!(snapshot.installed_plugins.len(), 1);
    assert_eq!(snapshot.installed_plugins[0].id(), &plugin_id);
    assert!(snapshot.installed_plugins[0].enabled());
    assert!(!snapshot.installed_plugins[0].high_risk_confirmed());
    assert_eq!(snapshot.plugin_audit_events.len(), 1);
    assert_eq!(snapshot.plugin_audit_events[0].plugin_id(), &plugin_id);
    assert_eq!(snapshot.plugin_audit_events[0].action(), &PluginAuditAction::Installed);
    Ok(())
}

#[test]
fn requires_confirmation_for_high_risk_plugin_install() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let manifest = plugin_manifest("com.elydora.scripter", &["page:script"])?;

    let error = install_error(&mut core, manifest, false)?;
    let snapshot = core.snapshot()?;

    assert!(matches!(
        error,
        CoreError::PluginHighRiskConfirmationRequired { id }
            if id.as_str() == "com.elydora.scripter"
    ));
    assert!(snapshot.installed_plugins.is_empty());
    assert!(snapshot.plugin_audit_events.is_empty());
    Ok(())
}

#[test]
fn installs_high_risk_plugin_after_confirmation() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let manifest = plugin_manifest("com.elydora.scripter", &["page:script"])?;

    let plugin_id = core.install_plugin(manifest, true)?;
    let snapshot = core.snapshot()?;

    assert_eq!(snapshot.installed_plugins.len(), 1);
    assert_eq!(snapshot.installed_plugins[0].id(), &plugin_id);
    assert!(snapshot.installed_plugins[0].high_risk_confirmed());
    assert_eq!(snapshot.plugin_audit_events[0].action(), &PluginAuditAction::Installed);
    Ok(())
}

#[test]
fn rejects_duplicate_plugin_install() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.install_plugin(plugin_manifest("com.elydora.reader", &["page:metadata"])?, false)?;
    let duplicate = plugin_manifest("com.elydora.reader", &["page:metadata"])?;

    let error = install_error(&mut core, duplicate, false)?;
    let snapshot = core.snapshot()?;

    assert!(matches!(
        error,
        CoreError::PluginAlreadyInstalled { id } if id.as_str() == "com.elydora.reader"
    ));
    assert_eq!(snapshot.installed_plugins.len(), 1);
    assert_eq!(snapshot.plugin_audit_events.len(), 1);
    Ok(())
}

#[test]
fn records_plugin_enable_disable_audit_events() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let plugin_id =
        core.install_plugin(plugin_manifest("com.elydora.reader", &["page:metadata"])?, false)?;

    core.disable_plugin(&plugin_id)?;
    let disabled_snapshot = core.snapshot()?;

    assert!(!disabled_snapshot.installed_plugins[0].enabled());
    assert_eq!(disabled_snapshot.plugin_audit_events.len(), 2);
    assert_eq!(disabled_snapshot.plugin_audit_events[1].action(), &PluginAuditAction::Disabled);

    core.enable_plugin(&plugin_id)?;
    let enabled_snapshot = core.snapshot()?;

    assert!(enabled_snapshot.installed_plugins[0].enabled());
    assert_eq!(enabled_snapshot.plugin_audit_events.len(), 3);
    assert_eq!(enabled_snapshot.plugin_audit_events[2].action(), &PluginAuditAction::Enabled);
    Ok(())
}

#[test]
fn uninstalls_plugin_and_records_audit_event() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let plugin_id =
        core.install_plugin(plugin_manifest("com.elydora.reader", &["page:metadata"])?, false)?;

    core.uninstall_plugin(&plugin_id)?;
    let snapshot = core.snapshot()?;

    assert!(snapshot.installed_plugins.is_empty());
    assert_eq!(snapshot.plugin_audit_events.len(), 2);
    assert_eq!(snapshot.plugin_audit_events[1].plugin_id(), &plugin_id);
    assert_eq!(snapshot.plugin_audit_events[1].action(), &PluginAuditAction::Uninstalled);
    Ok(())
}

#[test]
fn allows_reinstall_after_plugin_uninstall() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let plugin_id =
        core.install_plugin(plugin_manifest("com.elydora.reader", &["page:metadata"])?, false)?;
    core.uninstall_plugin(&plugin_id)?;

    let reinstalled_id =
        core.install_plugin(plugin_manifest("com.elydora.reader", &["page:metadata"])?, false)?;
    let snapshot = core.snapshot()?;

    assert_eq!(reinstalled_id, plugin_id);
    assert_eq!(snapshot.installed_plugins.len(), 1);
    assert_eq!(snapshot.plugin_audit_events.len(), 3);
    assert_eq!(snapshot.plugin_audit_events[2].action(), &PluginAuditAction::Installed);
    Ok(())
}

#[test]
fn rejects_unknown_plugin_state_change() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let plugin_id = PluginId::parse("com.elydora.missing")?;

    let error = core
        .disable_plugin(&plugin_id)
        .err()
        .ok_or_else(|| io::Error::other("missing plugin disable succeeded"))?;

    assert!(matches!(
        error,
        CoreError::PluginNotFound { id } if id.as_str() == "com.elydora.missing"
    ));
    Ok(())
}

#[test]
fn plugin_scoped_search_opens_installed_plugin_detail() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.install_plugin(plugin_manifest("com.elydora.reader", &["page:metadata"])?, false)?;

    core.set_command_query("@plugins reader");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Plugins,
            query: "reader".to_string()
        })
    );
    assert_eq!(active_tab.title(), "Plugin Details");
    assert_eq!(active_tab.url().as_str(), "ely://plugin/com.elydora.reader");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn plugin_scoped_search_preserves_query_without_match() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query("@plugins missing");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Plugins,
            query: "missing".to_string()
        })
    );
    assert_eq!(active_tab.url().as_str(), "ely://new-tab");
    assert_eq!(core.snapshot()?.command_query, "@plugins missing");
    Ok(())
}

fn install_error(
    core: &mut BrowserCore,
    manifest: PluginManifest,
    high_risk_confirmed: bool,
) -> Result<CoreError, Box<dyn Error>> {
    core.install_plugin(manifest, high_risk_confirmed)
        .err()
        .ok_or_else(|| io::Error::other("plugin install succeeded").into())
}

fn plugin_manifest(id: &str, permissions: &[&str]) -> Result<PluginManifest, Box<dyn Error>> {
    PluginManifest::from_toml(plugin_manifest_toml(id, permissions).as_str()).map_err(Into::into)
}

fn plugin_manifest_toml(id: &str, permissions: &[&str]) -> String {
    let permission_values = permissions
        .iter()
        .map(|permission| format!("\"{permission}\""))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        r#"
id = "{id}"
name = "Reader Exporter"
description = "Exports the active reader view."
author = "Elydora"
homepage = "https://elydora.com/plugins/reader"
permissions = [{permission_values}]
contributes = ["command-bar-command"]
min_ely_build = "0.1.0"
checksum = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"

[signature]
algorithm = "ed25519"
key_id = "elydora-alpha-plugins"
public_key = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
value = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
"#
    )
}

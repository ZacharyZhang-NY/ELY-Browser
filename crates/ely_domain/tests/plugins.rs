use std::{error::Error, io};

use ely_domain::{
    DomainError, PluginContributionPoint, PluginManifest, PluginPermission, PluginPermissionRisk,
    PluginSignatureAlgorithm,
};

#[test]
fn parses_signed_plugin_manifest() -> Result<(), Box<dyn Error>> {
    let manifest = PluginManifest::from_toml(valid_manifest().as_str())?;

    assert_eq!(manifest.id().as_str(), "com.elydora.reader");
    assert_eq!(manifest.name(), "Reader Exporter");
    assert_eq!(manifest.homepage(), "https://elydora.com/plugins/reader");
    assert_eq!(
        manifest.permissions(),
        &[PluginPermission::PageMetadata, PluginPermission::UiCommand]
    );
    assert_eq!(
        manifest.contributes(),
        &[PluginContributionPoint::CommandBarCommand, PluginContributionPoint::ReadingModeExporter]
    );
    assert_eq!(manifest.min_ely_build().to_string(), "0.1.0");
    assert_eq!(
        manifest.checksum(),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(manifest.signature().algorithm(), &PluginSignatureAlgorithm::Ed25519);
    assert_eq!(
        manifest.signature().value(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );
    Ok(())
}

#[test]
fn classifies_plugin_permission_risk_for_separate_confirmation() -> Result<(), Box<dyn Error>> {
    let manifest = PluginManifest::from_toml(valid_manifest().as_str())?;
    let high_risk_permissions = manifest.high_risk_permissions().collect::<Vec<_>>();

    assert!(high_risk_permissions.is_empty());
    assert_eq!(PluginPermission::PageMetadata.risk(), PluginPermissionRisk::Standard);
    assert_eq!(PluginPermission::UiCommand.risk(), PluginPermissionRisk::Standard);
    assert_eq!(PluginPermission::HistoryRead.risk(), PluginPermissionRisk::High);
    assert!(PluginPermission::PageScript.requires_separate_confirmation());
    assert!(PluginPermission::FilesystemWrite.requires_separate_confirmation());
    Ok(())
}

#[test]
fn rejects_unknown_plugin_permission() -> Result<(), Box<dyn Error>> {
    let manifest = valid_manifest().replace("page:metadata", "tabs:admin");

    let error = parse_error(manifest.as_str())?;

    assert!(matches!(
        error,
        DomainError::InvalidPluginPermission { value } if value == "tabs:admin"
    ));
    Ok(())
}

#[test]
fn rejects_duplicate_plugin_permission() -> Result<(), Box<dyn Error>> {
    let manifest = valid_manifest()
        .replace("[\"page:metadata\", \"ui:command\"]", "[\"page:metadata\", \"page:metadata\"]");

    let error = parse_error(manifest.as_str())?;

    assert!(matches!(
        error,
        DomainError::DuplicatePluginPermission { value } if value == "page:metadata"
    ));
    Ok(())
}

#[test]
fn rejects_unknown_plugin_contribution() -> Result<(), Box<dyn Error>> {
    let manifest = valid_manifest().replace("reading-mode-exporter", "hidden-root-hook");

    let error = parse_error(manifest.as_str())?;

    assert!(matches!(
        error,
        DomainError::InvalidPluginContribution { value } if value == "hidden-root-hook"
    ));
    Ok(())
}

#[test]
fn rejects_unsigned_plugin_manifest() -> Result<(), Box<dyn Error>> {
    let manifest = valid_manifest().replace(
        "value = \"BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB\"",
        "value = \"\"",
    );

    let error = parse_error(manifest.as_str())?;

    assert!(matches!(error, DomainError::EmptyField { field } if field == "signature"));
    Ok(())
}

#[test]
fn rejects_mismatched_plugin_checksum() -> Result<(), Box<dyn Error>> {
    let manifest = valid_manifest().replace(
        "checksum = \"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\"",
        "checksum = \"1234\"",
    );

    let error = parse_error(manifest.as_str())?;

    assert!(matches!(
        error,
        DomainError::InvalidPluginChecksum { value } if value == "1234"
    ));
    Ok(())
}

#[test]
fn rejects_non_http_plugin_homepage() -> Result<(), Box<dyn Error>> {
    let manifest = valid_manifest().replace(
        "homepage = \"https://elydora.com/plugins/reader\"",
        "homepage = \"file:///tmp/reader\"",
    );

    let error = parse_error(manifest.as_str())?;

    assert!(matches!(
        error,
        DomainError::InvalidPluginHomepage { value } if value == "file:///tmp/reader"
    ));
    Ok(())
}

#[test]
fn rejects_extra_manifest_fields() -> Result<(), Box<dyn Error>> {
    let manifest = valid_manifest().replace("[signature]", "process_escape = true\n\n[signature]");

    let error = parse_error(manifest.as_str())?;

    assert!(matches!(error, DomainError::InvalidPluginManifest { .. }));
    Ok(())
}

fn parse_error(manifest: &str) -> Result<DomainError, Box<dyn Error>> {
    match PluginManifest::from_toml(manifest) {
        Ok(_) => Err(io::Error::other("manifest parsed successfully").into()),
        Err(error) => Ok(error),
    }
}

fn valid_manifest() -> String {
    r#"
id = "com.elydora.reader"
name = "Reader Exporter"
description = "Exports the active reader view."
author = "Elydora"
homepage = "https://elydora.com/plugins/reader"
permissions = ["page:metadata", "ui:command"]
contributes = ["command-bar-command", "reading-mode-exporter"]
min_ely_build = "0.1.0"
checksum = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"

[signature]
algorithm = "ed25519"
value = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
"#
    .to_string()
}

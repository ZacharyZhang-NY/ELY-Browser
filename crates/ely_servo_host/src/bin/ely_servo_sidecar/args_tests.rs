use std::{env, path::PathBuf};

use super::{SidecarArgsError, SidecarCommand, parse_command};
use ely_domain::{
    DEFAULT_ZOOM_PERCENT, DomainError, ProfileId, SitePermissionDecision, SitePermissionFeature,
};
use ely_servo_host::RenderingContextKind;

#[test]
fn parses_snapshot_profile_identity() -> Result<(), SidecarArgsError> {
    let profile_id = ProfileId::new();
    let profile_data_dir = env::temp_dir().join(profile_id.as_str());
    let args = parse_snapshot_command(&profile_id, profile_data_dir.clone())?;

    assert_eq!(args.profile_id, profile_id);
    assert_eq!(args.profile_data_dir, profile_data_dir);
    assert_eq!(args.page_zoom_percent, DEFAULT_ZOOM_PERCENT);
    Ok(())
}

#[test]
fn rejects_invalid_snapshot_profile_id() {
    let command = parse_command(
        [
            "ely_servo_sidecar",
            "snapshot",
            "--url",
            "https://example.com",
            "--profile-id",
            "profile_invalid",
            "--profile-data-dir",
            "/tmp/profile",
            "--rgba-out",
            "/tmp/frame.rgba",
            "--width",
            "64",
            "--height",
            "64",
        ]
        .into_iter()
        .map(str::to_string),
    );

    assert!(matches!(command, Err(SidecarArgsError::Domain(DomainError::InvalidEntityId { .. }))));
}

#[test]
fn parses_snapshot_site_permissions() -> Result<(), SidecarArgsError> {
    let profile_id = ProfileId::new();
    let profile_data_dir = env::temp_dir().join(profile_id.as_str());
    let mut command = snapshot_command_args(&profile_id, profile_data_dir);
    command.push("--site-permission".to_string());
    command.push(
        r#"{"origin":"https://example.com","feature":"camera","decision":"allow-once"}"#
            .to_string(),
    );

    let args = snapshot_args(parse_command(command)?);
    assert_eq!(args.site_permissions.len(), 1);
    let permission = &args.site_permissions[0];
    assert_eq!(permission.origin.as_str(), "https://example.com");
    assert_eq!(permission.feature, SitePermissionFeature::Camera);
    assert_eq!(permission.decision, SitePermissionDecision::AllowOnce);
    Ok(())
}

#[test]
fn parses_snapshot_page_zoom_percent() -> Result<(), SidecarArgsError> {
    let profile_id = ProfileId::new();
    let profile_data_dir = env::temp_dir().join(profile_id.as_str());
    let mut command = snapshot_command_args(&profile_id, profile_data_dir);
    command.push("--page-zoom-percent".to_string());
    command.push("125".to_string());

    let args = snapshot_args(parse_command(command)?);
    assert_eq!(args.page_zoom_percent, 125);
    Ok(())
}

#[test]
fn rejects_out_of_range_snapshot_page_zoom_percent() {
    let profile_id = ProfileId::new();
    let profile_data_dir = env::temp_dir().join(profile_id.as_str());
    let mut command = snapshot_command_args(&profile_id, profile_data_dir);
    command.push("--page-zoom-percent".to_string());
    command.push("5".to_string());

    assert!(matches!(
        parse_command(command),
        Err(SidecarArgsError::Domain(DomainError::InvalidZoomPercent { value: 5, .. }))
    ));
}

fn parse_snapshot_command(
    profile_id: &ProfileId,
    profile_data_dir: PathBuf,
) -> Result<super::SnapshotArgs, SidecarArgsError> {
    Ok(snapshot_args(parse_command(snapshot_command_args(profile_id, profile_data_dir))?))
}

fn snapshot_args(command: SidecarCommand) -> super::SnapshotArgs {
    match command {
        SidecarCommand::Snapshot(args) => args,
        SidecarCommand::Live(_) => unreachable!("expected snapshot command"),
    }
}

fn snapshot_command_args(profile_id: &ProfileId, profile_data_dir: PathBuf) -> Vec<String> {
    [
        "ely_servo_sidecar".to_string(),
        "snapshot".to_string(),
        "--url".to_string(),
        "https://example.com".to_string(),
        "--profile-id".to_string(),
        profile_id.as_str().to_string(),
        "--profile-data-dir".to_string(),
        profile_data_dir.display().to_string(),
        "--rgba-out".to_string(),
        "/tmp/frame.rgba".to_string(),
        "--width".to_string(),
        "64".to_string(),
        "--height".to_string(),
        "64".to_string(),
    ]
    .into_iter()
    .collect()
}

fn parse_live(extra_args: &[&str]) -> Result<super::LiveArgs, SidecarArgsError> {
    let base = ["ely_servo_sidecar", "live", "--profile-data-dir", "/tmp/sidecar-live"];
    let argv: Vec<String> =
        base.iter().chain(extra_args.iter()).map(|s| (*s).to_string()).collect();
    let SidecarCommand::Live(args) = parse_command(argv)? else {
        return Err(SidecarArgsError::UnknownCommand {
            value: "live-extracted-as-snapshot".into(),
        });
    };
    Ok(args)
}

#[test]
fn live_defaults_rendering_context_to_software() -> Result<(), SidecarArgsError> {
    let args = parse_live(&[])?;
    assert_eq!(args.rendering_context_kind, RenderingContextKind::Software);
    Ok(())
}

#[test]
fn live_accepts_explicit_software_rendering_context() -> Result<(), SidecarArgsError> {
    let args = parse_live(&["--rendering-context", "software"])?;
    assert_eq!(args.rendering_context_kind, RenderingContextKind::Software);
    Ok(())
}

#[test]
fn live_accepts_explicit_hardware_rendering_context() -> Result<(), SidecarArgsError> {
    let args = parse_live(&["--rendering-context", "hardware"])?;
    assert_eq!(args.rendering_context_kind, RenderingContextKind::Hardware);
    Ok(())
}

#[test]
fn live_accepts_iosurface_mach_service_name() -> Result<(), SidecarArgsError> {
    let args = parse_live(&["--iosurface-mach-service", "com.ely.test.iosurface"])?;
    assert_eq!(args.iosurface_mach_service.as_deref(), Some("com.ely.test.iosurface"));
    Ok(())
}

#[test]
fn live_rejects_unknown_rendering_context_value() {
    assert!(matches!(
        parse_live(&["--rendering-context", "gpu"]),
        Err(SidecarArgsError::InvalidRenderingContext { value }) if value == "gpu"
    ));
}

#[test]
fn live_requires_rendering_context_value() {
    assert!(matches!(
        parse_live(&["--rendering-context"]),
        Err(SidecarArgsError::MissingArgumentValue { name: "--rendering-context" })
    ));
}

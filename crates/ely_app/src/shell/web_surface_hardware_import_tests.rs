use std::{
    env,
    error::Error,
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use core_video::pixel_buffer::kCVPixelFormatType_32BGRA;
use ely_domain::{BrowserTab, ProfileId, SpaceId, TabId, UrlText};
use gpui::{Bounds, SurfaceLease, point, px, size, submit_surface_to_metal_for_test};
use objc2_io_surface::IOSurfaceRef;

use crate::{
    services::ProfileDataMode,
    shell::web_surface_frame::WebSurfaceFrame,
    shell::web_surface_state::{WebSurfaceInputOutcome, WebSurfaceState},
};

use super::{WebSurfaceStore, web_surface_profile_isolation_tests::ProfileProbeServer};

const CHILD_ENV: &str = "ELY_APP_HARDWARE_IOSURFACE_CHILD";
const TEST_NAME: &str = concat!(
    "shell::web_surface::web_surface_hardware_import_tests::",
    "web_surface_imports_hardware_iosurface",
);
const PROBE_TITLE: &str = "request=empty|document=hardware|storage=hardware|cache=cache-1";
const PROBE_WIDTH: u32 = 640;
const PROBE_HEIGHT: u32 = 480;
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

#[test]
fn web_surface_imports_hardware_iosurface() -> Result<(), Box<dyn Error>> {
    if env::var_os(CHILD_ENV).is_some() {
        return run_hardware_iosurface_probe();
    }

    let sidecar = build_hardware_sidecar()?;
    let output = Command::new(env::current_exe()?)
        .arg(TEST_NAME)
        .arg("--exact")
        .arg("--test-threads=1")
        .env(CHILD_ENV, "1")
        .env("ELY_SERVO_RENDERING_CONTEXT", "hardware")
        .env("ELY_SERVO_SIDECAR", sidecar)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    if output.status.success() && stdout.contains("running 1 test") {
        return Ok(());
    }

    Err(format!(
        "isolated hardware IOSurface test failed\nstatus: {}\nstdout: {}\nstderr: {}",
        output.status,
        stdout,
        String::from_utf8_lossy(&output.stderr),
    )
    .into())
}

fn build_hardware_sidecar() -> Result<PathBuf, Box<dyn Error>> {
    let workspace_manifest = PathBuf::from(env!("ELY_WORKSPACE_MANIFEST"));
    let workspace_root = workspace_manifest.parent().ok_or("missing workspace root")?;
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let output = Command::new(cargo)
        .args([
            "build",
            "--locked",
            "-p",
            "ely_servo_host",
            "--features",
            "servo-engine,hardware-render",
            "--bin",
            "ely_servo_sidecar",
        ])
        .current_dir(workspace_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "hardware sidecar build failed\nstatus: {}\nstdout: {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )
        .into());
    }

    let sidecar = sidecar_binary_path(workspace_root);
    if !sidecar.is_file() {
        return Err(format!("hardware sidecar is missing at {}", sidecar.display()).into());
    }
    Ok(sidecar)
}

fn sidecar_binary_path(workspace_root: &Path) -> PathBuf {
    let mut target_dir = env::var_os("CARGO_TARGET_DIR").map_or_else(
        || workspace_root.join("target"),
        |path| {
            let path = PathBuf::from(path);
            if path.is_absolute() { path } else { workspace_root.join(path) }
        },
    );
    if let Some(target) = env::var_os("CARGO_BUILD_TARGET") {
        target_dir.push(target);
    }
    target_dir.join("debug").join(format!("ely_servo_sidecar{}", env::consts::EXE_SUFFIX))
}

fn run_hardware_iosurface_probe() -> Result<(), Box<dyn Error>> {
    let mut server = ProfileProbeServer::start()?;
    let mut store = WebSurfaceStore::new();
    let url = format!("{}/probe?value=hardware", server.origin());
    let tab = BrowserTab::new(
        TabId::new(),
        SpaceId::new(),
        ProfileId::new(),
        "Hardware IOSurface probe",
        UrlText::parse(&url)?,
    );
    assert_eq!(
        store.record_viewport_size(tab.id(), probe_bounds(), 1.0),
        WebSurfaceInputOutcome::Applied,
    );
    assert!(store.ensure_surface(&tab, ProfileDataMode::Transient, &[]));

    let frame = wait_for_hardware_surface(&mut store, &tab)?;
    store.close_surface(tab.id());
    store.flush_runtime_for_test();
    assert_hardware_lease_lifecycle(frame)?;
    drop(store);
    server.finish()
}

fn wait_for_hardware_surface(
    store: &mut WebSurfaceStore,
    tab: &BrowserTab,
) -> Result<WebSurfaceFrame, Box<dyn Error>> {
    let started_at = Instant::now();
    let mut last_frame = None;
    loop {
        store.tick(std::slice::from_ref(tab.id()));
        match store.state(tab.id()) {
            Some(WebSurfaceState::Ready(frame)) => {
                last_frame = Some(format!(
                    "title={:?}, state={}, hardware={}",
                    frame.title(),
                    frame.render_state(),
                    frame.has_hardware_surface(),
                ));
                if frame.title() == Some(PROBE_TITLE) && frame.has_hardware_surface() {
                    return Ok(frame.clone());
                }
            }
            Some(WebSurfaceState::Failed { message }) => {
                return Err(format!("hardware IOSurface probe failed: {message}").into());
            }
            Some(WebSurfaceState::Loading { .. }) | None => {}
        }
        if started_at.elapsed() >= PROBE_TIMEOUT {
            return Err(format!(
                "timed out waiting for imported hardware IOSurface; last frame: {last_frame:?}",
            )
            .into());
        }
        thread::sleep(Duration::from_millis(2));
    }
}

fn assert_hardware_lease_lifecycle(frame: WebSurfaceFrame) -> Result<(), Box<dyn Error>> {
    let hardware_surface = frame.hardware_surface.clone().ok_or("hardware backing is missing")?;
    let pixel_buffer = hardware_surface.pixel_buffer();
    assert_eq!(pixel_buffer.get_pixel_format(), kCVPixelFormatType_32BGRA);

    let surface_id = frame
        .hardware_surface_id_for_test()
        .and_then(|surface_id| u32::try_from(surface_id).ok())
        .ok_or("hardware IOSurface ID is invalid")?;
    let iosurface = IOSurfaceRef::lookup(surface_id).ok_or("hardware IOSurface lookup failed")?;
    let active_use_count = iosurface.use_count();
    let released_use_count = active_use_count
        .checked_sub(1)
        .ok_or("hardware IOSurface use count was not incremented")?;
    assert_eq!(Arc::strong_count(&hardware_surface), 2);

    let weak_backing = Arc::downgrade(&hardware_surface);
    let submission = submit_surface_to_metal_for_test(
        pixel_buffer.clone(),
        SurfaceLease::from_arc(hardware_surface.clone()),
    )?;
    drop(frame);
    drop(hardware_surface);

    assert_eq!(weak_backing.strong_count(), 1);
    assert_eq!(iosurface.use_count(), active_use_count);
    submission.finish()?;

    assert_eq!(weak_backing.strong_count(), 0);
    assert_eq!(iosurface.use_count(), released_use_count);
    Ok(())
}

fn probe_bounds() -> Bounds<gpui::Pixels> {
    Bounds::new(point(px(0.0), px(0.0)), size(px(PROBE_WIDTH as f32), px(PROBE_HEIGHT as f32)))
}

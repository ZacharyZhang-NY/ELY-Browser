#![cfg(feature = "hardware-render")]

use dpi::PhysicalSize;
use ely_servo_host::{HardwareOffscreenContext, IOSurfaceHandle, IOSurfaceIdentity};
use servo::RenderingContext;

#[cfg(target_os = "macos")]
const HARDWARE_HOST_CHILD_ENV: &str = "ELY_SERVO_HARDWARE_HOST_CHILD";

#[test]
fn identity_uses_handle_dimensions() {
    let handle = IOSurfaceHandle { mach_port_name: 7, surface_id: 11, width: 640, height: 480 };

    assert_eq!(
        IOSurfaceIdentity::from_handle(handle),
        IOSurfaceIdentity { surface_id: 11, width: 640, height: 480 }
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn hardware_context_constructs() -> Result<(), String> {
    let context = HardwareOffscreenContext::new(PhysicalSize::new(64, 64))
        .map_err(|error| format!("hardware context creation failed: {error:?}"))?;
    assert_eq!(context.size(), PhysicalSize::new(64, 64));
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn presented_surface_exposes_iosurface_identity_and_mach_port() -> Result<(), String> {
    let size = PhysicalSize::new(256, 192);
    let context = HardwareOffscreenContext::new(size)
        .map_err(|error| format!("hardware context creation failed: {error:?}"))?;

    assert_eq!(
        context
            .peek_iosurface_identity()
            .map_err(|error| format!("identity probe failed: {error:?}"))?,
        None
    );
    context.make_current().map_err(|error| format!("make current failed: {error:?}"))?;
    context.prepare_for_rendering();
    context.present();

    let identity = context
        .peek_iosurface_identity()
        .map_err(|error| format!("identity probe failed: {error:?}"))?
        .ok_or_else(|| "present did not expose an IOSurface".to_string())?;
    let handle = context
        .current_iosurface_mach_port()
        .map_err(|error| format!("Mach port creation failed: {error:?}"))?;

    assert_ne!(handle.mach_port_name, 0);
    assert_eq!(identity, IOSurfaceIdentity::from_handle(handle));
    assert_eq!(identity.width, size.width);
    assert_eq!(identity.height, size.height);
    assert_eq!(deallocate_mach_port(handle.mach_port_name), mach2::kern_return::KERN_SUCCESS);
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn hardware_host_paints_and_presents_an_iosurface() -> Result<(), Box<dyn std::error::Error>> {
    use std::env;
    use std::process::{Command, Stdio};

    if env::var_os(HARDWARE_HOST_CHILD_ENV).is_some() {
        return exercise_hardware_host();
    }

    let output = Command::new(env::current_exe()?)
        .arg("--exact")
        .arg("hardware_host_paints_and_presents_an_iosurface")
        .env(HARDWARE_HOST_CHILD_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    if output.status.success() {
        return Ok(());
    }

    Err(format!(
        "hardware host child failed\nstatus: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .into())
}

#[cfg(target_os = "macos")]
fn exercise_hardware_host() -> Result<(), Box<dyn std::error::Error>> {
    use std::thread;
    use std::time::Duration;

    use ely_domain::{ProfileId, TabId, UrlText};
    use ely_servo_host::{
        NavigationRequest, RenderingContextKind, ServoHost, ServoSurfaceSize, SoftwareServoHost,
    };

    let size = ServoSurfaceSize::new(320, 240);
    let mut host = SoftwareServoHost::new_with_config_dir_and_kind(
        size,
        None,
        RenderingContextKind::Hardware,
    )?;
    let tab_id = TabId::new();
    let webview_id = host.create_webview(tab_id.clone(), ProfileId::new())?;
    host.navigate(NavigationRequest {
        webview_id: webview_id.clone(),
        tab_id,
        url: UrlText::parse("data:text/html,%3Cbody%20style%3D%27background%3A%230369a1%27%3E")?,
    })?;

    for _ in 0..5_000 {
        host.tick();
        if host.snapshot(&webview_id)?.has_pending_frame() {
            host.paint_without_readback(&webview_id)?;
            if host.peek_iosurface_identity(&webview_id)?.is_some() {
                let handle = host
                    .current_iosurface_handle(&webview_id)?
                    .ok_or("hardware webview did not expose an IOSurface handle")?;
                assert_eq!(handle.width, 320);
                assert_eq!(handle.height, 240);
                assert_eq!(
                    deallocate_mach_port(handle.mach_port_name),
                    mach2::kern_return::KERN_SUCCESS
                );
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(2));
    }

    Err("timed out waiting for a hardware IOSurface frame".into())
}

#[cfg(target_os = "macos")]
#[expect(unsafe_code)]
fn deallocate_mach_port(name: u32) -> i32 {
    // `name` is a live send right minted by IOSurfaceCreateMachPort in this task.
    unsafe { mach2::mach_port::mach_port_deallocate(mach2::traps::mach_task_self(), name) }
}

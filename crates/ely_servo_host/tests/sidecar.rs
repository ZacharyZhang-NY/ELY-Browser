#![cfg(feature = "servo-engine")]

use std::{
    error::Error,
    io::{self, BufRead, BufReader, Write},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use ely_domain::{ProfileId, TabId};
use ely_servo_host::MAX_REQUEST_LINE_BYTES;
use serde_json::{Value, json};

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
#[path = "sidecar/mach_receiver.rs"]
mod mach_receiver;
#[path = "sidecar/pages.rs"]
mod pages;
#[path = "sidecar/support.rs"]
mod support;

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
use mach_receiver::{MachSurfaceReceiver, verify_iosurface};
use support::{
    HEIGHT, LIVE_PROTOCOL_VERSION, MAX_FRAME_DIMENSION, RESPONSE_TIMEOUT, Sidecar, TestDirectory,
    TestServer, WIDTH, ensure_request,
};

#[test]
fn live_sidecar_streams_rgba_and_flushes_profile_storage_on_shutdown() -> Result<(), Box<dyn Error>>
{
    let server = TestServer::start()?;
    let root = TestDirectory::new()?;
    let persisted_dir = root.path().join("persisted");
    let fresh_dir = root.path().join("fresh");
    let profile_id = ProfileId::new();

    let mut writer = Sidecar::spawn(&persisted_dir)?;
    let stored = writer
        .ensure_and_wait_visible(&profile_id, &server.url("/set"), "stored-cookie-yes-storage-yes")
        .map_err(|error| io::Error::other(format!("{error}; server={}", server.diagnostics())))?;
    assert_eq!(stored.width, WIDTH);
    assert_eq!(stored.height, HEIGHT);
    assert!(stored.non_white_pixel_count > 0);
    assert!(stored.content_pixel_count > 0);
    assert_ne!(stored.sample_hash, 0);
    writer.shutdown()?;

    assert!(persisted_dir.join("cookie_jar.json").is_file());
    assert!(persisted_dir.join("localstorage.json").is_file());
    assert!(persisted_dir.join("webstorage").is_dir());

    let mut reader = Sidecar::spawn(&persisted_dir)?;
    reader.ensure_and_wait_visible(
        &profile_id,
        &server.url("/read"),
        "read-cookie-yes-storage-yes",
    )?;
    reader.shutdown()?;

    let mut fresh = Sidecar::spawn(&fresh_dir)?;
    fresh.ensure_and_wait_visible(
        &profile_id,
        &server.url("/read"),
        "read-cookie-no-storage-no",
    )?;
    fresh.shutdown()?;
    Ok(())
}

#[test]
fn live_sidecar_leases_profile_data_directory_for_process_lifetime() -> Result<(), Box<dyn Error>> {
    let root = TestDirectory::new()?;
    let leased_dir = root.path().join("leased");
    let independent_dir = root.path().join("independent");
    let mut owner = Sidecar::spawn(&leased_dir)?;
    assert!(leased_dir.join(".ely-servo-sidecar.lock").is_file());

    let rejected = Command::new(env!("CARGO_BIN_EXE_ely_servo_sidecar"))
        .arg("live")
        .arg("--profile-data-dir")
        .arg(&leased_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()?;
    let stderr = String::from_utf8(rejected.stderr)?;
    assert!(!rejected.status.success(), "second sidecar unexpectedly acquired the profile lease");
    assert!(
        stderr.contains("ProfileDataDirectoryInUse"),
        "second sidecar reported an unexpected error: {stderr}"
    );

    let mut independent = Sidecar::spawn(&independent_dir)?;
    independent.shutdown()?;
    owner.shutdown()?;

    let mut replacement = Sidecar::spawn(&leased_dir)?;
    replacement.shutdown()?;
    Ok(())
}

#[test]
fn servo_originated_history_url_does_not_trigger_a_second_navigation() -> Result<(), Box<dyn Error>>
{
    let server = TestServer::start()?;
    let root = TestDirectory::new()?;
    let profile_id = ProfileId::new();
    let tab_id = TabId::new();
    let initial_url = server.url("/history");
    let history_url = server.url("/history?state=1");
    let mut sidecar = Sidecar::spawn(root.path())?;

    let ensure = ensure_request(&tab_id, &profile_id, &initial_url);
    let mut response = sidecar.exchange(&ensure)?;
    let started_at = Instant::now();
    loop {
        if let Some(error) = response.error {
            return Err(io::Error::other(format!("sidecar response error: {error}")).into());
        }
        if response.frame.as_ref().is_some_and(|frame| {
            frame.loaded_url.as_deref() == Some(history_url.as_str())
                && frame.title.as_deref() == Some("history-ready")
        }) {
            break;
        }
        if started_at.elapsed() >= RESPONSE_TIMEOUT {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("timed out waiting for history URL {history_url}"),
            )
            .into());
        }
        thread::sleep(Duration::from_millis(2));
        response = sidecar.exchange(&json!({ "type": "poll", "tab_id": tab_id.as_str() }))?;
    }

    sidecar.exchange(&ensure_request(&tab_id, &profile_id, &history_url))?;
    for _ in 0..20 {
        thread::sleep(Duration::from_millis(5));
        sidecar.exchange(&json!({ "type": "poll", "tab_id": tab_id.as_str() }))?;
    }

    assert_eq!(server.request_count("/history"), 1, "{}", server.diagnostics());
    sidecar.shutdown()?;
    Ok(())
}

#[test]
fn live_sidecar_delivers_a_valid_white_page() -> Result<(), Box<dyn Error>> {
    let server = TestServer::start()?;
    let root = TestDirectory::new()?;
    let profile_id = ProfileId::new();
    let mut sidecar = Sidecar::spawn(root.path())?;

    let frame = sidecar.ensure_and_wait(&profile_id, &server.url("/white"), "white-ready")?;

    assert_eq!(frame.non_white_pixel_count, 0);
    assert_eq!(frame.content_pixel_count, 0);
    sidecar.shutdown()?;
    Ok(())
}

#[test]
fn live_sidecar_gates_rsa_private_operations() -> Result<(), Box<dyn Error>> {
    let server = TestServer::start()?;
    let root = TestDirectory::new()?;
    let mut sidecar = Sidecar::spawn(root.path())?;

    sidecar.ensure_and_wait(
        &ProfileId::new(),
        &server.url("/rsa-private-operations"),
        "rsa-private-operations-gated",
    )?;

    sidecar.shutdown()?;
    Ok(())
}

#[test]
fn live_sidecar_rejects_an_incompatible_protocol() -> Result<(), Box<dyn Error>> {
    let root = TestDirectory::new()?;
    let mut sidecar = Sidecar::spawn_without_handshake(root.path())?;

    let response = sidecar.exchange(&json!({
        "type": "handshake",
        "protocol_version": LIVE_PROTOCOL_VERSION + 1,
    }))?;

    assert_eq!(response.protocol_version, Some(LIVE_PROTOCOL_VERSION));
    assert!(response.error.as_deref().is_some_and(|error| error.contains("protocol mismatch")));
    Ok(())
}

#[test]
fn live_sidecar_drains_an_oversized_request_line_and_recovers_the_handshake()
-> Result<(), Box<dyn Error>> {
    let root = TestDirectory::new()?;
    let mut child = Command::new(env!("CARGO_BIN_EXE_ely_servo_sidecar"))
        .arg("live")
        .arg("--profile-data-dir")
        .arg(root.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdin = child.stdin.take().ok_or_else(|| io::Error::other("missing stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| io::Error::other("missing stdout"))?;
    let mut stdout = BufReader::new(stdout);
    let mut oversized = br#"{"type":"handshake","protocol_version":4}"#.to_vec();
    oversized.resize(MAX_REQUEST_LINE_BYTES + 4_096, b' ');
    oversized.push(b'\n');

    stdin.write_all(&oversized)?;
    serde_json::to_writer(
        &mut stdin,
        &json!({"type": "handshake", "protocol_version": LIVE_PROTOCOL_VERSION}),
    )?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;

    let mut line = String::new();
    stdout.read_line(&mut line)?;
    let oversized_response: Value = serde_json::from_str(&line)?;
    assert_eq!(
        oversized_response["error"],
        format!("live request line exceeds the {MAX_REQUEST_LINE_BYTES}-byte protocol limit")
    );

    line.clear();
    stdout.read_line(&mut line)?;
    let handshake_response: Value = serde_json::from_str(&line)?;
    assert_eq!(handshake_response["protocol_version"], LIVE_PROTOCOL_VERSION);
    assert!(handshake_response["error"].is_null());

    serde_json::to_writer(&mut stdin, &json!({"type": "shutdown"}))?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    line.clear();
    stdout.read_line(&mut line)?;
    let shutdown_response: Value = serde_json::from_str(&line)?;
    assert!(shutdown_response["error"].is_null());
    drop(stdin);
    assert!(child.wait()?.success());
    Ok(())
}

#[test]
fn live_sidecar_rejects_oversized_frame_dimensions() -> Result<(), Box<dyn Error>> {
    let root = TestDirectory::new()?;
    let profile_id = ProfileId::new();
    let tab_id = TabId::new();
    let mut sidecar = Sidecar::spawn(root.path())?;
    let mut ensure = ensure_request(&tab_id, &profile_id, "about:blank");
    ensure["width"] = json!(MAX_FRAME_DIMENSION + 1);

    let response = sidecar.exchange(&ensure)?;

    assert!(response.error.as_deref().is_some_and(|error| error.contains("dimension limit")));
    sidecar.shutdown()?;
    Ok(())
}

#[test]
fn live_sidecar_rejects_overlong_request_url_without_exiting() -> Result<(), Box<dyn Error>> {
    let root = TestDirectory::new()?;
    let profile_id = ProfileId::new();
    let tab_id = TabId::new();
    let mut sidecar = Sidecar::spawn(root.path())?;
    let overlong_url = format!("https://example.com/{}", "a".repeat(32 * 1024));

    let response = sidecar.exchange(&ensure_request(&tab_id, &profile_id, &overlong_url))?;

    assert!(response.frame.is_none());
    assert!(
        response
            .error
            .as_deref()
            .is_some_and(|error| error == "request URL exceeds the 32768-byte live protocol limit")
    );
    sidecar.shutdown()?;
    Ok(())
}

#[test]
fn live_sidecar_retires_a_session_with_an_overlong_loaded_url() -> Result<(), Box<dyn Error>> {
    let server = TestServer::start()?;
    let root = TestDirectory::new()?;
    let profile_id = ProfileId::new();
    let tab_id = TabId::new();
    let mut sidecar = Sidecar::spawn(root.path())?;
    let mut response = sidecar.exchange(&ensure_request(
        &tab_id,
        &profile_id,
        &server.url("/oversized-history"),
    ))?;
    let started_at = Instant::now();

    loop {
        if response.error.as_deref()
            == Some("loaded URL exceeds the 32768-byte live protocol limit")
        {
            assert!(response.frame.is_none());
            break;
        }
        if started_at.elapsed() >= RESPONSE_TIMEOUT {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "timed out waiting for oversized loaded URL rejection",
            )
            .into());
        }
        thread::sleep(Duration::from_millis(2));
        response = sidecar.exchange(&json!({ "type": "poll", "tab_id": tab_id.as_str() }))?;
    }

    let mut response =
        sidecar.exchange(&ensure_request(&tab_id, &profile_id, &server.url("/white")))?;
    let started_at = Instant::now();
    while response.frame.as_ref().and_then(|frame| frame.title.as_deref()) != Some("white-ready") {
        if let Some(error) = response.error {
            return Err(io::Error::other(format!("replacement session failed: {error}")).into());
        }
        if started_at.elapsed() >= RESPONSE_TIMEOUT {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "timed out waiting for replacement session",
            )
            .into());
        }
        thread::sleep(Duration::from_millis(2));
        response = sidecar.exchange(&json!({ "type": "poll", "tab_id": tab_id.as_str() }))?;
    }

    sidecar.shutdown()?;
    Ok(())
}

#[test]
fn live_sidecar_rejects_duplicate_permission_snapshot_entries() -> Result<(), Box<dyn Error>> {
    let root = TestDirectory::new()?;
    let profile_id = ProfileId::new();
    let tab_id = TabId::new();
    let mut sidecar = Sidecar::spawn(root.path())?;
    let mut ensure = ensure_request(&tab_id, &profile_id, "about:blank");
    ensure["site_permissions"] = json!([
        {
            "origin": "https://example.com",
            "feature": "camera",
            "state": "allow-always",
            "revision": 1,
        },
        {
            "origin": "https://example.com",
            "feature": "camera",
            "state": "deny-always",
            "revision": 2,
        },
    ]);

    let response = sidecar.exchange(&ensure)?;

    assert!(response.error.as_deref().is_some_and(|error| error.contains("duplicate permission")));
    sidecar.shutdown()?;
    Ok(())
}

#[test]
fn live_sidecar_accepts_permission_snapshot_lifecycle() -> Result<(), Box<dyn Error>> {
    let root = TestDirectory::new()?;
    let profile_id = ProfileId::new();
    let tab_id = TabId::new();
    let mut sidecar = Sidecar::spawn(root.path())?;
    let mut ensure = ensure_request(&tab_id, &profile_id, "about:blank");

    for (generation, state, revision) in [(1, "allow-once", 1), (2, "transferred-allow-once", 2)] {
        ensure["site_permission_generation"] = json!(generation);
        ensure["site_permissions"] = json!([{
            "origin": "https://example.com",
            "feature": "camera",
            "state": state,
            "revision": revision,
        }]);
        let response = sidecar.exchange(&ensure)?;
        assert!(response.error.is_none(), "state={state} error={:?}", response.error);
    }

    ensure["site_permission_generation"] = json!(3);
    ensure["site_permissions"] = json!([]);
    let response = sidecar.exchange(&ensure)?;
    assert!(response.error.is_none(), "empty snapshot error={:?}", response.error);

    ensure["site_permission_generation"] = json!(1);
    ensure["site_permissions"] = json!([{
        "origin": "https://example.com",
        "feature": "camera",
        "state": "allow-once",
        "revision": 1,
    }]);
    let response = sidecar.exchange(&ensure)?;
    assert!(response.error.is_none(), "stale snapshot error={:?}", response.error);

    sidecar.shutdown()?;
    Ok(())
}

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
#[test]
fn hardware_sidecar_transfers_a_real_iosurface_mach_descriptor() -> Result<(), Box<dyn Error>> {
    let receiver = MachSurfaceReceiver::new()?;
    let server = TestServer::start()?;
    let root = TestDirectory::new()?;
    let profile_id = ProfileId::new();
    let tab_id = TabId::new();
    let mut sidecar = Sidecar::spawn_hardware(root.path(), receiver.service_name())?;
    let mut response =
        sidecar.exchange(&ensure_request(&tab_id, &profile_id, &server.url("/white")))?;
    let started_at = Instant::now();

    let imported_surface_id = loop {
        if let Some(error) = response.error {
            return Err(io::Error::other(format!("hardware sidecar error: {error}")).into());
        }
        if let (Some(frame), Some(handle), Some(current_surface_id)) =
            (response.frame.as_ref(), response.surface_handle, response.current_surface_id)
        {
            assert_eq!(frame.rgba_byte_count, 0);
            assert_eq!(current_surface_id, handle.surface_id);
            assert_eq!((frame.width, frame.height), (handle.width, handle.height));
            assert_ne!(handle.mach_port_name, 0);
            let received_port = receiver.receive(handle.surface_id, Duration::from_secs(2))?;
            verify_iosurface(received_port, handle.width, handle.height)?;
            break handle.surface_id;
        }
        if started_at.elapsed() >= RESPONSE_TIMEOUT {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "timed out waiting for a hardware IOSurface frame",
            )
            .into());
        }
        thread::sleep(Duration::from_millis(2));
        response = sidecar.exchange(&json!({
            "type": "poll",
            "tab_id": tab_id.as_str(),
            "ready_surface_ids": [],
            "pending_surface_ids": [],
        }))?;
    };

    let replay = sidecar.exchange(&json!({
        "type": "poll",
        "tab_id": tab_id.as_str(),
        "ready_surface_ids": [imported_surface_id],
        "pending_surface_ids": [],
    }))?;
    assert_eq!(replay.current_surface_id, Some(imported_surface_id));
    assert!(replay.surface_handle.is_none());
    assert_eq!(replay.frame.as_ref().map(|frame| frame.rgba_byte_count), Some(0));

    let republished = sidecar.exchange(&json!({
        "type": "poll",
        "tab_id": tab_id.as_str(),
        "ready_surface_ids": [],
        "pending_surface_ids": [],
    }))?;
    let republished_handle = republished
        .surface_handle
        .ok_or_else(|| io::Error::other("evicted IOSurface was not republished"))?;
    assert_eq!(republished.current_surface_id, Some(imported_surface_id));
    assert_eq!(republished_handle.surface_id, imported_surface_id);
    let republished_port = receiver.receive(imported_surface_id, Duration::from_secs(2))?;
    verify_iosurface(republished_port, republished_handle.width, republished_handle.height)?;

    let pending = sidecar.exchange(&json!({
        "type": "poll",
        "tab_id": tab_id.as_str(),
        "ready_surface_ids": [],
        "pending_surface_ids": [imported_surface_id],
    }))?;
    assert!(pending.frame.is_none());
    assert!(pending.surface_handle.is_none());

    let reimported = sidecar.exchange(&json!({
        "type": "poll",
        "tab_id": tab_id.as_str(),
        "ready_surface_ids": [imported_surface_id],
        "pending_surface_ids": [],
    }))?;
    assert_eq!(reimported.current_surface_id, Some(imported_surface_id));
    assert!(reimported.surface_handle.is_none());
    assert_eq!(reimported.frame.as_ref().map(|frame| frame.rgba_byte_count), Some(0));

    sidecar.shutdown()?;
    Ok(())
}

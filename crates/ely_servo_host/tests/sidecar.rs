#![cfg(feature = "servo-engine")]

use std::{error::Error, process::Command};

const WIDTH: u64 = 640;
const HEIGHT: u64 = 480;

#[test]
fn sidecar_snapshots_prd_site_to_rgba_file() -> Result<(), Box<dyn Error>> {
    let output_path =
        std::env::temp_dir().join(format!("ely-servo-sidecar-{}-example.rgba", std::process::id()));
    if output_path.exists() {
        std::fs::remove_file(&output_path)?;
    }

    let output = Command::new(env!("CARGO_BIN_EXE_ely_servo_sidecar"))
        .arg("snapshot")
        .arg("--url")
        .arg("https://example.com")
        .arg("--rgba-out")
        .arg(&output_path)
        .arg("--width")
        .arg(WIDTH.to_string())
        .arg("--height")
        .arg(HEIGHT.to_string())
        .output()?;

    assert!(
        output.status.success(),
        "status: {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report_field_as_u64(&report, "width")?, WIDTH);
    assert_eq!(report_field_as_u64(&report, "height")?, HEIGHT);
    assert_eq!(report_field_as_u64(&report, "rgba_byte_count")?, WIDTH * HEIGHT * 4);
    assert!(report_field_as_u64(&report, "non_white_pixel_count")? > 0);
    assert!(report_field_as_u64(&report, "sample_hash")? > 0);
    assert_eq!(std::fs::metadata(&output_path)?.len(), WIDTH * HEIGHT * 4);

    std::fs::remove_file(&output_path)?;
    Ok(())
}

fn report_field_as_u64(
    report: &serde_json::Value,
    field: &'static str,
) -> Result<u64, Box<dyn Error>> {
    report
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| format!("missing numeric report field: {field}").into())
}

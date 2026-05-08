#![cfg(feature = "servo-engine")]

use std::{error::Error, process::Command};

const WIDTH: u64 = 640;
const HEIGHT: u64 = 480;
const PRD_SITE_COMPATIBILITY_URLS: &[&str] = &["https://example.com", "https://servo.org"];

#[test]
fn sidecar_snapshots_prd_sites_to_rgba_files() -> Result<(), Box<dyn Error>> {
    for site_url in PRD_SITE_COMPATIBILITY_URLS {
        snapshot_prd_site(site_url)?;
    }

    Ok(())
}

fn snapshot_prd_site(site_url: &str) -> Result<(), Box<dyn Error>> {
    let site_name = site_url
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() { character } else { '-' })
        .collect::<String>();
    let output_path = std::env::temp_dir()
        .join(format!("ely-servo-sidecar-{}-{site_name}.rgba", std::process::id()));

    if output_path.exists() {
        std::fs::remove_file(&output_path)?;
    }

    let output = Command::new(env!("CARGO_BIN_EXE_ely_servo_sidecar"))
        .arg("snapshot")
        .arg("--url")
        .arg(site_url)
        .arg("--rgba-out")
        .arg(&output_path)
        .arg("--width")
        .arg(WIDTH.to_string())
        .arg("--height")
        .arg(HEIGHT.to_string())
        .output()?;

    assert!(
        output.status.success(),
        "{site_url}\nstatus: {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report_field_as_u64(&report, "width")?, WIDTH, "{site_url}");
    assert_eq!(report_field_as_u64(&report, "height")?, HEIGHT, "{site_url}");
    assert_eq!(report_field_as_u64(&report, "rgba_byte_count")?, WIDTH * HEIGHT * 4, "{site_url}");
    assert!(report_field_as_u64(&report, "non_white_pixel_count")? > 0, "{site_url}");
    assert!(report_field_as_u64(&report, "sample_hash")? > 0, "{site_url}");
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

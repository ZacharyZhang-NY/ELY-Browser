use std::{
    env,
    error::Error,
    fs,
    io::BufReader,
    process::{ChildStdin, ChildStdout},
};

use ely_domain::{ProfileId, TabId};

use super::{
    RESPONSE_TIMEOUT, build_ensure, cleanup, read_response_with_bytes, spawn_sidecar, write_request,
};

const SOLID_RED_DATA_URL: &str =
    "data:text/html,<body style=\"margin:0;background:%23ff0000;height:4000px\">";
const SOLID_BLUE_DATA_URL: &str =
    "data:text/html,<body style=\"margin:0;background:%230000ff;height:4000px\">";

#[test]
#[ignore = "drives a real sidecar via stdin/stdout; takes a few seconds"]
fn red_data_url_yields_red_rgba() -> Result<(), Box<dyn Error>> {
    assert_solid_color_renders("software", SOLID_RED_DATA_URL, ColorTarget::Red)
}

#[test]
#[ignore = "drives a real sidecar via stdin/stdout; takes a few seconds"]
fn blue_data_url_yields_blue_rgba() -> Result<(), Box<dyn Error>> {
    assert_solid_color_renders("software", SOLID_BLUE_DATA_URL, ColorTarget::Blue)
}

#[derive(Clone, Copy)]
enum ColorTarget {
    Red,
    Blue,
}

impl ColorTarget {
    fn label(self) -> &'static str {
        match self {
            ColorTarget::Red => "red",
            ColorTarget::Blue => "blue",
        }
    }
}

fn assert_solid_color_renders(
    kind: &str,
    url: &str,
    target: ColorTarget,
) -> Result<(), Box<dyn Error>> {
    let profile_id = ProfileId::new();
    let tab = TabId::new();
    let profile_data_dir = env::temp_dir().join(format!(
        "ely-pixel-{}-{}-{}",
        std::process::id(),
        target.label(),
        profile_id.as_str(),
    ));
    fs::create_dir_all(&profile_data_dir)?;

    let mut child = spawn_sidecar(kind, &profile_data_dir)?;
    let mut stdin = child.stdin.take().ok_or("sidecar stdin missing")?;
    let stdout = child.stdout.take().ok_or("sidecar stdout missing")?;
    let mut reader = BufReader::new(stdout);
    let outcome = drive_solid_color_render(&mut stdin, &mut reader, &tab, &profile_id, url, target);

    drop(stdin);
    let _ = child.wait();
    cleanup(&profile_data_dir)?;
    outcome
}

fn drive_solid_color_render(
    stdin: &mut ChildStdin,
    reader: &mut BufReader<ChildStdout>,
    tab: &TabId,
    profile_id: &ProfileId,
    url: &str,
    target: ColorTarget,
) -> Result<(), Box<dyn Error>> {
    let mut bytes = Vec::new();
    let mut report = None;
    for iteration in 0..30 {
        let scroll_y = if iteration == 0 {
            0
        } else if iteration % 2 == 1 {
            1
        } else {
            -1
        };
        let request = build_ensure(tab, profile_id, url, 0, scroll_y, false);
        write_request(stdin, &request)?;
        let (response, response_bytes) = read_response_with_bytes(reader, RESPONSE_TIMEOUT)?;
        if let Some(error) = response.error.as_ref() {
            return Err(format!("sidecar error: {error}").into());
        }
        if let Some(frame_report) = response.frame {
            if !response_bytes.is_empty()
                && sample_matches_target(
                    &response_bytes,
                    frame_report.width,
                    frame_report.height,
                    target,
                )
            {
                report = Some(frame_report);
                bytes = response_bytes;
                break;
            }
            if !response_bytes.is_empty() {
                bytes = response_bytes;
                report = Some(frame_report);
            }
        }
    }

    let report = report.ok_or("never received a frame with bytes")?;
    let width = report.width as usize;
    let height = report.height as usize;
    assert_eq!(bytes.len(), width * height * 4, "rgba byte count must match width * height * 4",);

    let mut samples = Vec::new();
    for fy in [1, 2, 3] {
        for fx in [1, 2, 3] {
            let x = width * fx / 4;
            let y = height * fy / 4;
            let idx = (y * width + x) * 4;
            samples.push((x, y, bytes[idx], bytes[idx + 1], bytes[idx + 2], bytes[idx + 3]));
        }
    }
    eprintln!("[pixel sample {}] {:?}", target.label(), samples);

    let hits =
        samples.iter().filter(|(_x, _y, r, g, b, _a)| matches_color(*r, *g, *b, target)).count();
    assert!(
        hits >= 5,
        "expected at least 5/9 center-quadrant pixels to be {} after rendering {}; got samples {:?}",
        target.label(),
        url,
        samples,
    );
    Ok(())
}

fn sample_matches_target(bytes: &[u8], width: u32, height: u32, target: ColorTarget) -> bool {
    let w = width as usize;
    let h = height as usize;
    if bytes.len() < w * h * 4 || w == 0 || h == 0 {
        return false;
    }
    let cx = w / 2;
    let cy = h / 2;
    let idx = (cy * w + cx) * 4;
    matches_color(bytes[idx], bytes[idx + 1], bytes[idx + 2], target)
}

fn matches_color(r: u8, g: u8, b: u8, target: ColorTarget) -> bool {
    match target {
        ColorTarget::Red => r >= 200 && g <= 60 && b <= 60,
        ColorTarget::Blue => r <= 60 && g <= 60 && b >= 200,
    }
}

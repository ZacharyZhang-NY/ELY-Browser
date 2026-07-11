#![cfg(all(feature = "servo-engine", target_os = "macos"))]

use std::{error::Error, thread, time::Duration};

use ely_domain::{ProfileId, TabId, UrlText};
use ely_servo_host::{
    NavigationRequest, ServoHost, ServoSurfaceSize, SoftwareServoHost, WebViewState,
};

const EXPECTED_PASS_COLOR: [u8; 3] = [31, 143, 76];

#[test]
fn macos_fallback_preserves_css_family_style_and_language() -> Result<(), Box<dyn Error>> {
    let mut host = SoftwareServoHost::new(ServoSurfaceSize::new(320, 240))?;
    let tab_id = TabId::new();
    let profile_id = ProfileId::new();
    let webview_id = host.create_webview(tab_id.clone(), profile_id)?;

    host.navigate(NavigationRequest {
        webview_id: webview_id.clone(),
        tab_id,
        url: UrlText::parse(font_probe_url())?,
    })?;

    for _ in 0..5_000 {
        host.tick();
        let snapshot = host.snapshot(&webview_id)?;
        if snapshot.has_pending_frame() {
            host.paint(&webview_id)?;
        }

        let snapshot = host.snapshot(&webview_id)?;
        if snapshot.state() == &WebViewState::Complete
            && host
                .last_rendered_frame()
                .is_ok_and(|frame| center_pixel_rgb(&frame) == EXPECTED_PASS_COLOR)
        {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(2));
    }

    let actual = host.last_rendered_frame().map(|frame| center_pixel_rgb(&frame));
    let title = host.snapshot(&webview_id)?.title().map(str::to_owned);
    Err(format!(
        "macOS font fallback probe failed with center pixel {actual:?} and title {title:?}"
    )
    .into())
}

fn font_probe_url() -> String {
    let html = r#"<!doctype html>
<html><head><meta charset="utf-8"><title>starting</title><style>
html,body{margin:0;width:100%;height:100%;background:rgb(181,37,46)}
canvas{position:absolute;left:-10000px;top:-10000px}
</style></head><body><script>
try {
const cases = [
  ['zh-Hans', '搜索中文', '48px Arial, sans-serif', "48px 'PingFang SC'"],
  ['ja', '日本語検索', '48px Arial, sans-serif', "48px 'Hiragino Sans'"],
  ['ko', '한국어검색', '48px Arial, sans-serif', "48px 'Apple SD Gothic Neo'"],
  ['hi', 'हिन्दी', '48px Arial, sans-serif', "48px 'Kohinoor Devanagari'"],
  ['zh-Hans', '搜索中文', '48px Times New Roman, serif', "48px 'Songti SC'"]
];
function pixels(lang, text, font) {
  const canvas = document.createElement('canvas');
  canvas.lang = lang;
  canvas.width = 512;
  canvas.height = 96;
  document.body.appendChild(canvas);
  const context = canvas.getContext('2d');
  context.fillStyle = 'black';
  context.font = font;
  context.fillText(text, 8, 64);
  return context.getImageData(0, 0, canvas.width, canvas.height).data;
}
function equal(left, right) {
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index++) {
    if (left[index] !== right[index]) return false;
  }
  return true;
}
const results = cases.map(([lang, text, requested, expected]) =>
  equal(pixels(lang, text, requested), pixels(lang, text, expected))
);
const passed = results.every(Boolean);
document.title = results.map((result, index) => `${index}:${result}`).join(',');
document.body.style.background = passed ? 'rgb(31,143,76)' : 'rgb(181,37,46)';
} catch (error) {
  document.title = `${error.name}:${error.message}`;
  document.body.style.background = 'rgb(181,37,46)';
}
</script></body></html>"#;
    format!("data:text/html,{}", percent_encode(html))
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn center_pixel_rgb(frame: &ely_servo_host::RenderedFrame) -> [u8; 3] {
    let index = (((frame.height() / 2) * frame.width() + frame.width() / 2) * 4) as usize;
    let rgba = &frame.rgba_bytes()[index..index + 4];
    [rgba[0], rgba[1], rgba[2]]
}

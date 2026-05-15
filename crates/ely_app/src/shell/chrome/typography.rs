use std::borrow::Cow;

use gpui::App;

const NEWSREADER_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Newsreader.ttf");
const NEWSREADER_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/Newsreader-Italic.ttf");
const GEIST_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Geist-Regular.ttf");

/// The bundled Newsreader.ttf is the 16pt optical-size cut. GPUI's text
/// system matches by the font's TrueType name-id 1, which is
/// "Newsreader 16pt" for this file (the unqualified "Newsreader" only
/// appears in name-id 16). Using the wrong string silently falls back
/// to the default sans, so headings would render in Geist instead of
/// the design's display serif.
pub(crate) const SERIF_FAMILY: &str = "Newsreader 16pt";
pub(crate) const SANS_FAMILY: &str = "Geist";

/// Register every bundled UI font with GPUI's text system.
///
/// Newsreader is the design's display serif (Hero headlines, settings
/// titles, reading-list cover). Geist is the design's body sans —
/// without it GPUI falls back to `.SystemUIFont` (SF Pro on macOS),
/// which is too humanist for the geometric calm tech aesthetic the
/// design specifies.
pub(crate) fn register_serif_fonts(cx: &App) -> gpui::Result<()> {
    cx.text_system().add_fonts(vec![
        Cow::Borrowed(NEWSREADER_REGULAR),
        Cow::Borrowed(NEWSREADER_ITALIC),
        Cow::Borrowed(GEIST_REGULAR),
    ])
}

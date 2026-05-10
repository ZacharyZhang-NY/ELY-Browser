use std::borrow::Cow;

use gpui::App;

const NEWSREADER_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/Newsreader.ttf");
const NEWSREADER_ITALIC: &[u8] =
    include_bytes!("../../../assets/fonts/Newsreader-Italic.ttf");
const GEIST_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/Geist-Regular.ttf");

pub(crate) const SERIF_FAMILY: &str = "Newsreader";
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

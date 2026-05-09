use std::borrow::Cow;

use gpui::App;

const NEWSREADER_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/Newsreader.ttf");
const NEWSREADER_ITALIC: &[u8] =
    include_bytes!("../../../assets/fonts/Newsreader-Italic.ttf");

pub(crate) const SERIF_FAMILY: &str = "Newsreader";

pub(crate) fn register_serif_fonts(cx: &App) -> gpui::Result<()> {
    cx.text_system().add_fonts(vec![
        Cow::Borrowed(NEWSREADER_REGULAR),
        Cow::Borrowed(NEWSREADER_ITALIC),
    ])
}

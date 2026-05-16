use ely_domain::TabId;

use super::web_surface_frame::WebSurfaceFrame;

/// One page's worth of metadata observed in a Ready frame. The
/// controller applies these to the `BrowserTab` after the frame has
/// been swapped into the surface state. Title and favicon are
/// independent: navigation often settles the URL first, then Servo
/// emits a title change a frame or two later.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WebSurfacePageMetadata {
    pub(super) tab_id: TabId,
    pub(super) title: Option<String>,
    pub(super) favicon_url: Option<String>,
}

impl WebSurfacePageMetadata {
    pub(super) fn from_frame(tab_id: &TabId, frame: &WebSurfaceFrame) -> Option<Self> {
        let title = frame.title().map(str::to_string);
        let favicon_url = frame
            .loaded_url()
            .and_then(|loaded| ely_domain::UrlText::parse(loaded).ok())
            .and_then(|url| url.favicon_url());
        if title.is_none() && favicon_url.is_none() {
            return None;
        }
        Some(Self { tab_id: tab_id.clone(), title, favicon_url })
    }
}

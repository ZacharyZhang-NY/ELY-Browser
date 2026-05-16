use ely_domain::TabId;

use super::web_surface_frame::WebSurfaceFrame;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct WebSurfaceMetadataTracker {
    last_title: Option<String>,
    last_loaded_url: Option<String>,
}

impl WebSurfaceMetadataTracker {
    pub(super) fn changed_metadata_for(
        &mut self,
        tab_id: &TabId,
        frame: &WebSurfaceFrame,
    ) -> Option<WebSurfacePageMetadata> {
        let title = frame.title().map(str::to_string);
        let loaded_url = frame.loaded_url().map(str::to_string);
        if self.last_title == title && self.last_loaded_url == loaded_url {
            return None;
        }

        self.last_title = title.clone();
        self.last_loaded_url = loaded_url.clone();
        WebSurfacePageMetadata::from_parts(tab_id, title, loaded_url)
    }
}

/// One page's worth of metadata observed in a Ready frame. The
/// controller applies these to the `BrowserTab` after the frame has
/// been swapped into the surface state. Title and favicon key are
/// independent: navigation often settles the URL first, then Servo
/// emits a title change a frame or two later.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WebSurfacePageMetadata {
    pub(super) tab_id: TabId,
    pub(super) title: Option<String>,
    pub(super) favicon_key: Option<String>,
}

impl WebSurfacePageMetadata {
    fn from_parts(
        tab_id: &TabId,
        title: Option<String>,
        loaded_url: Option<String>,
    ) -> Option<Self> {
        let favicon_key = loaded_url
            .as_deref()
            .and_then(|loaded| ely_domain::UrlText::parse(loaded).ok())
            .and_then(|url| url.favicon_key());
        if title.is_none() && favicon_key.is_none() {
            return None;
        }
        Some(Self { tab_id: tab_id.clone(), title, favicon_key })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use ely_domain::TabId;

    use super::*;
    use crate::{
        services::servo_live::ServoLiveFrame,
        shell::{web_surface_frame::WebSurfaceFrame, web_surface_geometry::WebSurfaceScrollOffset},
    };

    #[test]
    fn repeated_frame_metadata_emits_once() -> Result<(), Box<dyn Error>> {
        let tab_id = TabId::new();
        let frame = frame()?;
        let mut tracker = WebSurfaceMetadataTracker::default();

        let first = tracker.changed_metadata_for(&tab_id, &frame);
        let second = tracker.changed_metadata_for(&tab_id, &frame);

        assert!(first.is_some());
        assert_eq!(second, None);
        Ok(())
    }

    fn frame() -> Result<WebSurfaceFrame, Box<dyn Error>> {
        let rgba_pixel = vec![255u8, 255, 255, 255];
        let live = ServoLiveFrame::for_test(1, 1, rgba_pixel);
        Ok(WebSurfaceFrame::from_live_frame(
            "https://example.com/".to_string(),
            WebSurfaceScrollOffset::default(),
            100,
            live,
        )?)
    }
}

use std::sync::Mutex;

use ely_domain::{BrowserTab, ColorScheme, ProfileId, SpaceId, TabId, UrlText};
use gpui::{Bounds, point, px, size};

use crate::services::{
    ProfileDataMode,
    servo_live::{ServoLiveEnsureRequest, ServoLiveFrame},
};

use super::{
    super::{
        web_surface::WebSurfaceStore,
        web_surface_state::WebSurfaceInputOutcome,
        web_surface_worker::{LiveRuntimeClient, LiveRuntimeClientError},
    },
    WebSurfaceRuntime,
};

static COLOR_SCHEMES: Mutex<Vec<ColorScheme>> = Mutex::new(Vec::new());

struct ThemeRecordingClient;

impl LiveRuntimeClient for ThemeRecordingClient {
    fn ensure(
        &mut self,
        request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        COLOR_SCHEMES
            .lock()
            .map_err(|_| "color scheme recorder lock was poisoned".to_string())?
            .push(request.color_scheme);
        Ok(None)
    }

    fn poll(&mut self, _tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Ok(None)
    }

    fn close(&mut self, _tab_id: String) -> Result<(), LiveRuntimeClientError> {
        Ok(())
    }
}

#[test]
fn browser_color_scheme_reaches_each_web_surface_ensure() -> Result<(), String> {
    COLOR_SCHEMES.lock().map_err(|_| "color scheme recorder lock was poisoned")?.clear();
    let runtime =
        WebSurfaceRuntime::new_with_client_factory(|_| Ok(Box::new(ThemeRecordingClient)));
    let mut store = WebSurfaceStore::new_with_runtime(runtime);
    let tab = BrowserTab::new(
        TabId::new(),
        SpaceId::new(),
        ProfileId::new(),
        "Theme",
        UrlText::parse("https://example.com/theme").map_err(|error| error.to_string())?,
    );
    let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(640.0), px(480.0)));

    assert_eq!(store.record_viewport_size(tab.id(), bounds, 1.0), WebSurfaceInputOutcome::Applied,);
    let _ = store.ensure_surface(&tab, ProfileDataMode::Transient, &[]);
    store.flush_runtime_for_test();

    store.set_color_scheme(ColorScheme::Dark);
    let _ = store.ensure_surface(&tab, ProfileDataMode::Transient, &[]);
    store.flush_runtime_for_test();

    assert_eq!(
        *COLOR_SCHEMES.lock().map_err(|_| "color scheme recorder lock was poisoned")?,
        vec![ColorScheme::Light, ColorScheme::Dark],
    );
    Ok(())
}

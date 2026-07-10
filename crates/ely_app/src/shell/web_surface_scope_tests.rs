use std::{error::Error, sync::Mutex};

use ely_domain::{BrowserTab, ProfileId, SpaceId, TabId, UrlText};
use gpui::{Bounds, point, px, size};

use crate::services::{
    ProfileDataMode,
    servo_live::{ServoLiveEnsureRequest, ServoLiveFrame},
};

use super::super::{
    web_surface_frame::WebSurfaceFrame,
    web_surface_geometry::WebSurfaceScrollOffset,
    web_surface_runtime::{
        WebSurfaceRuntime, WebSurfaceRuntimeFrame, WebSurfaceUrlChange, WebSurfaceUrlChangeKind,
    },
    web_surface_state::{WebSurfaceInputOutcome, WebSurfaceState},
    web_surface_worker::{LiveRuntimeClient, LiveRuntimeClientError},
};
use super::WebSurfaceStore;

static ENSURES: Mutex<Vec<RecordedEnsure>> = Mutex::new(Vec::new());

#[derive(Debug)]
struct RecordedEnsure {
    profile_id: String,
    had_input: bool,
}

struct RecordingClient;

impl LiveRuntimeClient for RecordingClient {
    fn ensure(
        &mut self,
        request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        ENSURES.lock().map_err(|_| "ensure recorder lock was poisoned".to_string())?.push(
            RecordedEnsure {
                profile_id: request.profile_id,
                had_input: request.scroll_delta_x != 0
                    || request.scroll_delta_y != 0
                    || request.click_x.is_some()
                    || request.click_y.is_some()
                    || request.hover_x.is_some()
                    || request.hover_y.is_some()
                    || request.typed_text.is_some(),
            },
        );
        Ok(None)
    }

    fn poll(&mut self, _tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Ok(None)
    }

    fn close(&mut self, _tab_id: String) -> Result<(), LiveRuntimeClientError> {
        Ok(())
    }
}

fn recording_client_factory(
    _config_dir: std::path::PathBuf,
) -> Result<Box<dyn LiveRuntimeClient>, String> {
    Ok(Box::new(RecordingClient))
}

#[test]
fn profile_scope_change_clears_pixels_input_and_focus_before_ensure() -> Result<(), Box<dyn Error>>
{
    ENSURES.lock().map_err(|_| "ensure recorder lock was poisoned")?.clear();
    let runtime = WebSurfaceRuntime::new_with_client_factory(recording_client_factory);
    let mut store = WebSurfaceStore::new_with_runtime(runtime);
    let tab_id = TabId::new();
    let url = UrlText::parse("https://example.com/shared")?;
    let profile_a = ProfileId::new();
    let profile_b = ProfileId::new();
    let tab_a =
        BrowserTab::new(tab_id.clone(), SpaceId::new(), profile_a.clone(), "Shared", url.clone());
    let tab_b = BrowserTab::new(tab_id, SpaceId::new(), profile_b.clone(), "Shared", url);

    assert_eq!(
        store.record_viewport_size(tab_a.id(), viewport_bounds(), 1.0),
        WebSurfaceInputOutcome::Applied,
    );
    assert!(store.ensure_surface(&tab_a, ProfileDataMode::Transient, &[]));
    store.flush_runtime_for_test();
    let frame = WebSurfaceFrame::from_live_frame(
        tab_a.url().as_str().to_string(),
        WebSurfaceScrollOffset::default(),
        100,
        ServoLiveFrame::for_test(1, 1, vec![0, 0, 0, 255]),
    )?;
    store.surface_mut(tab_a.id()).state = Some(WebSurfaceState::Ready(frame));
    assert!(
        store.state_for_scope(tab_a.id(), &profile_b, ProfileDataMode::Transient).is_none(),
        "a model scope change must hide old pixels before the next ensure",
    );

    assert_eq!(
        store
            .record_click_point(tab_a.id(), tab_a.url().as_str(), point(px(120.0), px(80.0)), 1.0,),
        WebSurfaceInputOutcome::Applied,
    );
    assert_eq!(
        store.record_typed_text(tab_a.id(), tab_a.url().as_str(), "secret"),
        WebSurfaceInputOutcome::Applied,
    );
    assert!(store.ensure_surface(&tab_b, ProfileDataMode::Transient, &[]));
    store.flush_runtime_for_test();

    assert!(store.keyboard_focus.is_none());
    assert!(matches!(
        store.state(tab_b.id()),
        Some(WebSurfaceState::Loading { previous_frame: None, .. })
    ));
    let ensures = ENSURES.lock().map_err(|_| "ensure recorder lock was poisoned")?;
    assert_eq!(ensures.len(), 2);
    assert_eq!(ensures[0].profile_id, profile_a.as_str());
    assert_eq!(ensures[1].profile_id, profile_b.as_str());
    assert!(!ensures[1].had_input);
    Ok(())
}

fn viewport_bounds() -> Bounds<gpui::Pixels> {
    Bounds::new(point(px(0.0), px(0.0)), size(px(640.0), px(480.0)))
}

#[test]
fn navigation_holds_loading_white_frame_and_accepts_complete_white_page()
-> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new_with_runtime(WebSurfaceRuntime::new_with_client_factory(
        recording_client_factory,
    ));
    let tab_id = TabId::new();
    let requested_url = "https://example.com/white".to_string();
    let previous = WebSurfaceFrame::from_live_frame(
        requested_url.clone(),
        WebSurfaceScrollOffset::default(),
        100,
        ServoLiveFrame::for_test(1, 1, vec![0, 0, 0, 255]),
    )?;
    store.surface_mut(&tab_id).state = Some(WebSurfaceState::Loading {
        requested_url: requested_url.clone(),
        previous_frame: Some(previous),
    });
    let loading_white = WebSurfaceFrame::from_live_frame(
        requested_url.clone(),
        WebSurfaceScrollOffset::default(),
        100,
        ServoLiveFrame::for_test_with_render_state(1, 1, vec![255; 4], "loading"),
    )?;
    let complete_white = WebSurfaceFrame::from_live_frame(
        requested_url,
        WebSurfaceScrollOffset::default(),
        100,
        ServoLiveFrame::for_test_with_render_state(1, 1, vec![255; 4], "complete"),
    )?;

    assert!(store.should_hold_initial_frame(&tab_id, &loading_white, false));
    assert!(!store.should_hold_initial_frame(&tab_id, &complete_white, false));
    Ok(())
}

#[test]
fn progressive_loading_frame_replaces_an_earlier_ready_frame() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new_with_runtime(WebSurfaceRuntime::new_with_client_factory(
        recording_client_factory,
    ));
    let tab_id = TabId::new();
    let requested_url = "https://example.com/progressive".to_string();
    let first_loading = WebSurfaceFrame::from_live_frame(
        requested_url.clone(),
        WebSurfaceScrollOffset::default(),
        100,
        ServoLiveFrame::for_test_with_render_state(1, 1, vec![16, 32, 64, 255], "loading"),
    )?;
    store.surface_mut(&tab_id).state = Some(WebSurfaceState::Ready(first_loading));
    let next_loading = WebSurfaceFrame::from_live_frame(
        requested_url,
        WebSurfaceScrollOffset::default(),
        100,
        ServoLiveFrame::for_test_with_render_state(1, 1, vec![64, 32, 16, 255], "loading"),
    )?;

    assert!(!store.should_hold_initial_frame(&tab_id, &next_loading, true));
    Ok(())
}

#[test]
fn held_loading_frame_preserves_previous_pixels_and_delivers_metadata() -> Result<(), Box<dyn Error>>
{
    let mut store = WebSurfaceStore::new_with_runtime(WebSurfaceRuntime::new_with_client_factory(
        recording_client_factory,
    ));
    let tab_id = TabId::new();
    let requested_url = "https://example.com/held".to_string();
    let previous = WebSurfaceFrame::from_live_frame(
        requested_url.clone(),
        WebSurfaceScrollOffset::default(),
        100,
        ServoLiveFrame::for_test_with_title(1, 1, vec![0, 0, 0, 255], "complete", "Previous"),
    )?;
    store.surface_mut(&tab_id).state = Some(WebSurfaceState::Loading {
        requested_url: requested_url.clone(),
        previous_frame: Some(previous),
    });
    let held = WebSurfaceFrame::from_live_frame(
        requested_url,
        WebSurfaceScrollOffset::default(),
        100,
        ServoLiveFrame::for_test_with_title(1, 1, vec![255; 4], "loading", "Updated"),
    )?;
    let result = store.apply_runtime_frames(vec![WebSurfaceRuntimeFrame::Ready {
        tab_id: tab_id.clone(),
        frame: Box::new(held),
        url_change: Some(WebSurfaceUrlChange {
            tab_id: tab_id.clone(),
            loaded_url: "https://example.com/redirected".to_string(),
            kind: WebSurfaceUrlChangeKind::Observed,
        }),
    }]);

    assert!(matches!(
        store.state(&tab_id),
        Some(WebSurfaceState::Loading { previous_frame: Some(frame), .. })
            if frame.title() == Some("Previous")
    ));
    assert!(matches!(
        result.page_metadata.as_slice(),
        [metadata] if metadata.title.as_deref() == Some("Updated")
    ));
    assert_eq!(result.url_changes.len(), 1);
    Ok(())
}

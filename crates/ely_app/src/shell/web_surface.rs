use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use ely_domain::{BrowserTab, TabId};

use crate::services::ProfileDataMode;

use super::{
    web_surface_cadence::{ACTIVE_POLL_INTERVAL, IDLE_POLL_INTERVAL},
    web_surface_frame::WebSurfaceFrame,
    web_surface_permissions::WebSurfaceSitePermission,
    web_surface_runtime::{WebSurfaceRuntime, WebSurfaceRuntimeFrame},
    web_surface_state::{
        PerTabSurface, WebSurfaceEnsureKey, WebSurfaceKeyboardFocusState, WebSurfacePendingInput,
        WebSurfaceState, WebSurfaceTickResult,
    },
};

pub(super) struct WebSurfaceStore {
    runtime: WebSurfaceRuntime,
    /// Single owner of every per-tab invariant. See [`PerTabSurface`].
    pub(super) surfaces: BTreeMap<TabId, PerTabSurface>,
    /// Singleton because only one tab at a time holds keyboard focus
    /// across the whole window. Lives on the store, not per-tab.
    pub(super) keyboard_focus: Option<WebSurfaceKeyboardFocusState>,
}

impl WebSurfaceStore {
    pub(super) fn new() -> Self {
        Self { runtime: WebSurfaceRuntime::new(), surfaces: BTreeMap::new(), keyboard_focus: None }
    }

    #[cfg(test)]
    pub(super) fn new_with_runtime(runtime: WebSurfaceRuntime) -> Self {
        Self { runtime, surfaces: BTreeMap::new(), keyboard_focus: None }
    }

    #[cfg(test)]
    pub(super) fn state(&self, tab_id: &TabId) -> Option<&WebSurfaceState> {
        self.surfaces.get(tab_id).and_then(|surface| surface.state.as_ref())
    }

    pub(super) fn state_for_scope(
        &self,
        tab_id: &TabId,
        profile_id: &ely_domain::ProfileId,
        profile_data_mode: ProfileDataMode,
    ) -> Option<&WebSurfaceState> {
        self.surfaces
            .get(tab_id)
            .filter(|surface| surface.has_scope(profile_id, profile_data_mode))
            .and_then(|surface| surface.state.as_ref())
    }

    pub(super) fn ensure_surface(
        &mut self,
        tab: &BrowserTab,
        profile_data_mode: ProfileDataMode,
        permissions: &[WebSurfaceSitePermission],
    ) -> bool {
        if !is_external_web_url(tab.url().as_str()) {
            return false;
        }
        let requested_url = tab.url().as_str().to_string();
        let Some(size) = self.surfaces.get(tab.id()).and_then(|surface| surface.viewport_size)
        else {
            return false;
        };
        let ensure_key = WebSurfaceEnsureKey::new(
            requested_url.clone(),
            size,
            tab.profile_id().clone(),
            profile_data_mode,
            tab.zoom_percent(),
            permissions,
        );
        let scope_changed = self.surface_mut(tab.id()).reset_for_scope_change(&ensure_key);
        if scope_changed
            && self.keyboard_focus.as_ref().is_some_and(|focus| focus.tab_id == *tab.id())
        {
            self.keyboard_focus = None;
        }
        let runtime_has_session =
            self.runtime.has_session(tab.id(), tab.profile_id(), profile_data_mode);
        if runtime_has_session
            && self
                .surfaces
                .get(tab.id())
                .is_some_and(|surface| !surface.should_ensure(&ensure_key))
        {
            return false;
        }
        // Once the page has ensured at least once, defer further ensures
        // while the viewport is still being animated. Without this, a
        // sidebar slide or window-edge drag fires `ensure_surface` per
        // GPUI paint, each call hitting Servo's `rendering_context.resize`
        // which destroys and reallocates the framebuffer — the source of
        // the per-frame blank flash. The first ensure (when no prior
        // `last_ensure_key` is set) always fires so the page can load.
        let already_ensured =
            self.surfaces.get(tab.id()).is_some_and(|surface| surface.last_ensure_key.is_some());
        if already_ensured
            && self
                .surfaces
                .get(tab.id())
                .is_some_and(|surface| surface.viewport_size_is_settling(Instant::now()))
        {
            return false;
        }
        let previous_frame =
            self.previous_ready_frame(tab.id(), requested_url.as_str(), tab.zoom_percent());
        if let Err(message) =
            self.runtime.prepare_tab_scope(tab.id(), tab.profile_id(), profile_data_mode)
        {
            return self.record_ensure_failure(
                tab.id(),
                ensure_key,
                requested_url,
                previous_frame,
                message,
            );
        }
        let input = self.take_pending_input(tab.id(), requested_url.as_str());

        let ensure_result =
            self.runtime.ensure_tab(tab, size, profile_data_mode, permissions, input);

        match ensure_result {
            Ok(result) => {
                self.surface_mut(tab.id()).mark_ensured(ensure_key);
                if result.started_loading {
                    self.surface_mut(tab.id()).state = Some(WebSurfaceState::Loading {
                        requested_url: result.requested_url,
                        previous_frame,
                    });
                    return true;
                }
                false
            }
            Err(message) => self.record_ensure_failure(
                tab.id(),
                ensure_key,
                requested_url,
                previous_frame,
                message,
            ),
        }
    }

    fn record_ensure_failure(
        &mut self,
        tab_id: &TabId,
        ensure_key: WebSurfaceEnsureKey,
        requested_url: String,
        previous_frame: Option<WebSurfaceFrame>,
        message: String,
    ) -> bool {
        self.surface_mut(tab_id).mark_ensured(ensure_key);
        match previous_frame {
            Some(previous_frame) => {
                self.surface_mut(tab_id).state = Some(WebSurfaceState::Loading {
                    requested_url,
                    previous_frame: Some(previous_frame),
                });
                false
            }
            None => {
                self.surface_mut(tab_id).state = Some(WebSurfaceState::Failed { message });
                true
            }
        }
    }

    pub(super) fn tick(&mut self, visible_tab_ids: &[TabId]) -> WebSurfaceTickResult {
        let frames = self.runtime.tick(visible_tab_ids);
        self.apply_runtime_frames(frames)
    }

    fn apply_runtime_frames(
        &mut self,
        frames: Vec<WebSurfaceRuntimeFrame>,
    ) -> WebSurfaceTickResult {
        let mut result = WebSurfaceTickResult::default();

        for frame in frames {
            match frame {
                WebSurfaceRuntimeFrame::Ready { tab_id, frame, url_change } => {
                    let had_ready = matches!(
                        self.surfaces.get(&tab_id).and_then(|surface| surface.state.as_ref()),
                        Some(WebSurfaceState::Ready(_))
                    );
                    let metadata = self.surface_mut(&tab_id).changed_page_metadata(&tab_id, &frame);
                    result.page_metadata.extend(metadata);
                    result.url_changes.extend(url_change);
                    if self.should_hold_initial_frame(&tab_id, &frame, had_ready) {
                        continue;
                    }
                    if self
                        .surfaces
                        .get(&tab_id)
                        .is_none_or(|surface| !surface.matches_ready(&frame))
                    {
                        self.surface_mut(&tab_id).state = Some(WebSurfaceState::Ready(*frame));
                        result.changed = true;
                    }
                }
                WebSurfaceRuntimeFrame::Failed { tab_id, message } => {
                    let had_ready = matches!(
                        self.surfaces.get(&tab_id).and_then(|surface| surface.state.as_ref()),
                        Some(WebSurfaceState::Ready(_))
                    );
                    if had_ready {
                        tracing::warn!(
                            target: "ely::web_surface",
                            tab_id = %tab_id,
                            message = %message,
                            "transient surface error; keeping last frame",
                        );
                        continue;
                    }
                    self.surface_mut(&tab_id).state = Some(WebSurfaceState::Failed { message });
                    result.changed = true;
                }
            }
        }

        result
    }

    pub(super) fn next_tick_delay(&self, visible_tab_ids: &[TabId]) -> Duration {
        let now = Instant::now();
        // While any visible tab's viewport is mid-animation, poll at the
        // active 120 Hz cadence so the trailing-edge resize fires within
        // one frame of the gesture settling. Without this boost the
        // idle 80 ms polling adds a noticeable lag between letting go
        // of a resize and the page re-laying-out at the final size.
        let any_settling = visible_tab_ids.iter().any(|tab_id| {
            self.surfaces.get(tab_id).is_some_and(|s| s.viewport_size_is_settling(now))
        });
        if any_settling {
            return ACTIVE_POLL_INTERVAL;
        }
        self.runtime
            .next_poll_delay(visible_tab_ids, now)
            .unwrap_or(IDLE_POLL_INTERVAL)
            .min(IDLE_POLL_INTERVAL)
    }

    pub(super) fn retain_tabs(&mut self, open_tab_ids: &[TabId]) {
        let stale_tab_ids = self
            .surfaces
            .keys()
            .filter(|tab_id| !open_tab_ids.contains(*tab_id))
            .cloned()
            .collect::<Vec<_>>();
        for tab_id in stale_tab_ids {
            self.close_surface(&tab_id);
        }
    }

    fn take_pending_input(
        &mut self,
        tab_id: &TabId,
        requested_url: &str,
    ) -> WebSurfacePendingInput {
        let surface = self.surface_mut(tab_id);
        let scroll_offset = surface.scroll_offset_for(requested_url);
        let scroll_delta = surface.pending_scroll_delta.take();
        let scroll_point = surface.pending_scroll_point.take();
        let click_point = surface
            .click_point
            .take()
            .filter(|state| {
                state.requested_url == requested_url && state.scroll_offset == scroll_offset
            })
            .map(|state| state.point);
        let typed_text = surface
            .typed_text
            .take()
            .filter(|state| state.requested_url == requested_url)
            .map(|state| state.text);
        let hover_point = surface.hover_point.take();
        let enqueued_at = surface.pending_input_started_at.take();
        if scroll_delta.is_some()
            || click_point.is_some()
            || hover_point.is_some()
            || typed_text.is_some()
        {
            surface.mark_input_flushed(Instant::now());
        }

        WebSurfacePendingInput {
            enqueued_at,
            scroll_offset,
            scroll_delta,
            scroll_point,
            click_point,
            hover_point,
            typed_text,
        }
    }

    fn previous_ready_frame(
        &self,
        tab_id: &TabId,
        requested_url: &str,
        zoom_percent: u16,
    ) -> Option<WebSurfaceFrame> {
        match self.surfaces.get(tab_id).and_then(|surface| surface.state.as_ref()) {
            Some(WebSurfaceState::Ready(frame))
                if frame.requested_url == requested_url && frame.zoom_percent() == zoom_percent =>
            {
                Some(frame.clone())
            }
            Some(WebSurfaceState::Loading {
                requested_url: current_url, previous_frame, ..
            }) if current_url == requested_url => previous_frame
                .as_ref()
                .filter(|frame| frame.zoom_percent() == zoom_percent)
                .cloned(),
            _ => None,
        }
    }

    fn should_hold_initial_frame(
        &self,
        tab_id: &TabId,
        frame: &WebSurfaceFrame,
        has_previous_frame: bool,
    ) -> bool {
        frame.render_state() != "complete"
            && !has_previous_frame
            && self
                .previous_ready_frame(tab_id, frame.requested_url.as_str(), frame.zoom_percent())
                .is_some()
    }

    pub(super) fn surface_mut(&mut self, tab_id: &TabId) -> &mut PerTabSurface {
        self.surfaces.entry(tab_id.clone()).or_insert_with(PerTabSurface::new)
    }

    pub(super) fn close_surface(&mut self, tab_id: &TabId) {
        self.runtime.close_tab(tab_id);
        self.surfaces.remove(tab_id);
        if self.keyboard_focus.as_ref().is_some_and(|focus| focus.tab_id == *tab_id) {
            self.keyboard_focus = None;
        }
    }

    #[cfg(test)]
    pub(super) fn surface_for_test(&self, tab_id: &TabId) -> Option<&PerTabSurface> {
        self.surfaces.get(tab_id)
    }

    #[cfg(test)]
    pub(super) fn flush_runtime_for_test(&mut self) {
        self.runtime.flush_for_test();
    }
}

pub(super) fn is_external_web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

#[cfg(test)]
#[path = "web_surface_tests.rs"]
mod tests;

#[cfg(all(test, feature = "live-site-smoke"))]
#[path = "web_surface_live_site_tests.rs"]
mod web_surface_live_site_tests;

#[cfg(all(test, feature = "live-site-smoke"))]
#[path = "web_surface_profile_isolation_tests.rs"]
mod web_surface_profile_isolation_tests;

#[cfg(all(test, feature = "live-site-smoke", target_os = "macos"))]
#[path = "web_surface_hardware_import_tests.rs"]
mod web_surface_hardware_import_tests;

#[cfg(test)]
#[path = "web_surface_scope_tests.rs"]
mod web_surface_scope_tests;

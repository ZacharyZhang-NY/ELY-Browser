use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use ely_domain::{BrowserTab, TabId};

use crate::services::ProfileDataMode;

use super::{
    web_surface_cadence::IDLE_POLL_INTERVAL,
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

    pub(super) fn state(&self, tab_id: &TabId) -> Option<&WebSurfaceState> {
        self.surfaces.get(tab_id).and_then(|surface| surface.state.as_ref())
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
        let native_surface =
            self.surfaces.get(tab.id()).and_then(|surface| surface.native_surface.clone());
        #[cfg(not(test))]
        let Some(native_surface) = native_surface else {
            return false;
        };
        let ensure_key = WebSurfaceEnsureKey::new(
            requested_url.clone(),
            size,
            #[cfg(test)]
            native_surface.as_ref(),
            #[cfg(not(test))]
            Some(&native_surface),
            tab.zoom_percent(),
            permissions,
        );
        if self.surfaces.get(tab.id()).is_some_and(|surface| !surface.should_ensure(&ensure_key)) {
            return false;
        }
        let input = self.take_pending_input(tab.id(), requested_url.as_str());
        let previous_frame =
            self.previous_ready_frame(tab.id(), requested_url.as_str(), tab.zoom_percent());

        #[cfg(test)]
        let ensure_result = match native_surface {
            Some(native_surface) => self.runtime.ensure_tab_with_native_surface(
                tab,
                size,
                native_surface,
                profile_data_mode,
                permissions,
                input,
            ),
            None => self.runtime.ensure_tab(tab, size, profile_data_mode, permissions, input),
        };
        #[cfg(not(test))]
        let ensure_result = self.runtime.ensure_tab_with_native_surface(
            tab,
            size,
            native_surface,
            profile_data_mode,
            permissions,
            input,
        );

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
            Err(message) => {
                self.surface_mut(tab.id()).mark_ensured(ensure_key);
                self.surface_mut(tab.id()).state = Some(WebSurfaceState::Failed { message });
                true
            }
        }
    }

    pub(super) fn tick(&mut self, visible_tab_ids: &[TabId]) -> WebSurfaceTickResult {
        let frames = self.runtime.tick(visible_tab_ids);
        let mut result = WebSurfaceTickResult::default();

        for frame in frames {
            match frame {
                WebSurfaceRuntimeFrame::Ready { tab_id, frame, url_change } => {
                    let had_ready = matches!(
                        self.surfaces.get(&tab_id).and_then(|surface| surface.state.as_ref()),
                        Some(WebSurfaceState::Ready(_))
                    );
                    match self.initial_display_gate_message(&tab_id, &frame, had_ready) {
                        Ok(()) => {}
                        Err(message) => {
                            if !had_ready {
                                self.surface_mut(&tab_id).state =
                                    Some(WebSurfaceState::Failed { message });
                                result.changed = true;
                            }
                            continue;
                        }
                    }
                    if self.should_hold_initial_frame(&tab_id, &frame, had_ready) {
                        if !had_ready {
                            self.surface_mut(&tab_id).state = Some(WebSurfaceState::Loading {
                                requested_url: frame.requested_url.clone(),
                                previous_frame: None,
                            });
                            result.changed = true;
                        }
                        continue;
                    }
                    let metadata = self.surface_mut(&tab_id).changed_page_metadata(&tab_id, &frame);
                    result.page_metadata.extend(metadata);
                    if self
                        .surfaces
                        .get(&tab_id)
                        .is_none_or(|surface| !surface.matches_ready(&frame))
                    {
                        self.surface_mut(&tab_id).state = Some(WebSurfaceState::Ready(*frame));
                        result.changed = true;
                    }
                    result.url_changes.extend(url_change);
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
        self.runtime
            .next_poll_delay(visible_tab_ids, Instant::now())
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
        !has_previous_frame
            && self
                .previous_ready_frame(tab_id, frame.requested_url.as_str(), frame.zoom_percent())
                .is_none()
            && matches!(frame.has_visible_content_for_initial_display(), Ok(false))
    }

    fn initial_display_gate_message(
        &self,
        tab_id: &TabId,
        frame: &WebSurfaceFrame,
        has_previous_frame: bool,
    ) -> Result<(), String> {
        if has_previous_frame
            || self
                .previous_ready_frame(tab_id, frame.requested_url.as_str(), frame.zoom_percent())
                .is_some()
        {
            return Ok(());
        }
        frame.has_visible_content_for_initial_display().map(|_| ()).map_err(|error| {
            format!(
                "Servo hardware surface initial content check failed for {}: {error}",
                frame.requested_url
            )
        })
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
    pub(super) fn flush_runtime_for_test(&self) {
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

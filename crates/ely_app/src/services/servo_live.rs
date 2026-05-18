use std::{collections::BTreeMap, path::PathBuf};

use ely_domain::{
    ProfileId, SiteOrigin, SitePermissionDecision, SitePermissionFeature, TabId, UrlText, WebViewId,
};
use ely_servo_host::{
    HidpiScaleRequest, KeyboardTextRequest, MouseClickRequest, MouseHoverRequest,
    NavigationRequest, PageZoomRequest, PermissionDecision, PermissionRequest, ResizeRequest,
    ScrollRequest, ServoHost, ServoSurfaceSize, SoftwareServoHost,
};

#[path = "servo_live_types.rs"]
mod types;

pub(crate) use types::{
    ServoLiveEnsureRequest, ServoLiveError, ServoLiveFrame, ServoLiveSitePermission,
};

/// Servo's `WebViewBuilder` defaults the hidpi (device → CSS) scale to
/// 1.0 — see the contract documented on
/// [`ely_servo_host::HidpiScaleRequest`]. Mirroring the post-build state
/// in [`DirectWebViewSession`] makes the diff inside `apply_viewport`
/// the source of truth for whether `set_hidpi_scale` has to run.
const SERVO_DEFAULT_DEVICE_PIXEL_RATIO: f32 = 1.0;

/// Servo's `WebViewBuilder` likewise defaults page zoom to 1.0 (100%).
const SERVO_DEFAULT_PAGE_ZOOM_PERCENT: u16 = 100;

pub(crate) struct ServoLiveClient {
    host: SoftwareServoHost,
    sessions: BTreeMap<String, DirectWebViewSession>,
}

impl ServoLiveClient {
    pub fn new(profile_data_dir: PathBuf) -> Result<Self, ServoLiveError> {
        let host = SoftwareServoHost::new_with_config_dir(
            ServoSurfaceSize::new(1, 1),
            Some(profile_data_dir),
        )?;
        Ok(Self { host, sessions: BTreeMap::new() })
    }

    pub fn ensure(
        &mut self,
        request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, ServoLiveError> {
        let tab_id = TabId::parse(request.tab_id.clone())?;
        let profile_id = ProfileId::parse(request.profile_id.clone())?;
        let requested_url = UrlText::parse(request.url.clone())?;
        let webview_id = self.ensure_webview(&request, &tab_id, &profile_id)?;

        self.apply_viewport(&request, &webview_id)?;
        self.apply_permissions(&request, &webview_id, &profile_id)?;
        self.apply_navigation(&request, &webview_id, tab_id, requested_url)?;
        self.apply_input(&request, &webview_id)?;
        // Match Servo's `examples/winit_minimal.rs`: spin the event loop on
        // the embedder-side hot path, never paint. Painting is reactive in
        // Servo — `notify_new_frame_ready` on the delegate flags the
        // session, and the next `poll` (gated on `has_pending_frame`)
        // performs the single paint + present for that frame. Forcing a
        // paint here would clear the surface to the WebRender background
        // and present it *before* Servo has composited the navigated
        // page, which is what produced the per-redirect white-flash on
        // sites that perform a chain of redirects (google.com → /
        // → /?zx=…). The `ServoLiveFrame` we return only carries the
        // snapshot metadata; the on-screen surface is owned by Servo via
        // the native NSView and updated through `poll`.
        self.host.tick();
        if !self.session_uses_native_surface(&request.tab_id) {
            return Err(ServoLiveError::NativeSurfaceUnavailable);
        }

        let frame = self.frame_from_session(&request.tab_id, &webview_id)?;
        Ok(Some(frame))
    }

    pub fn poll(&mut self, tab_id: String) -> Result<Option<ServoLiveFrame>, ServoLiveError> {
        let Some(session) = self.sessions.get(&tab_id) else {
            return Ok(None);
        };
        let webview_id = session.webview_id.clone();
        let uses_native_surface = session.native_surface_id.is_some();

        self.host.tick();
        if !self.host.snapshot(&webview_id)?.has_pending_frame() {
            return Ok(None);
        }

        if !uses_native_surface {
            return Err(ServoLiveError::NativeSurfaceUnavailable);
        }
        self.host.paint_without_readback_with_completion(&webview_id, false)?;
        self.frame_from_session(&tab_id, &webview_id).map(Some)
    }

    pub fn close(&mut self, tab_id: String) -> Result<(), ServoLiveError> {
        let Some(session) = self.sessions.remove(&tab_id) else {
            return Ok(());
        };
        self.host.close_webview(&session.webview_id);
        Ok(())
    }

    fn ensure_webview(
        &mut self,
        request: &ServoLiveEnsureRequest,
        tab_id: &TabId,
        profile_id: &ProfileId,
    ) -> Result<WebViewId, ServoLiveError> {
        if self
            .sessions
            .get(&request.tab_id)
            .is_some_and(|session| session.profile_id != *profile_id)
            && let Some(session) = self.sessions.remove(&request.tab_id)
        {
            self.host.close_webview(&session.webview_id);
        }

        let native_surface_id =
            request.native_surface.as_ref().map(gpui::NativeSurfaceHandle::identity);
        if self
            .sessions
            .get(&request.tab_id)
            .is_some_and(|session| session.native_surface_id != native_surface_id)
            && let Some(session) = self.sessions.remove(&request.tab_id)
        {
            self.host.close_webview(&session.webview_id);
        }

        if let Some(session) = self.sessions.get(&request.tab_id) {
            return Ok(session.webview_id.clone());
        }

        let surface_size = ServoSurfaceSize::new(request.width, request.height);
        let webview_id = match request.native_surface.as_ref() {
            Some(native_surface) => self.host.create_webview_with_native_surface(
                tab_id.clone(),
                profile_id.clone(),
                surface_size,
                native_surface,
            )?,
            None => self.host.create_webview_with_size(
                tab_id.clone(),
                profile_id.clone(),
                surface_size,
            )?,
        };
        self.sessions.insert(
            request.tab_id.clone(),
            DirectWebViewSession {
                webview_id: webview_id.clone(),
                profile_id: profile_id.clone(),
                requested_url: None,
                width: request.width,
                height: request.height,
                // Servo's WebViewBuilder defaults page zoom to 1.0 (100%) and
                // hidpi scale to 1.0; record both as Servo's actual post-build
                // state so `apply_viewport` pushes the embedder-requested
                // values on the first ensure. Without this, a request whose
                // zoom/DPR happens to equal the cached request value would
                // bypass `set_page_zoom` / `set_hidpi_scale` and leave the
                // WebView at Servo's defaults — on Retina that collapses CSS
                // pixels onto device pixels and renders pages at half size.
                page_zoom_percent: SERVO_DEFAULT_PAGE_ZOOM_PERCENT,
                device_pixel_ratio: SERVO_DEFAULT_DEVICE_PIXEL_RATIO,
                native_surface_id,
            },
        );
        Ok(webview_id)
    }

    fn apply_viewport(
        &mut self,
        request: &ServoLiveEnsureRequest,
        webview_id: &WebViewId,
    ) -> Result<(), ServoLiveError> {
        let Some(session) = self.sessions.get_mut(&request.tab_id) else {
            return Ok(());
        };
        let change = ViewportChange::between(request, session);

        if let Some((width, height)) = change.resize {
            self.host.resize(ResizeRequest { webview_id: webview_id.clone(), width, height })?;
            session.width = width;
            session.height = height;
        }

        if let Some(scale_factor) = change.set_hidpi {
            self.host.set_hidpi_scale(HidpiScaleRequest {
                webview_id: webview_id.clone(),
                scale_factor,
            })?;
            session.device_pixel_ratio = scale_factor;
        }

        if let Some(page_zoom_percent) = change.set_page_zoom {
            self.host.set_page_zoom(PageZoomRequest {
                webview_id: webview_id.clone(),
                zoom_factor: f32::from(page_zoom_percent) / 100.0,
            })?;
            session.page_zoom_percent = page_zoom_percent;
        }

        Ok(())
    }

    fn apply_permissions(
        &mut self,
        request: &ServoLiveEnsureRequest,
        webview_id: &WebViewId,
        profile_id: &ProfileId,
    ) -> Result<(), ServoLiveError> {
        for permission in &request.site_permissions {
            let origin = SiteOrigin::parse(permission.origin.clone())?;
            let feature = SitePermissionFeature::parse(permission.feature.as_str())?;
            let decision = SitePermissionDecision::parse(permission.decision.as_str())?;
            self.host.set_permission(
                PermissionRequest {
                    webview_id: webview_id.clone(),
                    profile_id: profile_id.clone(),
                    origin,
                    feature,
                },
                PermissionDecision::from(decision),
            )?;
        }
        Ok(())
    }

    fn apply_navigation(
        &mut self,
        request: &ServoLiveEnsureRequest,
        webview_id: &WebViewId,
        tab_id: TabId,
        requested_url: UrlText,
    ) -> Result<(), ServoLiveError> {
        // Servo's `set_history` fires `notify_url_changed` on *every*
        // history mutation — full navigations, redirects, in-page
        // links, and JS-driven `history.pushState` / `replaceState`.
        // Our embedder fans that back through `WebSurfaceUrlChange`
        // into `tab.url`, which makes the next `ensure_surface` see a
        // "different" URL and call back into this method. Without a
        // guard we then send Servo `WebView::load(new_url)` for a URL
        // Servo just informed us it is *already* at — and `load` is a
        // hard navigation that aborts the live document, clears the
        // surface, and refetches.
        //
        // The google.com homepage `replaceState`s a `?zx=<timestamp>`
        // every second; under the unguarded path that turned into a
        // load-clear-refetch cycle per second, i.e. the continuous
        // white flash. Servo's own `webview.url()` (driven by
        // `set_history`) is the source of truth — if it already
        // matches the requested URL, this URL change came *from*
        // Servo and only needs an embedder-side bookkeeping sync.
        let servo_current_url =
            self.host.snapshot(webview_id)?.url().map(str::to_string);
        if servo_current_url.as_deref() == Some(requested_url.as_str()) {
            if let Some(session) = self.sessions.get_mut(&request.tab_id) {
                session.requested_url = Some(requested_url.as_str().to_string());
            }
            return Ok(());
        }

        let should_navigate = self
            .sessions
            .get(&request.tab_id)
            .and_then(|session| session.requested_url.as_deref())
            .is_none_or(|current| current != requested_url.as_str());
        if should_navigate {
            self.host.navigate(NavigationRequest {
                webview_id: webview_id.clone(),
                tab_id,
                url: requested_url.clone(),
            })?;
            if let Some(session) = self.sessions.get_mut(&request.tab_id) {
                session.requested_url = Some(requested_url.as_str().to_string());
            }
        }
        Ok(())
    }

    fn apply_input(
        &mut self,
        request: &ServoLiveEnsureRequest,
        webview_id: &WebViewId,
    ) -> Result<(), ServoLiveError> {
        if request.scroll_delta_x != 0 || request.scroll_delta_y != 0 {
            let point_x = request.scroll_point_x.ok_or(ServoLiveError::MissingScrollPoint)?;
            let point_y = request.scroll_point_y.ok_or(ServoLiveError::MissingScrollPoint)?;
            self.host.scroll(ScrollRequest {
                webview_id: webview_id.clone(),
                delta_x: request.scroll_delta_x,
                delta_y: request.scroll_delta_y,
                point_x,
                point_y,
            })?;
        }

        if let (Some(x), Some(y)) = (request.hover_x, request.hover_y) {
            self.host.hover(MouseHoverRequest { webview_id: webview_id.clone(), x, y })?;
        }

        if let (Some(x), Some(y)) = (request.click_x, request.click_y) {
            self.host.click(MouseClickRequest { webview_id: webview_id.clone(), x, y })?;
        }

        if let Some(text) = request.typed_text.as_ref() {
            self.host.type_text(KeyboardTextRequest {
                webview_id: webview_id.clone(),
                text: text.clone(),
            })?;
        }

        Ok(())
    }

    fn frame_from_session(
        &self,
        tab_id: &str,
        webview_id: &WebViewId,
    ) -> Result<ServoLiveFrame, ServoLiveError> {
        let Some(session) = self.sessions.get(tab_id) else {
            return Err(ServoLiveError::Host(ely_servo_host::ServoHostError::WebViewNotFound {
                id: webview_id.clone(),
            }));
        };
        if session.native_surface_id.is_some() {
            let snapshot = self.host.snapshot(webview_id)?;
            return Ok(ServoLiveFrame::from_presented(
                snapshot,
                session.width,
                session.height,
                session.device_pixel_ratio,
            ));
        }
        Err(ServoLiveError::NativeSurfaceUnavailable)
    }

    fn session_uses_native_surface(&self, tab_id: &str) -> bool {
        self.sessions.get(tab_id).is_some_and(|session| session.native_surface_id.is_some())
    }
}

struct DirectWebViewSession {
    webview_id: WebViewId,
    profile_id: ProfileId,
    requested_url: Option<String>,
    width: u32,
    height: u32,
    page_zoom_percent: u16,
    device_pixel_ratio: f32,
    native_surface_id: Option<usize>,
}

/// Calls that `apply_viewport` needs to make to bring the underlying
/// Servo WebView state in line with the embedder's request. Extracted
/// as a pure value so the diff decision is testable without a live
/// `SoftwareServoHost` — see `tests` below.
#[derive(Debug, Default, PartialEq)]
struct ViewportChange {
    resize: Option<(u32, u32)>,
    set_hidpi: Option<f32>,
    set_page_zoom: Option<u16>,
}

impl ViewportChange {
    fn between(request: &ServoLiveEnsureRequest, session: &DirectWebViewSession) -> Self {
        Self {
            resize: (session.width != request.width || session.height != request.height)
                .then_some((request.width, request.height)),
            set_hidpi: (session.device_pixel_ratio != request.device_pixel_ratio)
                .then_some(request.device_pixel_ratio),
            set_page_zoom: (session.page_zoom_percent != request.page_zoom_percent)
                .then_some(request.page_zoom_percent),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ely_domain::WebViewId;

    fn session(width: u32, height: u32, dpr: f32, zoom: u16) -> DirectWebViewSession {
        DirectWebViewSession {
            webview_id: WebViewId::new(),
            profile_id: ProfileId::new(),
            requested_url: None,
            width,
            height,
            page_zoom_percent: zoom,
            device_pixel_ratio: dpr,
            native_surface_id: Some(0xC0FFEE),
        }
    }

    fn request(width: u32, height: u32, dpr: f32, zoom: u16) -> ServoLiveEnsureRequest {
        ServoLiveEnsureRequest {
            tab_id: String::from("tab"),
            profile_id: String::from("profile"),
            url: String::from("https://example.com/"),
            width,
            height,
            page_zoom_percent: zoom,
            device_pixel_ratio: dpr,
            native_surface: None,
            scroll_delta_x: 0,
            scroll_delta_y: 0,
            scroll_point_x: None,
            scroll_point_y: None,
            click_x: None,
            click_y: None,
            hover_x: None,
            hover_y: None,
            typed_text: None,
            site_permissions: Vec::new(),
        }
    }

    #[test]
    fn fresh_retina_session_pushes_hidpi_only() {
        // A 1440×900 Retina viewport arrives as physical 2880×1800 with
        // DPR=2.0. `ensure_webview` stamps the session with Servo's
        // post-build defaults (1.0 DPR, 100 % zoom) and the request's
        // physical dimensions, so only the hidpi push should fire.
        let change = ViewportChange::between(
            &request(2880, 1800, 2.0, SERVO_DEFAULT_PAGE_ZOOM_PERCENT),
            &session(
                2880,
                1800,
                SERVO_DEFAULT_DEVICE_PIXEL_RATIO,
                SERVO_DEFAULT_PAGE_ZOOM_PERCENT,
            ),
        );
        assert_eq!(
            change,
            ViewportChange { resize: None, set_hidpi: Some(2.0), set_page_zoom: None },
        );
    }

    #[test]
    fn fresh_standard_dpi_session_pushes_nothing() {
        // On a 1.0-DPR display the fresh session already matches the
        // request — Servo's defaults are exactly what we asked for.
        let change = ViewportChange::between(
            &request(
                1280,
                720,
                SERVO_DEFAULT_DEVICE_PIXEL_RATIO,
                SERVO_DEFAULT_PAGE_ZOOM_PERCENT,
            ),
            &session(
                1280,
                720,
                SERVO_DEFAULT_DEVICE_PIXEL_RATIO,
                SERVO_DEFAULT_PAGE_ZOOM_PERCENT,
            ),
        );
        assert_eq!(change, ViewportChange::default());
    }

    #[test]
    fn page_zoom_change_pushes_only_zoom() {
        // User toggled zoom to 150 % on a session that's already at the
        // right size and DPR — only `set_page_zoom` should fire.
        let change = ViewportChange::between(
            &request(2880, 1800, 2.0, 150),
            &session(2880, 1800, 2.0, SERVO_DEFAULT_PAGE_ZOOM_PERCENT),
        );
        assert_eq!(
            change,
            ViewportChange { resize: None, set_hidpi: None, set_page_zoom: Some(150) },
        );
    }

    #[test]
    fn cross_monitor_dpr_change_pushes_only_hidpi() {
        // A window dragged from a 2.0-DPR monitor to a 1.0-DPR one keeps
        // the same physical surface (GPUI re-emits the viewport at the
        // new scale) — only the DPR push should fire.
        let change = ViewportChange::between(
            &request(2880, 1800, 1.0, SERVO_DEFAULT_PAGE_ZOOM_PERCENT),
            &session(2880, 1800, 2.0, SERVO_DEFAULT_PAGE_ZOOM_PERCENT),
        );
        assert_eq!(
            change,
            ViewportChange { resize: None, set_hidpi: Some(1.0), set_page_zoom: None },
        );
    }
}

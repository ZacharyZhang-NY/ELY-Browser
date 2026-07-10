use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc, Once,
        atomic::{AtomicBool, Ordering},
    },
};

use dpi::PhysicalSize;
use ely_domain::{ProfileId, TabId, WebViewId};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use servo::{
    DevicePoint, DeviceVector2D, Opts, Scroll, Servo, ServoBuilder, WebViewBuilder, WebViewPoint,
    WebViewVector,
};

#[path = "runtime_context.rs"]
mod runtime_context;
#[cfg(all(feature = "hardware-render", target_os = "macos"))]
#[path = "runtime_hardware.rs"]
mod runtime_hardware;
#[path = "runtime_paint.rs"]
mod runtime_paint;
#[path = "runtime_preferences.rs"]
mod runtime_preferences;

use runtime_context::hidpi_scale_from_factor;
pub use runtime_context::{RenderingContextKind, ServoSurfaceSize};
use runtime_preferences::ely_servo_preferences;
use url::Url;

use crate::{
    ConsumedPermission, HidpiScaleRequest, KeyboardTextRequest, MouseClickRequest,
    MouseDragRequest, MouseHoverRequest, NavigationRequest, PageZoomRequest,
    PermissionSnapshotRequest, RenderedFrame, ResizeRequest, ScrollRequest, ServoHost,
    ServoHostError, TouchTapRequest, WebViewSnapshot, WebViewState,
    runtime_input::{
        send_keyboard_text, send_mouse_click, send_mouse_drag, send_mouse_hover, send_touch_tap,
    },
    runtime_permissions::{
        PermissionStore, drain_consumed_permissions, replace_permission_decisions,
    },
    runtime_waker::ServoWakeFlag,
    runtime_webview::{HostWebView, HostWebViewDelegate},
};

static SERVO_RUNTIME_STARTED: AtomicBool = AtomicBool::new(false);
static RUSTLS_PROVIDER: Once = Once::new();

pub struct SoftwareServoHost {
    servo: Servo,
    default_surface_size: ServoSurfaceSize,
    rendering_context_kind: RenderingContextKind,
    webviews: HashMap<WebViewId, HostWebView>,
    permissions: PermissionStore,
    wake_requested: Arc<AtomicBool>,
    last_rendered_frame: Option<RenderedFrame>,
}

impl SoftwareServoHost {
    pub fn new(size: ServoSurfaceSize) -> Result<Self, ServoHostError> {
        Self::new_with_config_dir_and_kind(size, None, RenderingContextKind::Software)
    }

    pub fn new_with_config_dir(
        size: ServoSurfaceSize,
        config_dir: Option<PathBuf>,
    ) -> Result<Self, ServoHostError> {
        Self::new_with_config_dir_and_kind(size, config_dir, RenderingContextKind::Software)
    }

    /// Construct the host with an explicit [`RenderingContextKind`].
    pub fn new_with_config_dir_and_kind(
        size: ServoSurfaceSize,
        config_dir: Option<PathBuf>,
        rendering_context_kind: RenderingContextKind,
    ) -> Result<Self, ServoHostError> {
        if SERVO_RUNTIME_STARTED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(ServoHostError::RuntimeAlreadyStarted);
        }

        let host = Self::new_started(size, config_dir, rendering_context_kind);
        if host.is_err() {
            SERVO_RUNTIME_STARTED.store(false, Ordering::Release);
        }
        host
    }

    pub fn create_webview_with_size(
        &mut self,
        tab_id: TabId,
        profile_id: ProfileId,
        size: ServoSurfaceSize,
    ) -> Result<WebViewId, ServoHostError> {
        let handles = self.new_rendering_context(size)?;
        self.create_webview_in_context(tab_id, profile_id, handles)
    }

    pub fn create_webview_with_native_surface<S>(
        &mut self,
        tab_id: TabId,
        profile_id: ProfileId,
        size: ServoSurfaceSize,
        native_surface: &S,
    ) -> Result<WebViewId, ServoHostError>
    where
        S: HasDisplayHandle + HasWindowHandle + ?Sized,
    {
        let handles = self.new_rendering_context_for_native_surface(size, native_surface)?;
        self.create_webview_in_context(tab_id, profile_id, handles)
    }

    pub fn close_webview(&mut self, webview_id: &WebViewId) -> bool {
        self.webviews.remove(webview_id).is_some()
    }

    fn new_started(
        size: ServoSurfaceSize,
        config_dir: Option<PathBuf>,
        rendering_context_kind: RenderingContextKind,
    ) -> Result<Self, ServoHostError> {
        install_rustls_provider();
        let wake_requested = Arc::new(AtomicBool::new(false));
        let mut builder = ServoBuilder::default()
            .preferences(ely_servo_preferences())
            .event_loop_waker(Box::new(ServoWakeFlag::new(wake_requested.clone())));
        if let Some(config_dir) = config_dir {
            builder = builder.opts(Opts { config_dir: Some(config_dir), ..Opts::default() });
        }
        let servo = builder.build();

        Ok(Self {
            servo,
            default_surface_size: size,
            rendering_context_kind,
            webviews: HashMap::new(),
            permissions: PermissionStore::default(),
            wake_requested,
            last_rendered_frame: None,
        })
    }

    /// Returns the current snapshot and acknowledges metadata-only
    /// updates without clearing Servo's frame-ready signal.
    pub fn snapshot_and_mark_metadata_observed(
        &self,
        webview_id: &WebViewId,
    ) -> Result<WebViewSnapshot, ServoHostError> {
        let webview = self.webview(webview_id)?;
        let snapshot = webview.snapshot(webview_id);
        webview.delegate.mark_metadata_observed();
        Ok(snapshot)
    }

    fn drain_after_webview_close(&self) {
        for _ in 0..16 {
            self.servo.spin_event_loop();
        }
    }
}

fn install_rustls_provider() {
    RUSTLS_PROVIDER.call_once(|| {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}

impl ServoHost for SoftwareServoHost {
    fn create_webview(
        &mut self,
        tab_id: TabId,
        profile_id: ProfileId,
    ) -> Result<WebViewId, ServoHostError> {
        let handles = self.new_rendering_context(self.default_surface_size)?;
        self.create_webview_in_context(tab_id, profile_id, handles)
    }

    fn navigate(&mut self, request: NavigationRequest) -> Result<(), ServoHostError> {
        let url = Url::parse(request.url.as_str()).map_err(|_| {
            ServoHostError::InvalidNavigationUrl { value: request.url.as_str().to_string() }
        })?;
        self.servo.spin_event_loop();
        let servo = self.servo.clone();
        let webview = self
            .webviews
            .get_mut(&request.webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: request.webview_id.clone() })?;

        let requested_url = url.to_string();
        let has_loaded_page =
            matches!(webview.current_url().as_deref(), Some(value) if value != "about:blank");
        let should_create_initial_document = webview.requested_url.is_none() && !has_loaded_page;

        webview.delegate.set_state(WebViewState::Loading);
        if should_create_initial_document {
            let hidpi_scale_factor = webview.webview.hidpi_scale_factor();
            webview.webview = WebViewBuilder::new(&servo, webview.rendering_context.clone())
                .delegate(webview.delegate.clone())
                .url(url)
                // The live path pushes DPR before first navigation. Preserve that scale when
                // replacing the about:blank WebView so CSS viewport = physical surface / DPR.
                .hidpi_scale_factor(hidpi_scale_factor)
                .build();
            // The input-accepting invariant lives in `webview_for_input`.
            webview.webview.show();
            webview.webview.focus();
        } else {
            webview.webview.load(url);
        }
        webview.requested_url = Some(requested_url);
        Ok(())
    }

    fn scroll(&mut self, request: ScrollRequest) -> Result<(), ServoHostError> {
        if request.delta_x == 0 && request.delta_y == 0 {
            return Ok(());
        }

        let webview = self.webview_for_input(&request.webview_id)?;
        webview.webview.notify_scroll_event(
            Scroll::Delta(WebViewVector::Device(DeviceVector2D::new(
                request.delta_x as f32,
                request.delta_y as f32,
            ))),
            WebViewPoint::Device(DevicePoint::new(request.point_x as f32, request.point_y as f32)),
        );
        Ok(())
    }

    fn resize(&mut self, request: ResizeRequest) -> Result<(), ServoHostError> {
        let webview = self
            .webviews
            .get(&request.webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: request.webview_id.clone() })?;

        // `WebView::resize` is the single entry point — it routes through
        // `paint().resize_rendering_context`, which:
        //   1. early-returns if `rendering_context.size() == new_size`;
        //   2. calls `rendering_context.resize` itself;
        //   3. calls `webview_renderer.set_rect(new_viewport_rect)` so the
        //      compositor relays out the page at the new size; and
        //   4. sends `transaction.set_document_view(...)` so WebRender's
        //      document viewport matches the surface.
        //
        // Calling `rendering_context.resize` *ourselves* before that path
        // is what produced the original bug: surfman saw the new size,
        // Servo's early-return then skipped steps 3 and 4, and the page
        // stayed laid out at the original size while we presented a
        // larger surface — content collapsed to the top-left of a giant
        // viewport (StableLance) or rendered nothing at all (Google).
        // The Servo `winit_minimal` reference embedder calls
        // `webview.resize` and nothing else; mirror that exactly.
        let size = PhysicalSize::new(request.width, request.height);
        webview.webview.resize(size);
        Ok(())
    }

    fn set_page_zoom(&mut self, request: PageZoomRequest) -> Result<(), ServoHostError> {
        let webview = self
            .webviews
            .get(&request.webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: request.webview_id.clone() })?;

        webview.webview.set_page_zoom(request.zoom_factor);
        Ok(())
    }

    fn set_hidpi_scale(&mut self, request: HidpiScaleRequest) -> Result<(), ServoHostError> {
        let webview = self
            .webviews
            .get(&request.webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: request.webview_id.clone() })?;

        let scale = hidpi_scale_from_factor(request.scale_factor);
        webview.webview.set_hidpi_scale_factor(scale);
        Ok(())
    }

    fn hover(&mut self, request: MouseHoverRequest) -> Result<(), ServoHostError> {
        let webview = self.webview_for_input(&request.webview_id)?;
        send_mouse_hover(&webview.webview, request.x, request.y);
        Ok(())
    }

    fn click(&mut self, request: MouseClickRequest) -> Result<(), ServoHostError> {
        let webview = self.webview_for_input(&request.webview_id)?;
        send_mouse_click(&webview.webview, request.x, request.y);
        Ok(())
    }

    fn drag(&mut self, request: MouseDragRequest) -> Result<(), ServoHostError> {
        let webview = self.webview_for_input(&request.webview_id)?;
        send_mouse_drag(
            &webview.webview,
            request.from_x,
            request.from_y,
            request.to_x,
            request.to_y,
        );
        Ok(())
    }

    fn touch_tap(&mut self, request: TouchTapRequest) -> Result<(), ServoHostError> {
        let webview = self.webview_for_input(&request.webview_id)?;
        send_touch_tap(&webview.webview, request.x, request.y);
        Ok(())
    }

    fn type_text(&mut self, request: KeyboardTextRequest) -> Result<(), ServoHostError> {
        let webview = self.webview_for_input(&request.webview_id)?;
        send_keyboard_text(&webview.webview, &request.text);
        Ok(())
    }

    fn replace_permissions(
        &mut self,
        request: PermissionSnapshotRequest,
    ) -> Result<(), ServoHostError> {
        let webview = self
            .webviews
            .get(&request.webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: request.webview_id.clone() })?;
        if webview.profile_id != request.profile_id {
            return Err(ServoHostError::PermissionProfileMismatch {
                webview_id: request.webview_id,
                expected: webview.profile_id.clone(),
                actual: request.profile_id,
            });
        }
        let mut keys = HashSet::new();
        for entry in &request.entries {
            if !keys.insert((entry.origin.clone(), entry.feature)) {
                return Err(ServoHostError::DuplicatePermissionSnapshotEntry {
                    profile_id: request.profile_id,
                    origin: entry.origin.clone(),
                    feature: entry.feature,
                });
            }
        }
        replace_permission_decisions(
            &self.permissions,
            &request.profile_id,
            request.generation,
            request.entries,
        );
        Ok(())
    }

    fn take_consumed_permissions(&mut self) -> Vec<ConsumedPermission> {
        drain_consumed_permissions(&self.permissions)
    }

    fn state(&self, webview_id: &WebViewId) -> Result<WebViewState, ServoHostError> {
        Ok(self.webview(webview_id)?.state())
    }

    fn snapshot(&self, webview_id: &WebViewId) -> Result<WebViewSnapshot, ServoHostError> {
        self.webview(webview_id).map(|webview| webview.snapshot(webview_id))
    }

    fn tick(&mut self) -> bool {
        let requested = self.wake_requested.swap(false, Ordering::AcqRel);
        self.servo.spin_event_loop();
        requested
    }

    fn paint(&mut self, webview_id: &WebViewId) -> Result<(), ServoHostError> {
        self.paint_with_readback(webview_id)
    }

    fn last_rendered_frame(&self) -> Result<RenderedFrame, ServoHostError> {
        self.last_rendered_frame.clone().ok_or(ServoHostError::RenderedFrameUnavailable)
    }
}

impl Drop for SoftwareServoHost {
    fn drop(&mut self) {
        self.webviews.clear();
        self.drain_after_webview_close();
        SERVO_RUNTIME_STARTED.store(false, Ordering::Release);
    }
}

impl SoftwareServoHost {
    fn create_webview_in_context(
        &mut self,
        tab_id: TabId,
        profile_id: ProfileId,
        handles: runtime_context::RenderingContextHandles,
    ) -> Result<WebViewId, ServoHostError> {
        let webview_id = WebViewId::new();
        let delegate =
            Rc::new(HostWebViewDelegate::new(profile_id.clone(), self.permissions.clone()));
        let webview = WebViewBuilder::new(&self.servo, handles.rendering_context.clone())
            .delegate(delegate.clone())
            .build();
        // Cosmetic: makes the first frame paint into the rendering
        // context. The input-accepting invariant is owned by
        // `webview_for_input`, which re-asserts show/focus on every
        // dispatch — so a sibling tab's later creation (which would
        // steal focus here) cannot break input on this WebView.
        webview.show();
        webview.focus();

        self.webviews.insert(
            webview_id.clone(),
            HostWebView {
                tab_id,
                profile_id,
                rendering_context: handles.rendering_context,
                #[cfg(feature = "hardware-render")]
                hardware_context: handles.hardware_context,
                webview,
                delegate,
                requested_url: None,
            },
        );

        Ok(webview_id)
    }

    fn webview(&self, webview_id: &WebViewId) -> Result<&HostWebView, ServoHostError> {
        self.webviews
            .get(webview_id)
            .ok_or_else(|| ServoHostError::WebViewNotFound { id: webview_id.clone() })
    }

    /// Returns a WebView guaranteed to accept input.
    ///
    /// Servo's hit-test silently absorbs `notify_input_event` on a
    /// hidden or unfocused WebView. The create + first-navigate paths
    /// call `show()`/`focus()` for first-frame visibility, but every
    /// later operation that creates a sibling WebView (multi-tab)
    /// calls `focus()` on the new one, silently stealing focus from
    /// the foreground tab. `load()` (later navigates), `resize`,
    /// `set_page_zoom`, and `paint` do not re-focus, so by the time a
    /// click arrives the visible tab's WebView is unreachable.
    /// Re-asserting per dispatch keeps the invariant on the dispatch
    /// path instead of spread across creation, navigation, and
    /// tab-switching.
    fn webview_for_input(&self, webview_id: &WebViewId) -> Result<&HostWebView, ServoHostError> {
        let webview = self.webview(webview_id)?;
        webview.webview.show();
        webview.webview.focus();
        Ok(webview)
    }
}

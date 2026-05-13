use std::{
    cell::RefCell,
    collections::HashMap,
    env,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use dpi::PhysicalSize;
use ely_domain::{ProfileId, TabId, WebViewId};
use euclid::Scale;
use servo::{
    DeviceIndependentPixel, DeviceIntPoint, DeviceIntRect, DeviceIntSize, DevicePixel, DevicePoint,
    DeviceVector2D, Opts, RenderingContext, Scroll, Servo, ServoBuilder, WebViewBuilder,
    WebViewPoint, WebViewVector,
};

/// Wrap an `f32` scale factor in Servo's typed `Scale<f32, DeviceIndependentPixel,
/// DevicePixel>`. The clamp guards against `NaN`/`inf` reaching Servo's
/// layout (which assumes a positive finite scale).
fn hidpi_scale_from_factor(scale_factor: f32) -> Scale<f32, DeviceIndependentPixel, DevicePixel> {
    let safe = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor.clamp(0.5, 5.0)
    } else {
        1.0
    };
    Scale::new(safe)
}
use url::Url;

use crate::{
    HidpiScaleRequest, KeyboardTextRequest, MouseClickRequest, MouseDragRequest, MouseHoverRequest,
    NavigationRequest, PageZoomRequest, PermissionDecision, PermissionRequest, RenderedFrame,
    ResizeRequest, ScreenshotRequest, ScrollRequest, ServoHost, ServoHostError, TouchTapRequest,
    WebViewSnapshot, WebViewState,
    runtime_input::{
        send_keyboard_text, send_mouse_click, send_mouse_drag, send_mouse_hover, send_touch_tap,
    },
    runtime_permissions::{PermissionStore, set_permission_decision},
    runtime_waker::ServoWakeFlag,
    runtime_webview::{HostWebView, HostWebViewDelegate},
};

static SERVO_RUNTIME_STARTED: AtomicBool = AtomicBool::new(false);
const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(20);
const SCREENSHOT_POLL_INTERVAL: Duration = Duration::from_millis(2);

/// Default upper bound on how long `paint()` will spin the Servo event
/// loop waiting for `notify_new_frame_ready` after dispatching
/// `webview.paint()`. 32 ms is two 60 Hz frames — enough headroom for
/// the paint thread to land a real framebuffer before we read it back,
/// short enough that a stuck paint can't stall the input/render loop.
/// Overridable via `ELY_PAINT_BARRIER_MS`; `0` disables the barrier and
/// restores the pre-T15 "fire and read" behaviour.
const DEFAULT_PAINT_BARRIER_MS: u64 = 32;
const PAINT_BARRIER_POLL_INTERVAL: Duration = Duration::from_millis(2);

fn paint_barrier_budget() -> Duration {
    static BUDGET: OnceLock<Duration> = OnceLock::new();
    *BUDGET.get_or_init(|| {
        let ms = env::var("ELY_PAINT_BARRIER_MS")
            .ok()
            .and_then(|raw| raw.parse::<u64>().ok())
            .unwrap_or(DEFAULT_PAINT_BARRIER_MS);
        Duration::from_millis(ms)
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServoSurfaceSize {
    width: u32,
    height: u32,
}

impl ServoSurfaceSize {
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self { width: width.max(1), height: height.max(1) }
    }

    fn physical(self) -> PhysicalSize<u32> {
        PhysicalSize { width: self.width, height: self.height }
    }
}

/// Selects the `RenderingContext` implementation each webview gets.
///
/// `Software` uses Servo's built-in `SoftwareRenderingContext`, which
/// rasterises on the CPU. `Hardware` uses the vendored
/// [`HardwareOffscreenContext`](crate::HardwareOffscreenContext),
/// which rasterises through the real GPU adapter against a
/// `SurfaceType::Generic` offscreen surface. The `Hardware` variant
/// is only available when the `hardware-render` feature is enabled;
/// requesting it without the feature is a configuration error
/// surfaced via `ServoHostError::HardwareRenderUnavailable`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderingContextKind {
    #[default]
    Software,
    Hardware,
}

/// Pair of rendering-context handles produced by
/// [`SoftwareServoHost::new_rendering_context`]. The trait-object
/// handle drives Servo's compositor; the concrete hardware handle is
/// kept on the side so the host can call macOS-specific methods
/// (IOSurface mach port extraction) without downcasting.
struct RenderingContextHandles {
    rendering_context: Rc<dyn RenderingContext>,
    #[cfg(feature = "hardware-render")]
    hardware_context: Option<Rc<crate::HardwareOffscreenContext>>,
}

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
    ///
    /// `Hardware` requires the `hardware-render` feature; the call
    /// fails with `ServoHostError::HardwareRenderUnavailable` if the
    /// feature wasn't compiled in. This is the constructor the
    /// sidecar binary will use once a `--rendering-context` CLI
    /// flag lands; today the default path through `new` and
    /// `new_with_config_dir` keeps the software behaviour unchanged.
    pub fn new_with_config_dir_and_kind(
        size: ServoSurfaceSize,
        config_dir: Option<PathBuf>,
        rendering_context_kind: RenderingContextKind,
    ) -> Result<Self, ServoHostError> {
        if rendering_context_kind == RenderingContextKind::Hardware
            && !cfg!(feature = "hardware-render")
        {
            return Err(ServoHostError::HardwareRenderUnavailable);
        }

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
        self.create_webview_in_context(tab_id, profile_id, size)
    }

    fn new_started(
        size: ServoSurfaceSize,
        config_dir: Option<PathBuf>,
        rendering_context_kind: RenderingContextKind,
    ) -> Result<Self, ServoHostError> {
        let wake_requested = Arc::new(AtomicBool::new(false));
        let mut builder = ServoBuilder::default()
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
            permissions: Rc::new(RefCell::new(HashMap::new())),
            wake_requested,
            last_rendered_frame: None,
        })
    }
}

impl ServoHost for SoftwareServoHost {
    fn create_webview(
        &mut self,
        tab_id: TabId,
        profile_id: ProfileId,
    ) -> Result<WebViewId, ServoHostError> {
        self.create_webview_in_context(tab_id, profile_id, self.default_surface_size)
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
            webview.webview = WebViewBuilder::new(&servo, webview.rendering_context.clone())
                .delegate(webview.delegate.clone())
                .url(url)
                .build();
            // Cosmetic: makes the freshly built WebView paint its
            // first frame. The input-accepting invariant lives in
            // `webview_for_input`; we deliberately do not re-show or
            // re-focus on the `load()` branch so a background tab
            // finishing a load cannot steal focus from the foreground
            // tab between the user's mouse-down and the next render.
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

        let size = PhysicalSize::new(request.width, request.height);
        webview.rendering_context.resize(size);
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

    fn capture_screenshot(
        &mut self,
        request: ScreenshotRequest,
    ) -> Result<RenderedFrame, ServoHostError> {
        let webview = self.webview(&request.webview_id)?.webview.clone();
        let captured_image = Rc::new(RefCell::new(None));
        let callback_image = captured_image.clone();
        webview.take_screenshot(None, move |result| {
            callback_image.replace(Some(result));
        });

        let started_at = Instant::now();
        while captured_image.borrow().is_none() {
            if started_at.elapsed() >= SCREENSHOT_TIMEOUT {
                return Err(ServoHostError::ScreenshotTimedOut { id: request.webview_id.clone() });
            }

            self.tick();
            if self.snapshot(&request.webview_id)?.has_pending_frame() {
                self.paint(&request.webview_id)?;
            }
            thread::sleep(SCREENSHOT_POLL_INTERVAL);
        }

        let Some(result) = captured_image.borrow_mut().take() else {
            return Err(ServoHostError::RenderedFrameUnavailable);
        };
        let image = result.map_err(|error| ServoHostError::ScreenshotUnavailable {
            reason: format!("{error:?}"),
        })?;
        let frame = RenderedFrame::from_rgba_bytes(image.width(), image.height(), image.into_raw());
        self.last_rendered_frame = Some(frame.clone());
        Ok(frame)
    }

    fn set_permission(
        &mut self,
        request: PermissionRequest,
        decision: PermissionDecision,
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

        set_permission_decision(&self.permissions, request, decision);
        Ok(())
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
        let rendering_context = self.webview(webview_id)?.rendering_context.clone();
        rendering_context.make_current().map_err(|_| ServoHostError::RenderingContextNotCurrent)?;
        rendering_context.prepare_for_rendering();
        // `webview.paint()` dispatches a render command to Servo's paint
        // thread — it does NOT block until the framebuffer is consistent.
        // Without a barrier, `read_rendered_frame` below races the paint
        // thread and reliably reads the cleared-white state on data: URLs.
        // Clear the pending-frame flag first so we can detect the *next*
        // `notify_new_frame_ready` (the one our `paint()` triggers), then
        // pump the event loop until Servo reports the new frame is ready
        // or `paint_barrier_budget()` elapses. On timeout we fall through
        // and read anyway, preserving the pre-T15 fast path for callers
        // that explicitly disable the barrier with `ELY_PAINT_BARRIER_MS=0`.
        {
            let webview = self.webview(webview_id)?;
            webview.delegate.mark_frame_presented();
            webview.webview.paint();
        }
        self.wait_for_paint_completion(webview_id);
        let rendered_frame = Self::read_rendered_frame(rendering_context.as_ref())?;
        rendering_context.present();
        self.webview(webview_id)?.delegate.mark_frame_presented();
        self.last_rendered_frame = Some(rendered_frame);
        Ok(())
    }

    fn last_rendered_frame(&self) -> Result<RenderedFrame, ServoHostError> {
        self.last_rendered_frame.clone().ok_or(ServoHostError::RenderedFrameUnavailable)
    }
}

impl SoftwareServoHost {
    fn create_webview_in_context(
        &mut self,
        tab_id: TabId,
        profile_id: ProfileId,
        size: ServoSurfaceSize,
    ) -> Result<WebViewId, ServoHostError> {
        let webview_id = WebViewId::new();
        let handles = self.new_rendering_context(size)?;
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

    /// Cheap peek at the IOSurface identity bound to this webview's
    /// hardware context. Returns `None` for software webviews and on
    /// non-macOS hosts; otherwise the surfman `SurfaceID`-derived
    /// identity plus dimensions. Used by the sidecar's live loop to
    /// dedup mach port creation.
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    pub fn peek_iosurface_identity(
        &self,
        webview_id: &WebViewId,
    ) -> Result<Option<crate::IOSurfaceIdentity>, ServoHostError> {
        let webview = self.webview(webview_id)?;
        let Some(hardware) = webview.hardware_context.as_ref() else {
            return Ok(None);
        };
        hardware
            .peek_iosurface_identity()
            .map(Some)
            .map_err(|_| ServoHostError::RenderingContextUnavailable)
    }

    /// Mint a fresh mach port for the IOSurface bound to this
    /// webview's hardware context. The caller is responsible for
    /// transferring the port to the receiving process; if no transfer
    /// happens, the port leaks. Software webviews return `None`.
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    pub fn current_iosurface_handle(
        &self,
        webview_id: &WebViewId,
    ) -> Result<Option<crate::IOSurfaceHandle>, ServoHostError> {
        let webview = self.webview(webview_id)?;
        let Some(hardware) = webview.hardware_context.as_ref() else {
            return Ok(None);
        };
        hardware
            .current_iosurface_mach_port()
            .map(Some)
            .map_err(|_| ServoHostError::RenderingContextUnavailable)
    }

    fn new_rendering_context(
        &self,
        size: ServoSurfaceSize,
    ) -> Result<RenderingContextHandles, ServoHostError> {
        match self.rendering_context_kind {
            RenderingContextKind::Software => {
                let rendering_context = Rc::new(
                    servo::SoftwareRenderingContext::new(size.physical())
                        .map_err(|_| ServoHostError::RenderingContextUnavailable)?,
                );
                rendering_context
                    .make_current()
                    .map_err(|_| ServoHostError::RenderingContextNotCurrent)?;
                Ok(RenderingContextHandles {
                    rendering_context,
                    #[cfg(feature = "hardware-render")]
                    hardware_context: None,
                })
            }
            #[cfg(feature = "hardware-render")]
            RenderingContextKind::Hardware => {
                let hardware = Rc::new(
                    crate::HardwareOffscreenContext::new(size.physical())
                        .map_err(|_| ServoHostError::RenderingContextUnavailable)?,
                );
                hardware.make_current().map_err(|_| ServoHostError::RenderingContextNotCurrent)?;
                Ok(RenderingContextHandles {
                    rendering_context: hardware.clone(),
                    hardware_context: Some(hardware),
                })
            }
            #[cfg(not(feature = "hardware-render"))]
            RenderingContextKind::Hardware => Err(ServoHostError::HardwareRenderUnavailable),
        }
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

    /// Spin Servo's event loop until the webview's delegate observes a
    /// fresh `notify_new_frame_ready` callback (i.e. the framebuffer is
    /// consistent for readback) or [`paint_barrier_budget`] elapses. The
    /// caller is responsible for clearing the pending-frame flag before
    /// dispatching `webview.paint()`; otherwise this returns immediately
    /// off the *previous* frame and the race is preserved.
    ///
    /// Returns silently on timeout — `paint()` falls through to
    /// `read_rendered_frame` so callers still get whatever pixels the
    /// rendering context currently holds. That keeps the fast path open
    /// when `ELY_PAINT_BARRIER_MS=0` disables the budget entirely, and
    /// matches the pre-T15 behaviour on the (rare) case where Servo
    /// can't land a frame inside two refresh intervals.
    fn wait_for_paint_completion(&mut self, webview_id: &WebViewId) {
        let budget = paint_barrier_budget();
        if budget.is_zero() {
            return;
        }
        let started_at = Instant::now();
        loop {
            self.servo.spin_event_loop();
            let ready = self
                .webviews
                .get(webview_id)
                .is_some_and(|webview| webview.delegate.has_pending_frame());
            if ready {
                return;
            }
            if started_at.elapsed() >= budget {
                return;
            }
            thread::sleep(PAINT_BARRIER_POLL_INTERVAL);
        }
    }

    fn read_rendered_frame(
        rendering_context: &dyn RenderingContext,
    ) -> Result<RenderedFrame, ServoHostError> {
        let size = rendering_context.size();
        let width =
            i32::try_from(size.width).map_err(|_| ServoHostError::RenderedFrameUnavailable)?;
        let height =
            i32::try_from(size.height).map_err(|_| ServoHostError::RenderedFrameUnavailable)?;
        let frame_rect = DeviceIntRect::from_origin_and_size(
            DeviceIntPoint::new(0, 0),
            DeviceIntSize::new(width, height),
        );
        let image = rendering_context
            .read_to_image(frame_rect)
            .ok_or(ServoHostError::RenderedFrameUnavailable)?;

        Ok(RenderedFrame::from_rgba_bytes(size.width, size.height, image.into_raw()))
    }
}

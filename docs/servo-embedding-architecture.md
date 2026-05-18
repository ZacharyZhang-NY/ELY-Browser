# Servo Embedding Architecture

## Decision

ELY is a Servo-based browser. The page renderer lives in the application process and follows Servo's embedder model:

```text
┌──────────────────────────── ELY App Process ────────────────────────────┐
│                                                                          │
│  GPUI chrome                                                              │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │ Sidebar  Toolbar  Tabs  Settings                                  │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                          │
│  Servo content host                                                       │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │ Servo + WebView + RenderingContext                                │  │
│  │ notify_new_frame_ready -> repaint -> paint -> present             │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

The app process owns WebView lifecycle, navigation, input, permissions, frame readiness, and rendering context presentation.

## Current Code Boundary

```text
WebSurfaceStore
  -> GPUI NativeSurface
  -> LiveRuntimeWorker
  -> ServoLiveClient
  -> SoftwareServoHost
  -> servo::Servo + servo::WebView + servo::WindowRenderingContext
```

`ServoLiveClient` is now an in-process adapter over `ely_servo_host::SoftwareServoHost`. It shares one Servo runtime across profile scopes, creates multiple WebViews inside that runtime, and routes scroll, hover, click, keyboard, resize, zoom, navigation, and permissions through the `ServoHost` API.

Normal page display now enters GPUI through `native_surface(...)`. GPUI creates a platform child surface for the content bounds, passes that raw handle into Servo's `WindowRenderingContext`, and presents with `paint_without_readback_with_completion`. The RGBA readback path remains inside `ely_servo_host` for low-level tests.

Current platform child surfaces:

- macOS: child `NSView` with an AppKit raw window handle.
- Windows: child `HWND` with a Win32 raw window handle.
- Linux/X11: child XCB window with XCB display/window handles.
- Linux/Wayland: child `wl_surface` attached as a `wl_subsurface` with Wayland display/surface handles.

## Upstream Servo Route

Servo's own shell route is the model for the final ELY rendering path:

```text
Window event
  -> Servo spin_event_loop
  -> WebViewDelegate::notify_new_frame_ready
  -> window request_redraw
  -> WebView::paint
  -> RenderingContext::present
```

Relevant upstream evidence from Servo `7c48af7`:

- `ports/servoshell/window.rs` creates `WebViewBuilder::new(state.servo(), platform_window.rendering_context())`.
- `ports/servoshell/window.rs` repaints with `webview.paint()` and `rendering_context().present()`.
- `ports/servoshell/running_app_state.rs` handles `notify_new_frame_ready` by marking the owning window for repaint.
- `components/paint/paint.rs` owns one WebRender painter per `RenderingContext` and keeps paint coordination inside Servo's rendering pipeline.

## Target Boundaries

```text
crates/ely_browser_core
  Owns browser domain state: tabs, spaces, profiles, search, settings.

crates/ely_app/src/shell
  Owns GPUI chrome and command surfaces.

crates/ely_app/src/servo_embed
  Owns in-process Servo runtime, platform content view attachment,
  WebView lifecycle, repaint dispatch, and web input routing.

crates/ely_servo_host
  Owns the current in-process compatibility adapter while the native
  platform rendering surface lands.
```

Each production source file in the final embedding path stays below 500 lines. Large responsibilities split by ownership:

- `runtime.rs`: `Servo`, wake handling, webview registry.
- `platform_view.rs`: platform content surface attachment.
- `delegate.rs`: Servo `WebViewDelegate` implementation.
- `input.rs`: GPUI event to Servo input conversion.
- `paint.rs`: repaint and present coordination.
- `metadata.rs`: title, URL, favicon, load-state propagation.

## Migration Slices

1. App process owns the Servo runtime and WebView registry.
2. App process routes web input and navigation directly into Servo.
3. GPUI exposes one `NativeSurfaceHandle` element contract for platform child surfaces.
4. macOS creates a child `NSView` and hands the AppKit raw handle to Servo.
5. Windows creates a child `HWND` and hands the Win32 raw handle to Servo.
6. Linux/X11 creates a child XCB window and hands the XCB raw handle to Servo.
7. Linux/Wayland creates a child `wl_surface`/subsurface and hands the Wayland raw handle to Servo.
8. GPUI web page display uses native content surface presentation for every platform target.

## Acceptance Gates

- `cargo run` opens a normal web page with Servo inside the ELY app process.
- Scrolling a live web page uses Servo input events and Servo repaint callbacks.
- Page display presents through a Servo rendering context.
- macOS, Windows, Linux/X11, and Linux/Wayland each have a platform child surface implementation under the same `NativeSurfaceHandle` contract.
- Address/search, tabs, spaces, profiles, settings, permissions, and sync compile through existing domain APIs.
- Every new source file stays below 500 lines.
- User-facing chrome stays clean.

## First Platform Surface Target

```text
GPUI Window
  -> GPUI NativeSurface element
  -> platform child surface for web content bounds
  -> Servo WindowRenderingContext
  -> Servo WebView
```

This slice gives Servo a native surface in the ELY app process. The compatibility adapter remains a narrow bridge for tests and non-normal display probes.

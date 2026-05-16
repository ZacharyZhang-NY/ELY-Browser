# Servo Embedding Architecture

## Decision

ELY is a Servo-based browser. The page renderer is Servo itself, embedded in the application process and attached to a real platform rendering surface. The browser chrome can stay GPUI, while web content must follow Servo's embedder model:

```text
┌──────────────────────────── ELY App Process ────────────────────────────┐
│                                                                         │
│  GPUI chrome                                                             │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │ Sidebar  Toolbar  Tabs  Settings                                 │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                                                                         │
│  Servo content host                                                      │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │ Servo + WebView + WindowRenderingContext                         │  │
│  │ notify_new_frame_ready -> window repaint -> paint -> present      │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

The normal page-display path excludes external rendering sidecars, stdout frame transport, RGBA frame payloads, cross-process IOSurface handoff, and GPUI `RenderImage` uploads for live web content.

## Root Cause

The current ELY page path is a remote-frame architecture:

```text
GPUI shell
  -> WebSurfaceStore
  -> LiveRuntimeWorker
  -> ServoLiveClient
  -> ely_servo_sidecar stdin/stdout JSON
  -> SoftwareServoHost
  -> Servo WebView paint
  -> RGBA payload or IOSurface handle
  -> GPUI surface/image element
```

This makes page interaction depend on worker scheduling, IPC, polling cadence, surface import, frame object churn, and GPUI scene refresh. Hardware IOSurface reduces byte volume, while the architecture still behaves like a remote compositor.

Servo's own embedder route is direct:

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
- `components/paint/paint.rs` owns one WebRender painter per `RenderingContext` and explicitly avoids blocking paint on the constellation.

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
  Transitional compatibility surface for explicit screenshots and isolated
  compatibility tools. Normal live page display leaves this crate.
```

Each production source file in the new embedding path stays below 500 lines. Large responsibilities split by ownership:

- `runtime.rs`: `Servo`, wake handling, webview registry.
- `platform_view.rs`: platform content surface attachment.
- `delegate.rs`: Servo `WebViewDelegate` implementation.
- `input.rs`: GPUI event to Servo input conversion.
- `paint.rs`: repaint and present coordination.
- `metadata.rs`: title, URL, favicon, load-state propagation.

## Migration Slices

1. Create `servo_embed` as an in-process module behind the existing tab/domain model.
2. Add a macOS platform content view using the GPUI window's raw AppKit handle.
3. Build Servo `WindowRenderingContext` or child context against the platform content view.
4. Move one active tab to in-process `Servo + WebView + RenderingContext`.
5. Route scroll, mouse, keyboard, resize, zoom, and navigation directly into the active `WebView`.
6. Replace `WebSurfaceStore` for normal web pages with the in-process host.
7. Delete sidecar spawning from normal page display.
8. Keep explicit page screenshot capture as a user-command path only.

## Acceptance Gates

- `cargo run` opens a normal web page without starting `ely_servo_sidecar`.
- Scrolling a live web page uses Servo input events and Servo repaint callbacks.
- The page display path contains no RGBA frame payload transport.
- The page display path contains no stdout JSON frame loop.
- The page display path contains no GPUI `RenderImage` upload for live web content.
- Address/search, tabs, spaces, profiles, settings, permissions, sync, and explicit screenshots continue to compile and behave through existing domain APIs.
- Every new source file stays below 500 lines.
- No user-facing frontend status, logs, debug panels, or explanatory clutter are added.

## First Implementation Target

The first code slice is macOS content-view attachment:

```text
GPUI Window
  -> raw AppKit NSView
  -> ELY child NSView for web content bounds
  -> Servo WindowRenderingContext
  -> Servo WebView
```

This slice creates the real platform surface required by Servo's direct rendering model. Once the content view exists, Servo can paint into a native surface in the ELY app process, and the remote-frame path can be removed tab by tab.

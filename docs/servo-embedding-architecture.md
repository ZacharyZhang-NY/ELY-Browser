# Servo Embedding Architecture

## Decision

ELY runs one Servo process for each browser profile. Servo's configuration and site-data manager
are process-scoped, so the process boundary is the storage partition boundary for cookies, HTTP
cache, local storage, and other profile data.

```text
ELY App Process
  GPUI chrome
  WebSurfaceStore
    Profile Runtime Broker
      Profile A worker ── stdio IPC ──> Servo sidecar A ──> profiles/A/servo
      Profile B worker ── stdio IPC ──> Servo sidecar B ──> profiles/B/servo
      Private worker   ── stdio IPC ──> Servo sidecar P ──> transient directory
```

Each sidecar binds to the first `ProfileId` it receives and rejects requests for another profile.
The parent broker also keys workers by `ProfileId + ProfileDataMode` and discards responses whose
scope no longer matches the tab session.

## Current Code Boundary

```text
WebSurfaceStore
  -> WebSurfaceRuntime
  -> one LiveRuntimeWorker per profile scope
  -> ServoLiveClient
  -> ely_servo_sidecar live
  -> SoftwareServoHost
  -> servo::Servo + servo::WebView
     macOS
       -> HardwareOffscreenContext (CGL + surfman)
       -> IOSurface + Mach descriptor
       -> CVPixelBuffer
       -> GPUI surface element
     Linux / Windows
       -> SoftwareRenderingContext
       -> RGBA frame
       -> GPUI RenderImage
```

The app process owns browser state, profile routing, input coalescing, frame cadence, and GPUI
presentation. A sidecar owns one Servo runtime, its WebViews, and the profile's persistent storage.
The line-delimited JSON protocol carries commands and frame metadata. Software frames append an
exact `width * height * 4` RGBA payload. macOS hardware frames set `rgba_byte_count` to zero and
select an imported IOSurface through `surface_handle` and `current_surface_id`. The Mach channel
transfers the IOSurface send right; the JSON port number is diagnostic metadata.

The protocol supports:

- `handshake`: verify protocol version `3` before accepting browser commands.
- `ensure`: create or update a WebView, navigation, viewport, zoom, permissions, and input.
- `poll`: advance Servo and return pending frame or metadata state.
- `close`: destroy one tab's WebView.
- `shutdown`: acknowledge graceful process shutdown so Servo flushes profile storage.

Each `ensure` carries the complete permission snapshot for its Profile. The sidecar
atomically replaces that Profile at the newest request generation, so cross-tab round-robin cannot
restore an older grant. Entries carry per-key revisions plus explicit `transferred-allow-once`
state and full-snapshot removal semantics. BrowserCore moves an accepted one-time grant into
revocable transferred state, the worker preserves the grant request from idle
coalescing, and the sidecar returns a consumption receipt after the first matching request. A fresh
worker interprets transferred state as consumed and returns the same completion receipt, which
keeps process-restart behavior fail closed.

`ensure` and `poll` carry `ready_surface_ids` and `pending_surface_ids`. The app distinguishes
completed imports from handles queued behind bounded importer backpressure. Cache entries retain
the IOSurface reference and keep only a weak reference to an active `CVPixelBuffer` backing. The
sidecar preserves pending publications, republishes a surface missing from both sets, and replays
the cached surface report once after an asynchronous import completes.

The release app locates an adjacent `ely_servo_sidecar` binary. `scripts/run_dev.sh` builds the
locked sidecar before starting the app and points the runtime at that binary. The macOS bundle
script places release executables in `Contents/MacOS`, installs `AppIcon.icns`, and verifies its
ad-hoc code signature. The native distribution script places adjacent release executables in one
directory for macOS, Linux, and Windows packaging inputs.

## Profile Data Contract

Persistent scopes use:

```text
<application-data-root>/profiles/<profile-id>/servo
```

The default Standard Profile stores its generated identity at
`<application-data-root>/profiles/default/profile-id`; later launches restore that identity before
constructing `BrowserCore`, so the persistent Servo path remains stable.

Transient scopes receive a `0700` directory under the user-specific ELY runtime directory on Linux
and the user-specific ELY cache directory on macOS and Windows. The root rejects symbolic links. A
root lock serializes creation and stale cleanup; each live directory holds an exclusive lease so
concurrent ELY instances preserve one another's data. Closing the final tab retires the profile
worker in the background, waits for sidecar shutdown, and then removes the directory. A new Private
scope starts immediately in a fresh random directory while earlier cleanup completes. Startup
removes unlocked crash remnants. A sidecar receives its directory at process launch and keeps it
for its process lifetime.

The integration gates prove these invariants with real Servo networking:

1. Cookie and local-storage values survive a graceful restart with the same directory.
2. A fresh directory starts with empty cookie and local-storage state.
3. Two simultaneous app-level profiles keep cookies, local storage, and HTTP cache isolated.
4. Reopening a tab in each profile observes only that profile's state and cached response.
5. Closing the final Private tab and reopening the same profile starts with empty site data.

## Upstream Servo Route

ELY pins Servo upstream commit `bc469fd5c17137373458508f88c3907cd1fcb69a`, workspace version
`0.4.0`, in `Cargo.toml` and `Cargo.lock`.

Servo's rendering lifecycle remains authoritative inside each sidecar:

```text
command or poll
  -> Servo spin_event_loop
  -> WebViewDelegate::notify_new_frame_ready
  -> WebView::paint
  -> RenderingContext frame
  -> sidecar response
```

Relevant upstream evidence at the pinned revision:

- `ports/servoshell/window.rs` builds WebViews from a Servo instance and rendering context.
- `ports/servoshell/window.rs` repaints with `webview.paint()` and presents the context.
- `ports/servoshell/running_app_state.rs` maps `notify_new_frame_ready` to repaint scheduling.
- `components/paint/paint.rs` owns one WebRender painter per rendering context.

## Ownership

```text
crates/ely_browser_core
  Browser domain state: tabs, spaces, profiles, search, settings.

crates/ely_app/src/shell
  GPUI chrome, profile broker, frame cadence, input, and presentation.

crates/ely_app/src/services
  Sidecar discovery, process lifecycle, and wire client.

crates/ely_servo_host
  Servo host API, rendering contexts, sidecar protocol, and process entry point.
```

Each production source file in the embedding path stays below 500 lines. Responsibilities remain
split by lifecycle, protocol, session state, frame output, and browser-shell orchestration.

## Rendering Transports

### macOS hardware transport

```text
Servo WebView::paint
  -> CGL HardwareOffscreenContext
  -> surfman swap chain
  -> IOSurface
  -> IOSurfaceCreateMachPort
  -> mach_msg port descriptor
  -> IOSurfaceLookupFromMachPort
  -> CVPixelBuffer
  -> GPUI surface element
  -> Metal BGRA texture
```

The parent creates a unique bootstrap Mach service before spawning the sidecar. The sidecar looks
up that service and moves each new IOSurface send right in a complex Mach message. The app verifies
the system IOSurface ID and dimensions, wraps the imported surface in a `CVPixelBuffer`, and
releases the received Mach right. The cache retains up to 16 inactive IOSurfaces and temporarily
retains additional surfaces whose GPU backing is still active.

Each hardware frame carries `current_surface_id`. The first frame for an IOSurface also carries its
dimensions and system surface ID in `surface_handle`. GPUI presents the selected `CVPixelBuffer`
through its surface element and macOS Metal BGRA pipeline. This path keeps pixels on the GPU side of
the profile process boundary.

CoreVideo raises the IOSurface use count while a `CVPixelBuffer` backing is alive. The frame and
GPUI `SurfaceLease` share that backing, and the Metal command-buffer completion handler releases
the final GPU lease. The host retains presented surfman surfaces and returns an acknowledged,
non-current surface to the swap chain only after `IOSurfaceIsInUse` becomes false. A sidecar waits
for the app's import acknowledgement before painting the next hardware frame.

The macOS parent selects hardware rendering by default. Setting
`ELY_SERVO_RENDERING_CONTEXT=software` selects the RGBA path for software-specific diagnostics.

### Linux and Windows software transport

Linux and Windows use Servo's `SoftwareRenderingContext`. The sidecar paints into RGBA8 memory,
writes the JSON frame header, then writes the bounded raw payload on stdout. The app validates the
dimensions and byte count before allocating and creates a GPUI `RenderImage`.

## Acceptance Gates

- Workspace format, check, lint, and unit tests pass.
- Servo host check and lint pass with `servo-engine,hardware-render` enabled.
- The macOS hardware-context test presents a real IOSurface and exports a live Mach send right.
- The hardware sidecar test transfers that right across processes and resolves it with
  `IOSurfaceLookupFromMachPort`.
- The app hardware test imports a real BGRA `CVPixelBuffer` and verifies its GPUI lease releases the
  IOSurface use count.
- Sidecar persistence integration passes against a loopback HTTP server.
- PRD live-site tests pass through the GPUI web-surface adapter.
- Profile Cookie, local-storage, and HTTP-cache isolation passes through two live sidecars.
- The real-window render probe captures visible page content.
- The macOS application bundle contains both executables in `Contents/MacOS`.
- Native distributions place both executables at the distribution root.
- Every production source file stays below 500 lines.

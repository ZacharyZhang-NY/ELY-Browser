# T10 — Zero-copy live frames via OffscreenRenderingContext + IOSurface

## Why this exists

Today every live frame walks an entirely CPU-side pipeline:

```
Servo SoftwareRenderingContext (CPU rasterise)
  → read_to_image: GPU(software)→CPU RGBA8 (8 MB / 1080p)
  → stdout pipe (one memcpy into kernel buffer, one out)
  → ServoLiveFrame::from_parts (move Vec, no copy)
  → AHasher 8 MB (~0.8 ms after the T10 hash swap)
  → ImageBuffer::from_raw + Arc<RenderImage> (cache hit reuses)
  → GPUI uploads as Metal texture and samples it as an Image
```

`Software` rasterising is intrinsically slow — Servo's compositor
walks the display list on a CPU thread and writes pixels into a host
buffer. The compositor + readback together are most of a `paint()`'s
wall-clock cost at 1080p. Even with the rest of the pipeline polished
(file pipe gone in `a80d039`, host-side `Vec` clone gone in `e02c0fd`,
identical-frame texture reuse in `7f3b8b4`, AHasher hot path), the
fundamental work — drawing pixels with the CPU and then handing the
host CPU buffer to the GPU — is what makes scroll feel non-native.

The roundtable (rounds 1 & 2 in this directory's git log) converged
on the same target: **let Servo paint to a GPU surface that the GPUI
window can sample directly, no host memory in the loop**. On macOS
that surface is an `IOSurface`. Brave/Chromium's GPU process
publishes IOSurfaces this way and the renderer process samples them
through a Mach port.

## What Servo gives us today

`servo-paint-api 0.1` exposes three `RenderingContext` constructors:

| Type | Backing | Headless? | IOSurface-backed on macOS? |
|---|---|---|---|
| `SoftwareRenderingContext` | software surfman adapter, CPU pixel buffer | yes | no |
| `WindowRenderingContext` | hardware surfman adapter, surface bound to a `RawWindowHandle` | no — needs a real window | yes (CGL backend uses IOSurface) |
| `OffscreenRenderingContext` | child of a `WindowRenderingContext`, paints into a separate framebuffer and blits back via `render_to_parent_callback` | not standalone | inherits parent's backing |

The hard constraint: **the only GPU-backed constructor requires a
`DisplayHandle + WindowHandle`**. Servo does not currently expose a
"headless hardware" rendering context that we could create from the
sidecar process without owning a window.

`SurfmanRenderingContext`, the underlying type, is in the same file
and IS hardware-capable headless — `Connection::new() →
create_adapter() → SurfmanRenderingContext::new` with a `Generic`
surface type would give us a hardware-backed offscreen context.
But its constructor is `fn new` (private). Reaching it requires
either patching `servo-paint-api` upstream or vendoring a thin wrapper.

## Target architecture

```
[Sidecar process]                          [Main GPUI process]
─────────────────                          ───────────────────
WebView paints with                        GPUI Metal/Blade
hardware compositor                                ▲
  │                                                │
  ▼                                          sample external texture
OffscreenRenderingContext                          │
(surfman hardware adapter)                  Metal MTLTexture
  │                                          (backed by IOSurface)
  ▼                                                ▲
IOSurface (Generic surface,                        │
CGL backend on macOS)                              │
  │                                                │
  ▼                                                │
extract IOSurface mach port name      ──── share via JSON header ───►
                                                   │
                                                   ▼
                                          import IOSurface as Metal
                                          texture (one-time per surface)
```

Per-frame: zero CPU memcpy, zero pipe traffic beyond the JSON header.
The sidecar only writes a small notification (`{"new_frame_seq": N,
"surface_id": "ioservice-port", "width": …, "height": …}`); the main
process re-samples the SAME texture (its contents have changed in
place).

## Stepping stones

### 1. Reorganise `ServoHost` to abstract over rendering-context kind

Today `SoftwareServoHost` hard-wires `SoftwareRenderingContext`. Split
the host into:

  * `ServoHost` trait (existing) — describes the embedder API surface
  * `SoftwareServoHost` (current) — keeps the CPU path running, no
    behaviour change
  * `HardwareServoHost` (new) — built on a hardware surfman context

Both implement the same `ServoHost` trait so the sidecar binary picks
one via CLI flag or environment, and the `live.rs` plumbing doesn't
know which is active. This is purely a refactor with no perf change;
it unlocks step 2.

### 2. Add a hardware headless rendering context

The cleanest path is a tiny vendored adapter that exposes
`SurfmanRenderingContext::new` directly with `create_adapter()` and a
`Generic` `SurfaceType`. The Servo crate's private constructor means
we either:

  (a) **Upstream contribute**: open a Servo PR adding
      `HardwareOffscreenRenderingContext` to `servo-paint-api`. Highest
      quality option; long round-trip with Servo maintainers.

  (b) **Vendor the relevant types into `ely_servo_host`**: copy the
      ~200 lines of `SurfmanRenderingContext` glue with a
      `pub fn new_headless_hardware(...)` constructor. Keeps the
      change inside our tree; risk is drifting against upstream.

  (c) **Open an upstream RFC for the API gap** while shipping (b)
      behind a feature flag, with the explicit intent of removing it
      once Servo merges (a).

Recommend (c): ship (b) under `feature = "iosurface"`, keep
`SoftwareServoHost` as the default until upstream lands.

### 3. macOS: extract the IOSurface from the surfman surface

surfman exposes the raw native handle on macOS via
`surfman::Surface::native_id()`. On the CGL backend the underlying
storage is an `IOSurface`. We need the `IOSurfaceRef`'s **mach port
name** (`IOSurfaceCreateMachPort`) to share it across processes.
This is a few lines of `core-foundation` + `objc2-io-surface` FFI.

### 4. Plumb the IOSurface mach port from sidecar to main

Extend the `LiveResponse` JSON header with an optional
`surface_handle: Option<IOSurfaceHandle>` where
`IOSurfaceHandle { mach_port_name: u32, width: u32, height: u32 }`.
On the FIRST frame after a resize the sidecar publishes a new handle;
subsequent frames reuse the same handle (the IOSurface contents have
been overwritten in place by the GPU, no further protocol needed).

### 5. Main process: import IOSurface as Metal external texture

GPUI uses Blade (or wgpu) as its render backend. Blade's Metal
backend has `Texture::from_iosurface` (or wgpu's
`Device::create_texture_from_hal` with a Metal hal texture built
from `MTLDevice::newTextureWithDescriptor:iosurface:plane:`). The
GPUI side needs:

  * a small bridge crate (or unsafe block) that wraps the mach port
    → `IOSurfaceRef` → `MTLTexture` chain
  * an `ImageSource` variant that carries an MTLTexture handle and
    bypasses the `RenderImage` + `ImageBuffer` allocation chain

The latter is the biggest reach into GPUI's public surface. Likely
needs an upstream gpui contribution or a local fork.

### 6. Replace the `Arc<RenderImage>` path for live frames

`WebSurfaceFrame::image: Arc<RenderImage>` becomes an enum:

```rust
enum WebSurfaceImage {
    Software(Arc<RenderImage>),  // fallback path, T6 hash dedup applies
    Hardware(MetalTextureHandle), // zero-copy path
}
```

`render_ready_web_surface` chooses the right `img(...)` / Metal
sampler based on the variant.

## Risks & open questions

  * **Servo upstream API gap** is the gating issue. Without step 2
    landing somehow, none of the rest is possible from a clean
    sidecar process.
  * **Cross-process IOSurface lifecycle**: if the sidecar crashes
    while the main process still holds an `MTLTexture`, the texture
    is dangling. Mach ports survive briefly; we need a "surface
    invalidated" notification on the IPC channel.
  * **GPU adapter compatibility**: surfman's hardware adapter on
    macOS picks the integrated GPU by default. GPUI may pick a
    discrete GPU. Mismatched adapters → IOSurface import either fails
    or silently corrupts. Need to either query GPUI's chosen adapter
    and force surfman to match, or use the system's default for both.
  * **Windows / Linux**: IOSurface is macOS-only. The same concept
    on Windows is `IDXGIResource1::CreateSharedHandle`; on Linux it's
    `EGL_EXT_image_dma_buf_import`. Each platform needs its own
    bridge; the JSON protocol stays the same, the bridge differs.
  * **Software fallback stays in**: not just because step 2 is
    blocked, but because some environments (CI, headless tests,
    sandboxed Mac App Store builds) may not allow GPU contexts.

## Already shipped on this branch

| Commit | Move |
|---|---|
| `840255f` | Lift web canvas out of in-flow so input_overlay lands on screen (prerequisite for input to work at all) |
| `a80d039` | Drop the file system from the live frame pixel pipe (8 MB syscall round-trip → in-process pipe) |
| `e02c0fd` | Drop the sidecar's per-frame `to_vec()` clone (extra 8 MB memcpy gone) |
| `7f3b8b4` | Dedup byte-identical RGBA payloads against the last frame's `Arc<RenderImage>` (idle pages stop re-uploading) |
| AHasher swap | SipHash13 → AHash for the dedup key (~5 ms → ~0.8 ms per cache-miss frame at 1080p) |

Each of these is a stepping stone; the IOSurface path eventually
deletes most of them (the host-side `Vec<u8>` lifecycle disappears
when GPU memory is the source of truth), but they make the current
software path's tail latency tolerable while the architectural work
above gets staged.

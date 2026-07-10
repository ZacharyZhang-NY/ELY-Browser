# ELY Browser

Native Rust + GPUI browser workspace for ELY Browser by Elydora.

The repository is organized around explicit product boundaries from `PRD.md`:
domain state, browser orchestration, design tokens, GPUI shell, and Servo host contracts.
Reference GPUI ecosystem repositories live under `references/` for local review and are excluded
from version control.

## Build Prerequisites

- Rust toolchain from `rust-toolchain.toml` (installed automatically by `rustup`).
- **macOS:** the full **Xcode** app plus its **Metal Toolchain** component. GPUI
  compiles Metal shaders at build time with `xcrun metal`, which the standalone
  Command Line Tools do not provide — a plain `cargo run` under Command Line
  Tools fails with `xcrun: error: unable to find utility "metal"`. Install it once:

  ```bash
  DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer \
    xcodebuild -downloadComponent MetalToolchain
  ```

  `scripts/run_dev.sh` then resolves a Metal-capable `DEVELOPER_DIR` automatically
  and stops with actionable guidance if none is found. Run the desktop shell with
  it rather than a bare `cargo run` — it also builds the Servo sidecar and wires
  `ELY_SERVO_SIDECAR`:

  ```bash
  scripts/run_dev.sh https://servo.org
  ```

- **Linux:** the native dependencies the CI `portable` job installs
  (`libasound2-dev libfontconfig1-dev libssl-dev libwayland-dev libx11-dev
  libx11-xcb-dev libxkbcommon-x11-dev libxrandr-dev`).
- **Windows:** the MSVC toolchain (Visual Studio Build Tools).

Linux and Windows render web content through Servo's software RGBA path, so they
need no Metal/Xcode; `scripts/run_dev.sh` selects the software context there.

## Local Commands

```bash
cargo --locked fmt --all --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
cargo check --locked -p ely_servo_host --features servo-engine,hardware-render --all-targets
cargo clippy --locked -p ely_servo_host --features servo-engine,hardware-render --all-targets -- -D warnings
cargo test --locked -p ely_servo_host --features servo-engine,hardware-render --all-targets -- --test-threads=1
scripts/verify_prd_site_rendering.sh
scripts/verify_windows_app_manifest.sh
scripts/create_macos_app_bundle.sh
scripts/create_native_distribution.sh
scripts/run_dev.sh
```

The macOS hardware tests cover the CGL rendering context, IOSurface export, sidecar Mach descriptor
transfer, GPUI surface lease, and IOSurface use-count release. `scripts/verify_prd_site_rendering.sh`
keeps the software RGBA assertions.

The local macOS QA bundle places both release executables in `Contents/MacOS`, installs
`AppIcon.icns`, and verifies its code signature. It uses an ad-hoc signature by default;
`ELY_CODESIGN_IDENTITY` selects an installed signing identity. Native distribution inputs place
`ely_app` and `ely_servo_sidecar` beside each other at the distribution root. The packaging scripts
verify those paths and executable permissions.

## Cloudflare Auth Configuration

`/api/auth/*` is served by Better Auth in the Cloudflare Worker. Local `wrangler dev`
uses `ELY_AUTH_BASE_URL` from `cloudflare/wrangler.toml`; deployed environments should
set the matching public Worker origin.

Use Wrangler secrets or an untracked `cloudflare/.dev.vars` file for the remaining auth
bindings:

```bash
wrangler secret put ELY_AUTH_SECRET
wrangler secret put ELY_AUTH_GOOGLE_CLIENT_ID
wrangler secret put ELY_AUTH_GOOGLE_CLIENT_SECRET
wrangler secret put ELY_AUTH_GITHUB_CLIENT_ID
wrangler secret put ELY_AUTH_GITHUB_CLIENT_SECRET
wrangler secret put ELY_AUTH_EMAIL_OTP_ENDPOINT
wrangler secret put ELY_AUTH_EMAIL_OTP_TOKEN
```

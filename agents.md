# agents.md — Engineering State of Truth

Read this before changing anything. PRD.md is the product spec (target);
this file records what is actually built and how to verify it. Update it
in the same commit as any change that makes it stale.

## Architecture (current, verified)

- `crates/ely_app` — GPUI shell: chrome, sidebar/spaces/tabs UI, internal
  `ely://` pages, sidecar broker, sync/auth UI state.
- `crates/ely_browser_core` — pure browser domain state (`BrowserCore`):
  tabs, spaces, profiles, bookmarks, history, settings, sync engine,
  local persistence. No IO except through callers.
- `crates/ely_servo_host` — Servo embedding, rendering contexts, the
  `ely_servo_sidecar` binary (one Servo process per profile; stdio JSON
  protocol v3 + macOS IOSurface Mach transport). See
  `docs/servo-embedding-architecture.md`.
- `crates/ely_sync_client` — worker API client, device identity/trust,
  E2E snapshot crypto, native-keychain bearer storage, sync-owner store.
- `crates/ely_domain`, `crates/ely_design_system` — types and tokens.
- `cloudflare/` — Workers API (Better Auth, D1, R2, KV), 232 contract
  tests via `npm test`.
- `third_party/gpui` — patched GPUI 0.2.2 (native surface, corner clip).

Servo pin: git rev in `Cargo.toml` `[workspace.dependencies]`; the whole
engine (55 crates) moves with that one rev. Keep
`docs/servo-embedding-architecture.md` pin reference in sync.

## Run

`scripts/run_dev.sh <url>` is the dev entrypoint: it builds the Servo sidecar,
wires `ELY_SERVO_SIDECAR`, picks the hardware (macOS) or software (Linux/Windows)
rendering context, and on macOS resolves a Metal-capable `DEVELOPER_DIR` (raw
`cargo run` under Command Line Tools fails at gpui's `xcrun metal` shader build —
see README Build Prerequisites). Screenshot probe: `scripts/verify_render.sh`.

## Verify (all must pass before commit; capture real exit codes)

```
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer  # full Xcode required (Metal)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo clippy -p ely_servo_host --features servo-engine,hardware-render --all-targets -- -D warnings
scripts/verify_prd_site_rendering.sh   # live-site e2e through real sidecars
scripts/audit_source_lines.sh          # every source file <= 500 lines
scripts/verify_render.sh               # real-window screenshot probe (release build)
(cd cloudflare && npm run check && npm test)
```

Workspace lints deny `unwrap/expect/panic/todo/dbg/unsafe` everywhere
including tests — return `Result` and use `?`.

## Real vs deferred (2026-07-10 audit; keep honest)

Real and verified:
- Rendering: Chrome-comparable on servo.org, Wikipedia, apple.com,
  github.com (screenshots; engine gaps = upstream Servo completeness,
  e.g. icon-font glyphs, WebGL-heavy pages).
- Local persistence: `local-state.json` per standard profile
  (`ely_browser_core/src/local_state.rs` + `shell/local_persistence.rs`);
  restore on launch, debounced save on mutation, save on quit; private
  profiles never persist; corrupt files quarantined loudly. Verified by
  restart e2e.
- Sync security chain: sync-owner binding, device trust/rebind/revoke,
  E2E snapshot crypto, bearer in native keychain, per-client rate limits
  on `/api/auth/*` (5/min for OTP email routes).
- Space accent: stored, synced, rendered (sidebar glyph), user-settable
  via `>space-accent #RRGGBB`; new spaces rotate a palette.
- Idle archive: policy sweeps every 30 min (`shell/timers.rs`) plus the
  manual Run Now; per-(space,profile) active-tab memory survives
  background-tab closes; shortcuts bind only the current platform's keys;
  sync-owner publish is first-claim-wins on every platform.

Deferred deliberately (do NOT fake; ship with their subsystem):
- Updates settings page — returns with a real updater.
- Diagnostics reporting toggle — returns with a real telemetry reporter
  (worker route `/api/telemetry/events` already exists and is tested).
- Advanced settings page — folded into owning sections; the decorative
  appearance accent row is gone (accent is Space-scoped).

Known structural debt (next big items, in order; full ranked audit with
file:line evidence lives in the 2026-07-10 bug-sweep report):
1. Sync merge model is snapshot-clobber: no tombstones (deletes can
   resurrect — closed tabs reopen everywhere), non-transactional apply
   (one bad record wedges sync at `state/sync_apply.rs`), remote-wins
   overwrite (back/forward stacks rebuilt empty at `state/sync.rs:302`,
   revoked site permissions re-granted, foreign records re-homed into the
   active profile at `state/sync.rs:395`), spaces converge by name not id,
   idle devices churn (byte-exact AlreadyCurrent + Vec-order serialization).
   Needs record-level merge + tombstones + Conflict Center (PRD §9). Do
   not "quick-fix"; fix before any multi-device testing.
2. Privacy: private-profile tabs are visible in the standard sidebar and
   Ctrl+Tab cycles into them (`state.rs` visible_tabs filters by space
   only; `tab_selection.rs`) — violates PRD §8.11.
3. Settings scalars are not persisted (search engine, new-tab
   destination, appearance, policies, the 9 sync toggles — a paused sync
   toggle silently re-enables on restart, privacy-relevant) and session
   fidelity is partial (stacks, splits, groups, archived tabs, downloads
   not yet in `local-state.json`; extend the local-state document).
4. Site permissions: only 5 of 16 features are enforced through Servo
   (`runtime_permissions.rs:153`); the other 11 rows are placebo toggles —
   trim the UI to enforced features until the engine covers them.
5. Engine/webview: redirect or pushState leaves tab state "loading"
   forever and pins stale pixels (`ely_servo_host/src/runtime_webview.rs`
   requested-vs-current URL reconciliation); persistent-profile sidecars
   are never reclaimed while the app runs; sidecar stderr is nulled;
   final URLs >32KiB cause a reload loop.
6. Downloads engine: pause/resume/cancel/retry are UI-only, progress
   never updates, checksum runs on the UI thread, open/reveal hardcode
   `/usr/bin/open`; `ely://auth/callback` exchange and save-page commands
   are unimplemented.
7. Smaller confirmed papercuts: several synced mutations never schedule
   an upload (splits, group toggles, deletions), trash_space leaks split
   layouts, reload of the current URL is a no-op (crashed tabs can't
   reload in place), mid-Vec tab inserts skip sort normalization, Esc
   doesn't close the command overlay, "Switch workspace" palette entry
   mislabels its action, Profiles page cannot create/delete profiles,
   vault rotation silently skips devices without wrapping keys,
   SyncStatus counters are hardcoded, second in-process Servo host panics
   (upstream OnceLock).
8. Integration tests litter the real data root with `profile_*` dirs
   (`~/Library/Application Support/com.elydora.ELY-Browser/profiles/`);
   tests should take an overridable data root.

Upstream watch: Servo's permission request lacks the requesting principal
and some DOM paths bypass the embedder broker — that upstream change is
the trigger to ungate clipboard/geolocation/notification/webrtc prefs.

## Conventions

- One problem per commit; full gate battery before each; push after.
- Root-cause fixes only; no fallback values that hide missing data.
- Red-green: reproduce with a failing test before fixing.
- Files stay under 500 lines (CI-enforced); split like
  `runtime.rs`/`runtime_paint.rs` or `state/local_visibility.rs`.
- Tracing targets: `ely::sync`, `ely::local_state` — add spans/fields on
  every new failure path so field issues are diagnosable.

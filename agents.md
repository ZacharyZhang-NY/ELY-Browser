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

Deferred deliberately (do NOT fake; ship with their subsystem):
- Updates settings page — returns with a real updater.
- Diagnostics reporting toggle — returns with a real telemetry reporter
  (worker route `/api/telemetry/events` already exists and is tested).
- Advanced settings page — folded into owning sections.

Known structural debt (next big items, in order):
1. Sync merge model is snapshot-clobber: no tombstones (deletes can
   resurrect), non-transactional apply, remote-wins. Needs record-level
   merge + tombstones + Conflict Center (PRD §9). Do not "quick-fix".
2. Session fidelity in local persistence: back/forward stacks, splits,
   tab groups, archived tabs, downloads, settings values are not yet in
   `local-state.json` (extend `LOCAL_STATE_REV`).
3. Download pause/resume/cancel are UI-only; `ely://auth/callback` code
   exchange and save-page commands are unimplemented.
4. Integration tests litter the real data root with `profile_*` dirs
   (`~/Library/Application Support/com.elydora.ELY-Browser/profiles/`);
   tests should take an overridable data root.

## Conventions

- One problem per commit; full gate battery before each; push after.
- Root-cause fixes only; no fallback values that hide missing data.
- Red-green: reproduce with a failing test before fixing.
- Files stay under 500 lines (CI-enforced); split like
  `runtime.rs`/`runtime_paint.rs` or `state/local_visibility.rs`.
- Tracing targets: `ely::sync`, `ely::local_state` — add spans/fields on
  every new failure path so field issues are diagnosable.

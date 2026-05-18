# ELY Browser

Native Rust + GPUI browser workspace for ELY Browser by Elydora.

The repository is organized around explicit product boundaries from `PRD.md`:
domain state, browser orchestration, design tokens, GPUI shell, and Servo host contracts.
Reference GPUI ecosystem repositories live under `references/` for local review and are excluded
from version control.

## Local Commands

```bash
cargo fmt --all --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo check -p ely_servo_host --features servo-engine --all-targets
cargo clippy -p ely_servo_host --features servo-engine --all-targets -- -D warnings
cargo test -p ely_servo_host --features servo-engine --test software_host
scripts/verify_prd_site_rendering.sh
scripts/verify_windows_app_manifest.sh
cargo run -p ely_app
```

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

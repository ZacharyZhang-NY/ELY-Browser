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
cargo test -p ely_servo_host --features servo-engine --test sidecar -- --test-threads=1
cargo test -p ely_app --features live-site-smoke --all-targets -- --test-threads=1
cargo run -p ely_app
```

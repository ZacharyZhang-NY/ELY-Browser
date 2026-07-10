#!/usr/bin/env bash
set -euo pipefail

echo "Verifying PRD live sites through the GPUI web surface adapter"
cargo build --locked -p ely_servo_host --features servo-engine --bin ely_servo_sidecar
ELY_SERVO_RENDERING_CONTEXT=software \
    cargo test --locked -p ely_app --features live-site-smoke --all-targets -- --test-threads=1 --nocapture

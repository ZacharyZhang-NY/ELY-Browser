#!/usr/bin/env bash
set -euo pipefail

echo "Verifying PRD live sites through the GPUI web surface adapter"
cargo test -p ely_app --features live-site-smoke --all-targets -- --test-threads=1 --nocapture

echo "Verifying PRD live sites through Servo sidecar RGBA snapshots"
cargo test -p ely_servo_host --features servo-engine --test sidecar -- --test-threads=1 --nocapture

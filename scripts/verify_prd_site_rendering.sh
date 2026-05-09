#!/usr/bin/env bash
set -euo pipefail

cargo test -p ely_app --features live-site-smoke --all-targets -- --test-threads=1
cargo test -p ely_servo_host --features servo-engine --test sidecar -- --test-threads=1

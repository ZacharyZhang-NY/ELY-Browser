#!/usr/bin/env bash
set -euo pipefail

echo "Verifying PRD live sites through the GPUI web surface adapter"
cargo test -p ely_app --features live-site-smoke --all-targets -- --test-threads=1 --nocapture

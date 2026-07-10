#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"
target_dir="${CARGO_TARGET_DIR:-${repo_root}/target}"
case "${target_dir}" in
    /*|[A-Za-z]:[\\/]*) ;;
    *) target_dir="${repo_root}/${target_dir}" ;;
esac
unset CARGO_BUILD_TARGET
sidecar_features="servo-engine"
rendering_context="software"
binary_suffix=""
if [[ "$(uname -s)" == "Darwin" ]]; then
    sidecar_features="servo-engine,hardware-render"
    rendering_context="hardware"
fi
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) binary_suffix=".exe" ;;
esac

cargo build --locked --manifest-path "${repo_root}/Cargo.toml" --target-dir "${target_dir}" \
    -p ely_servo_host \
    --features "${sidecar_features}" \
    --bin ely_servo_sidecar

export ELY_SERVO_SIDECAR="${target_dir}/debug/ely_servo_sidecar${binary_suffix}"
export ELY_SERVO_RENDERING_CONTEXT="${rendering_context}"
exec cargo run --locked --manifest-path "${repo_root}/Cargo.toml" \
    --target-dir "${target_dir}" \
    -p ely_app \
    -- "$@"

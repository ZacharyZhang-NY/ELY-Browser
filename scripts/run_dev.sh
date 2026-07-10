#!/usr/bin/env bash
set -euo pipefail

# gpui compiles its Metal shaders with `xcrun metal`, which ships only in the
# full Xcode Metal Toolchain — the standalone Command Line Tools do not provide
# it. Respect a developer directory that already resolves `metal`; otherwise
# fall back to a standard Xcode install; otherwise stop with the exact fix.
ely_ensure_metal_toolchain() {
    if xcrun --find metal >/dev/null 2>&1; then
        return 0
    fi
    local xcode_developer_dir="/Applications/Xcode.app/Contents/Developer"
    if [[ -d "${xcode_developer_dir}" ]] &&
        DEVELOPER_DIR="${xcode_developer_dir}" xcrun --find metal >/dev/null 2>&1; then
        export DEVELOPER_DIR="${xcode_developer_dir}"
        return 0
    fi
    cat >&2 <<'EOF'
error: the Metal shader compiler (`xcrun metal`) is unavailable.

gpui compiles Metal shaders at build time, which needs the full Xcode Metal
Toolchain. The Command Line Tools alone cannot build the macOS shell.

Fix (no sudo required):
  1. Install Xcode from the App Store.
  2. DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer \
       xcodebuild -downloadComponent MetalToolchain
  3. Re-run scripts/run_dev.sh
     (or point DEVELOPER_DIR at a non-standard Xcode location).
EOF
    return 1
}

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
    ely_ensure_metal_toolchain
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

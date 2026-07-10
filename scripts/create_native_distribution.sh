#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "${repo_root}"
target_dir="${CARGO_TARGET_DIR:-${repo_root}/target}"
case "${target_dir}" in
    /*|[A-Za-z]:[\\/]*) ;;
    *) target_dir="${repo_root}/${target_dir}" ;;
esac
source "${repo_root}/scripts/release_build_env.sh"
ely_configure_release_build_env "${repo_root}" "${target_dir}"
unset CARGO_BUILD_TARGET
distribution_dir="${repo_root}/target/distribution/ely-browser"
binary_suffix=""

case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) binary_suffix=".exe" ;;
esac

cargo build --release --locked --manifest-path "${repo_root}/Cargo.toml" --target-dir "${target_dir}" -p ely_app
sidecar_features="servo-engine"
if [[ "$(uname -s)" == "Darwin" ]]; then
    sidecar_features="servo-engine,hardware-render"
fi
cargo build --release --locked --manifest-path "${repo_root}/Cargo.toml" --target-dir "${target_dir}" -p ely_servo_host --features "${sidecar_features}" --bin ely_servo_sidecar

rm -rf "${distribution_dir}"
mkdir -p "${distribution_dir}"
cp "${target_dir}/release/ely_app${binary_suffix}" "${distribution_dir}/"
cp "${target_dir}/release/ely_servo_sidecar${binary_suffix}" "${distribution_dir}/"

case "$(uname -s)" in
    Darwin)
        cp "${repo_root}/packaging/macos/Info.plist" "${distribution_dir}/"
        cp "${repo_root}/generated-icons/macos/AppIcon.icns" "${distribution_dir}/"
        ;;
    Linux)
        cp "${repo_root}/packaging/linux/com.elydora.ely-browser.desktop" "${distribution_dir}/"
        ;;
    MINGW*|MSYS*|CYGWIN*)
        cp "${repo_root}/packaging/windows/ely_app.exe.manifest" "${distribution_dir}/"
        ;;
esac

if [[ -z "${binary_suffix}" ]]; then
    chmod 755 "${distribution_dir}/ely_app" "${distribution_dir}/ely_servo_sidecar"
fi

app_output="${distribution_dir}/ely_app${binary_suffix}"
sidecar_output="${distribution_dir}/ely_servo_sidecar${binary_suffix}"
"${repo_root}/scripts/verify_release_artifacts.sh" "${app_output}" "${sidecar_output}"
[[ -f "${app_output}" ]] || { echo "missing app executable at ${app_output}" >&2; exit 1; }
[[ -f "${sidecar_output}" ]] || { echo "missing sidecar executable at ${sidecar_output}" >&2; exit 1; }
if [[ -z "${binary_suffix}" ]]; then
    [[ -x "${app_output}" ]] || { echo "app is not executable: ${app_output}" >&2; exit 1; }
    [[ -x "${sidecar_output}" ]] || { echo "sidecar is not executable: ${sidecar_output}" >&2; exit 1; }
fi

echo "${distribution_dir}"

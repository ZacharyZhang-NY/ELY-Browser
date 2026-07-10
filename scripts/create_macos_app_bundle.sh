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
binary_path="${target_dir}/release/ely_app"
sidecar_path="${target_dir}/release/ely_servo_sidecar"
icon_path="${repo_root}/generated-icons/macos/AppIcon.icns"
bundle_root="${repo_root}/target/macos/ELY Browser.app"
contents_dir="${bundle_root}/Contents"
macos_dir="${contents_dir}/MacOS"
resources_dir="${contents_dir}/Resources"

cargo build --release --locked --manifest-path "${repo_root}/Cargo.toml" --target-dir "${target_dir}" -p ely_app
cargo build --release --locked --manifest-path "${repo_root}/Cargo.toml" --target-dir "${target_dir}" -p ely_servo_host --features servo-engine,hardware-render --bin ely_servo_sidecar

rm -rf "${bundle_root}"
mkdir -p "${macos_dir}" "${resources_dir}"
cp "${repo_root}/packaging/macos/Info.plist" "${contents_dir}/Info.plist"
cp "${icon_path}" "${resources_dir}/AppIcon.icns"
cp "${binary_path}" "${macos_dir}/ely_app"
cp "${sidecar_path}" "${macos_dir}/ely_servo_sidecar"
chmod 755 "${macos_dir}/ely_app"
chmod 755 "${macos_dir}/ely_servo_sidecar"
"${repo_root}/scripts/verify_release_artifacts.sh" \
    "${macos_dir}/ely_app" \
    "${macos_dir}/ely_servo_sidecar"
codesign_identity="${ELY_CODESIGN_IDENTITY:--}"
codesign --force --sign "${codesign_identity}" --timestamp=none "${macos_dir}/ely_app"
codesign --force --sign "${codesign_identity}" --timestamp=none "${macos_dir}/ely_servo_sidecar"
codesign --force --sign "${codesign_identity}" --timestamp=none "${bundle_root}"
codesign --verify --deep --strict "${bundle_root}"

[[ -x "${macos_dir}/ely_app" ]] || { echo "missing app executable in ${macos_dir}" >&2; exit 1; }
[[ -x "${macos_dir}/ely_servo_sidecar" ]] || { echo "missing sidecar executable in ${macos_dir}" >&2; exit 1; }
[[ -f "${resources_dir}/AppIcon.icns" ]] || { echo "missing app icon in ${resources_dir}" >&2; exit 1; }
[[ ! -e "${resources_dir}/ely_servo_sidecar" ]] || { echo "sidecar must be placed in ${macos_dir}" >&2; exit 1; }

echo "${bundle_root}"

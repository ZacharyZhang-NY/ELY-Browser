#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binary_path="${repo_root}/target/debug/ely_app"
sidecar_path="${repo_root}/target/debug/ely_servo_sidecar"
bundle_root="${repo_root}/target/macos/ELY Browser.app"
contents_dir="${bundle_root}/Contents"
macos_dir="${contents_dir}/MacOS"
resources_dir="${contents_dir}/Resources"

cargo build -p ely_app
cargo build -p ely_servo_host --features servo-engine,hardware-render --bin ely_servo_sidecar

rm -rf "${bundle_root}"
mkdir -p "${macos_dir}" "${resources_dir}"
cp "${repo_root}/packaging/macos/Info.plist" "${contents_dir}/Info.plist"
cp "${binary_path}" "${macos_dir}/ely_app"
cp "${sidecar_path}" "${macos_dir}/ely_servo_sidecar"
chmod 755 "${macos_dir}/ely_app"
chmod 755 "${macos_dir}/ely_servo_sidecar"

echo "${bundle_root}"

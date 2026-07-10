#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
target_dir="${CARGO_TARGET_DIR:-${repo_root}/target}"
case "${target_dir}" in
    /*|[A-Za-z]:[\\/]*) ;;
    *) target_dir="${repo_root}/${target_dir}" ;;
esac

source "${repo_root}/scripts/release_build_env.sh"

[[ "$#" -gt 0 ]] || { echo "usage: $0 <release-artifact>..." >&2; exit 2; }

scan_value() {
    local artifact="$1"
    local label="$2"
    local value="$3"
    [[ -n "${value}" ]] || return 0
    if LC_ALL=C grep -aFq -- "${value}" "${artifact}"; then
        echo "release artifact contains ${label}: ${artifact}" >&2
        return 1
    fi
}

scan_path() {
    local artifact="$1"
    local label="$2"
    local path="$3"
    local variant
    while IFS= read -r variant; do
        scan_value "${artifact}" "${label}" "${variant}"
    done < <(ely_path_variants "${path}")
}

cargo_home="${CARGO_HOME:-${HOME}/.cargo}"
rustup_home="${RUSTUP_HOME:-${HOME}/.rustup}"
rust_sysroot="$(rustc --print sysroot)"

for artifact in "$@"; do
    [[ -f "${artifact}" ]] || { echo "missing release artifact: ${artifact}" >&2; exit 1; }
    scan_path "${artifact}" "workspace path" "${repo_root}"
    scan_path "${artifact}" "Cargo home path" "${cargo_home}"
    scan_path "${artifact}" "Rustup home path" "${rustup_home}"
    scan_path "${artifact}" "Rust sysroot path" "${rust_sysroot}"
    scan_path "${artifact}" "target path" "${target_dir}"
    scan_value "${artifact}" "embedded Metal source marker" "-frecord-sources=yes"
    scan_value "${artifact}" "embedded Metal compiler command" "metal --driver-mode=metal"
done

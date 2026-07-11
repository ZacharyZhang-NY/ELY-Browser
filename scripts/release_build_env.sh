#!/usr/bin/env bash

ely_append_encoded_rustflag() {
    local flag="$1"
    local separator=$'\x1f'
    if [[ -n "${CARGO_ENCODED_RUSTFLAGS:-}" ]]; then
        CARGO_ENCODED_RUSTFLAGS+="${separator}${flag}"
    else
        CARGO_ENCODED_RUSTFLAGS="${flag}"
    fi
    export CARGO_ENCODED_RUSTFLAGS
}

ely_append_c_family_flag() {
    local flag="$1"
    CFLAGS="${CFLAGS:+${CFLAGS} }${flag}"
    CXXFLAGS="${CXXFLAGS:+${CXXFLAGS} }${flag}"
    export CFLAGS CXXFLAGS
}

ely_path_variants() {
    local path="$1"
    local canonical_path=""
    printf '%s\n' "${path}"
    if [[ -d "${path}" ]]; then
        canonical_path="$(cd "${path}" && pwd -P)"
        if [[ "${canonical_path}" != "${path}" ]]; then
            printf '%s\n' "${canonical_path}"
        fi
    fi
    if command -v cygpath >/dev/null 2>&1; then
        cygpath -m "${path}"
        cygpath -w "${path}"
    fi
}

ely_add_release_path_remap() {
    local path="$1"
    local replacement="$2"
    local variant
    while IFS= read -r variant; do
        [[ -n "${variant}" ]] || continue
        ely_append_encoded_rustflag "--remap-path-prefix=${variant}=${replacement}"
        case "$(uname -s)" in
            MINGW*|MSYS*|CYGWIN*)
                ely_append_c_family_flag "/pathmap:${variant}=${replacement}"
                ;;
            *)
                ely_append_c_family_flag "-ffile-prefix-map=${variant}=${replacement}"
                ;;
        esac
    done < <(ely_path_variants "${path}")
}

ely_set_release_build_revision() {
    local repo_root="$1"
    local revision
    if [[ -n "${ELY_BUILD_REVISION+x}" || ! -e "${repo_root}/.git" ]]; then
        return 0
    fi

    revision="$(git -C "${repo_root}" rev-parse --short=12 HEAD)"
    if [[ -n "$(git -C "${repo_root}" status --porcelain=v1 --untracked-files=normal)" ]]; then
        revision+="-dirty"
    fi
    ELY_BUILD_REVISION="${revision}"
    export ELY_BUILD_REVISION
}

ely_configure_release_build_env() {
    local repo_root="$1"
    local target_dir="$2"
    local cargo_home="${CARGO_HOME:-${HOME}/.cargo}"
    local rustup_home="${RUSTUP_HOME:-${HOME}/.rustup}"
    local rust_sysroot

    if [[ -n "${RUSTFLAGS:-}" ]]; then
        echo "RUSTFLAGS cannot be combined safely with release path remapping; use CARGO_ENCODED_RUSTFLAGS" >&2
        return 1
    fi

    ely_set_release_build_revision "${repo_root}"
    rust_sysroot="$(rustc --print sysroot)"
    ely_append_encoded_rustflag "--remap-path-scope=object"
    ely_add_release_path_remap "${cargo_home}" "[cargo]"
    ely_add_release_path_remap "${rustup_home}" "[rustup]"
    ely_add_release_path_remap "${rust_sysroot}" "[rust-sysroot]"
    ely_add_release_path_remap "${repo_root}" "."
    ely_add_release_path_remap "${target_dir}" "[target]"
}

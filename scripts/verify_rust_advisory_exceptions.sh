#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "${repo_root}"

rsa_count="$(
  awk '$0 == "name = \"rsa\"" { count += 1 } END { print count + 0 }' Cargo.lock
)"

if [[ "${rsa_count}" -ne 1 ]]; then
  echo "RSA advisory exception requires exactly one locked RSA package" >&2
  exit 1
fi

rsa_tree="$(
  cargo tree --locked -p ely_servo_host \
    --features servo-engine,hardware-render \
    -e features \
    -i rsa@0.10.0-rc.18
)"

if [[ "${rsa_tree%%$'\n'*}" != "rsa v0.10.0-rc.18 (${repo_root}/third_party/rsa)" ]]; then
  echo "RSA advisory exception requires the vendored 0.10.0-rc.18 package" >&2
  exit 1
fi

if [[ "${rsa_tree}" != *'rsa feature "private-key-operations-disabled"'* ]]; then
  echo "RSA advisory exception requires private-key-operations-disabled" >&2
  exit 1
fi

quick_xml_tree="$(
  cargo tree --workspace --all-features --target all --locked -i quick-xml@0.30.0 2>/dev/null
)"

if [[ -n "${quick_xml_tree}" ]]; then
  echo "quick-xml 0.30.0 advisory exception requires a lock-only dependency" >&2
  exit 1
fi

#!/usr/bin/env bash
set -euo pipefail

desktop_path="${1:-packaging/linux/com.elydora.ely-browser.desktop}"
expected_file_name="com.elydora.ely-browser.desktop"

if [[ ! -f "${desktop_path}" ]]; then
  echo "${desktop_path}: desktop entry file is missing"
  exit 1
fi

if [[ "$(basename "${desktop_path}")" != "${expected_file_name}" ]]; then
  echo "${desktop_path}: desktop entry filename expected '${expected_file_name}'"
  exit 1
fi

if ! grep -qx '\[Desktop Entry\]' "${desktop_path}"; then
  echo "${desktop_path}: [Desktop Entry] group is missing"
  exit 1
fi

desktop_value() {
  local key="$1"
  awk -F= -v key="${key}" '$1 == key { print substr($0, length(key) + 2); exit }' "${desktop_path}"
}

assert_value() {
  local key="$1"
  local expected="$2"
  local actual
  actual="$(desktop_value "${key}")"
  if [[ "${actual}" != "${expected}" ]]; then
    echo "${desktop_path}: ${key} expected '${expected}', got '${actual}'"
    exit 1
  fi
}

assert_value "Type" "Application"
assert_value "Name" "ELY Browser"
assert_value "GenericName" "Web Browser"
assert_value "Comment" "ELY Browser by Elydora"
assert_value "Exec" "ely_app %u"
assert_value "Icon" "com.elydora.ely-browser"
assert_value "Terminal" "false"
assert_value "Categories" "Network;WebBrowser;"
assert_value "MimeType" "x-scheme-handler/ely;"

echo "${desktop_path}: Linux desktop metadata ok"

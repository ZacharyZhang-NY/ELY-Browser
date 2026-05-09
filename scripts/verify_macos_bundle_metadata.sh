#!/usr/bin/env bash
set -euo pipefail

plist_path="${1:-packaging/macos/Info.plist}"

if [[ ! -f "${plist_path}" ]]; then
  echo "${plist_path}: plist file is missing"
  exit 1
fi

plutil -lint "${plist_path}" >/dev/null

plist_value() {
  /usr/libexec/PlistBuddy -c "Print :$1" "${plist_path}"
}

assert_value() {
  local key="$1"
  local expected="$2"
  local actual
  actual="$(plist_value "${key}")"
  if [[ "${actual}" != "${expected}" ]]; then
    echo "${plist_path}: ${key} expected '${expected}', got '${actual}'"
    exit 1
  fi
}

assert_value "CFBundleDisplayName" "ELY Browser"
assert_value "CFBundleName" "ELY Browser"
assert_value "CFBundleIdentifier" "com.elydora.ely-browser"
assert_value "CFBundleGetInfoString" "ELY Browser by Elydora"
assert_value "CFBundleExecutable" "ely_app"
assert_value "CFBundlePackageType" "APPL"
assert_value "LSApplicationCategoryType" "public.app-category.productivity"

url_name="$(/usr/libexec/PlistBuddy -c "Print :CFBundleURLTypes:0:CFBundleURLName" "${plist_path}")"
url_scheme="$(/usr/libexec/PlistBuddy -c "Print :CFBundleURLTypes:0:CFBundleURLSchemes:0" "${plist_path}")"

if [[ "${url_name}" != "com.elydora.ely-browser" ]]; then
  echo "${plist_path}: URL type name expected 'com.elydora.ely-browser', got '${url_name}'"
  exit 1
fi

if [[ "${url_scheme}" != "ely" ]]; then
  echo "${plist_path}: URL scheme expected 'ely', got '${url_scheme}'"
  exit 1
fi

echo "${plist_path}: macOS bundle metadata ok"

#!/usr/bin/env bash
set -euo pipefail

manifest_path="${1:-packaging/windows/ely_app.exe.manifest}"
expected_file_name="ely_app.exe.manifest"

if [[ ! -f "${manifest_path}" ]]; then
  echo "${manifest_path}: Windows app manifest is missing"
  exit 1
fi

if [[ "$(basename "${manifest_path}")" != "${expected_file_name}" ]]; then
  echo "${manifest_path}: Windows app manifest filename expected '${expected_file_name}'"
  exit 1
fi

xmllint --noout "${manifest_path}"

xml_value() {
  local xpath="$1"
  xmllint --xpath "string(${xpath})" "${manifest_path}"
}

assert_value() {
  local label="$1"
  local xpath="$2"
  local expected="$3"
  local actual
  actual="$(xml_value "${xpath}")"
  if [[ "${actual}" != "${expected}" ]]; then
    echo "${manifest_path}: ${label} expected '${expected}', got '${actual}'"
    exit 1
  fi
}

assert_value "manifestVersion" "/*[local-name()='assembly']/@manifestVersion" "1.0"
assert_value "assemblyIdentity type" "/*[local-name()='assembly']/*[local-name()='assemblyIdentity']/@type" "win32"
assert_value "assemblyIdentity name" "/*[local-name()='assembly']/*[local-name()='assemblyIdentity']/@name" "Elydora.ELYBrowser"
assert_value "assemblyIdentity version" "/*[local-name()='assembly']/*[local-name()='assemblyIdentity']/@version" "0.1.0.0"
assert_value "assemblyIdentity processorArchitecture" "/*[local-name()='assembly']/*[local-name()='assemblyIdentity']/@processorArchitecture" "amd64"
assert_value "description" "/*[local-name()='assembly']/*[local-name()='description']" "ELY Browser by Elydora"
assert_value "requestedExecutionLevel level" "//*[local-name()='requestedExecutionLevel']/@level" "asInvoker"
assert_value "requestedExecutionLevel uiAccess" "//*[local-name()='requestedExecutionLevel']/@uiAccess" "false"
assert_value "Windows 10/11 supportedOS" "//*[local-name()='supportedOS']/@Id" "{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"
assert_value "dpiAwareness" "//*[local-name()='dpiAwareness']" "PerMonitorV2, unaware"
assert_value "longPathAware" "//*[local-name()='longPathAware']" "true"
assert_value "activeCodePage" "//*[local-name()='activeCodePage']" "UTF-8"

echo "${manifest_path}: Windows app manifest metadata ok"

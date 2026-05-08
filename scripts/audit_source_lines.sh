#!/usr/bin/env bash
set -euo pipefail

max_lines=500
status=0

while IFS= read -r -d '' file; do
  lines="$(wc -l < "${file}" | tr -d ' ')"
  if [[ "${lines}" -gt "${max_lines}" ]]; then
    echo "${file}: ${lines} lines exceeds ${max_lines}"
    status=1
  fi
done < <(find crates cloudflare/src cloudflare/tests \( -name '*.rs' -o -name '*.ts' \) -print0)

exit "${status}"

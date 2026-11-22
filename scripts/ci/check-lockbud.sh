#!/bin/bash
set -euo pipefail
script_directory="$(cd "$(dirname "$0")" && pwd)"
log="${LOCKBUD_LOG_PATH:-lockbud-analysis.log}"
# Collect all crates rather than stopping at the first reviewed dependency report.
# Nonzero tool/compiler exits always fail; successful output is checked separately.
if cargo lockbud -k deadlock -b -l tokio_util -- --locked > "$log" 2>&1; then
  :
else
  status=$?
  cat "$log" >&2
  printf 'Lockbud did not complete successfully (exit %s).\n' "$status" >&2
  exit "$status"
fi
if ! python3 "$script_directory/check-lockbud-reports.py" "$log"; then
  cat "$log" >&2
  exit 1
fi

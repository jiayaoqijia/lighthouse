#!/bin/bash
set -euo pipefail

# Run lockbud to check for deadlocks and capture the output
if output=$(cargo lockbud -k deadlock -b -l tokio_util 2>&1); then
  :
else
  status=$?
  printf '%s\n' "$output" >&2
  printf 'Lockbud did not complete successfully (exit %s).\n' "$status" >&2
  exit "$status"
fi

# Check if lockbud returned any issues
if [[ "$output" == *'"bug_kind"'* ]]; then
  # Print the JSON payload
  echo "Lockbud detected issues:"
  echo "$output"

  # Exit with a non-zero status to indicate an error
  exit 1
else
  echo "No issues detected by Lockbud."
  exit 0
fi

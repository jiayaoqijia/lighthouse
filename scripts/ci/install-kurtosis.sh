#!/usr/bin/env bash
# Pin the official CLI artifact; the SDK apt index can return HTTP 503 without a usable package index.
set -euo pipefail
if [[ $# -ne 1 || $(uname -s) != Linux || $(uname -m) != x86_64 ]]; then
  echo "usage: install-kurtosis.sh DESTINATION_DIRECTORY (Linux x86_64)" >&2
  exit 2
fi
version=1.23.0
checksum=985bffafb6c76e6a6ed12c37a6b2d52b12fe0ccea659f93be4af1e9852b9bd4c
archive_name="kurtosis-cli_${version}_linux_amd64.tar.gz"
url="https://github.com/kurtosis-tech/kurtosis-cli-release-artifacts/releases/download/${version}/${archive_name}"
temporary=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/kurtosis-cli.XXXXXX")
trap 'rm -rf "$temporary"' EXIT
curl --fail --silent --show-error --location --retry 5 --retry-all-errors --connect-timeout 10 --max-time 180 "$url" -o "$temporary/$archive_name"
printf '%s  %s\n' "$checksum" "$temporary/$archive_name" | sha256sum --check --status
tar -xzf "$temporary/$archive_name" -C "$temporary"
mkdir -p "$1"
install -m 0755 "$temporary/kurtosis" "$1/kurtosis"
"$1/kurtosis" version

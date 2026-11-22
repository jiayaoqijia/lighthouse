#!/usr/bin/env bash
set -euo pipefail
subject="$(cd "$(dirname "$0")" && pwd)/check-lockbud.sh"
temporary="$(mktemp -d)"
trap 'rm -rf "$temporary"' EXIT
mkdir "$temporary/bin"
cat > "$temporary/bin/cargo" <<'CARGO'
#!/usr/bin/env bash
case "$LOCKBUD_TEST_CASE" in
  failed) echo 'compiler could not run'; exit 17 ;;
  finding) echo '{"bug_kind":"DoubleLock"}'; exit 0 ;;
  large) echo '{"bug_kind":"DoubleLock"}'; head -c 200000 /dev/zero | tr '\0' x; exit 0 ;;
  clean) echo 'analysis completed'; exit 0 ;;
esac
CARGO
chmod +x "$temporary/bin/cargo"
for scenario in failed finding large clean; do
  if PATH="$temporary/bin:$PATH" LOCKBUD_TEST_CASE="$scenario" bash "$subject" > "$temporary/$scenario.log" 2>&1; then
    result=0
  else
    result=$?
  fi
  if [[ "$scenario" == clean ]]; then
    [[ "$result" == 0 ]] || { cat "$temporary/$scenario.log"; exit 1; }
  else
    [[ "$result" != 0 ]] || { echo "Incorrect success for $scenario"; cat "$temporary/$scenario.log"; exit 1; }
  fi
done
echo 'Lockbud command failures, findings and successful analysis are distinguished.'

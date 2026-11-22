#!/usr/bin/env bash
set -euo pipefail
temporary="$(mktemp -d)"
trap 'rm -rf "$temporary"' EXIT
for scenario in clean double; do
  mkdir -p "$temporary/$scenario/src"
  printf '[package]\nname="lockbud_%s"\nversion="0.1.0"\nedition="2024"\n' "$scenario" > "$temporary/$scenario/Cargo.toml"
  if [[ "$scenario" == clean ]]; then
    printf '%s\n' 'fn main(){let lock=std::sync::Mutex::new(0);{let _first=lock.lock().unwrap();}let _second=lock.lock().unwrap();}' > "$temporary/$scenario/src/main.rs"
  else
    printf '%s\n' 'fn main(){let lock=std::sync::Mutex::new(0);let _first=lock.lock().unwrap();let _second=lock.lock().unwrap();}' > "$temporary/$scenario/src/main.rs"
  fi
  cargo generate-lockfile --manifest-path "$temporary/$scenario/Cargo.toml"
  if CARGO_TARGET_DIR="$temporary/target" cargo lockbud -k deadlock --exit-code-on-bug -- --locked --manifest-path "$temporary/$scenario/Cargo.toml" > "$temporary/$scenario.log" 2>&1; then
    result=0
  else
    result=$?
  fi
  if [[ "$scenario" == clean ]]; then
    [[ "$result" == 0 ]] || { cat "$temporary/$scenario.log"; exit 1; }
  else
    [[ "$result" != 0 ]] && grep -q '"bug_kind"' "$temporary/$scenario.log" || { cat "$temporary/$scenario.log"; exit 1; }
  fi
done
echo 'The actual analyzer compiles edition2024, accepts clean locking and rejects a double lock.'

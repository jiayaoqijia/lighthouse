#!/usr/bin/env python3
"""Reject unknown Lockbud findings and malformed output; retain the original log."""
import hashlib
import json
from pathlib import Path
import re
import sys


def canonical(value):
    if isinstance(value, str):
        value = re.sub(r"[^\s\"]*/tokio-1\.53\.2/", "tokio-1.53.2/", value)
        return re.sub(r"[^\s\"]*/lib/rustlib/src/rust/", "rust/", value)
    if isinstance(value, list):
        return [canonical(item) for item in value]
    if isinstance(value, dict):
        result = {key: canonical(item) for key, item in value.items()}
        if "callchains" in result:
            result["callchains"] = sorted(result["callchains"], key=lambda item: json.dumps(item, sort_keys=True))
        return result
    return value


def fingerprint(record):
    return hashlib.sha256(json.dumps(canonical(record), sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def check(log, policy):
    records = []
    decoder = json.JSONDecoder()
    for match in re.finditer(r"(?m)(?:^|\])\s*(\[)\s*$", log):
        try:
            payload, _ = decoder.raw_decode(log[match.start(1):])
        except ValueError:
            continue
        if isinstance(payload, list) and '"bug_kind"' in json.dumps(payload):
            records.extend(payload)
    # A parser/format change or truncated report must never silently look clean.
    markers = len(re.findall(r'"bug_kind"\s*:', log))
    if markers != len(records):
        raise ValueError(f"Unparsed finding output: {markers} markers, {len(records)} records")
    reviewed = {entry["sha256"] for entry in policy["findings"]}
    unknown = [fingerprint(record) for record in records if fingerprint(record) not in reviewed]
    if unknown:
        raise ValueError("Unreviewed Lockbud finding fingerprints: " + ", ".join(unknown))
    return len(records)


if __name__ == "__main__":
    policy_path = Path(__file__).with_name("lockbud-reviewed-findings.json")
    try:
        count = check(Path(sys.argv[1]).read_text(), json.loads(policy_path.read_text()))
    except (ValueError, KeyError, OSError, IndexError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
    print(f"Lockbud completed: {count} individually reviewed findings; no unreviewed findings.")

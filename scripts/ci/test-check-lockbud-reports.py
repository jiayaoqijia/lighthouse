#!/usr/bin/env python3
import copy
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("reports", ROOT / "check-lockbud-reports.py")
REPORTS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPORTS)
POLICY = json.loads((ROOT / "lockbud-reviewed-findings.json").read_text())


def log(records):
    return "[2026-11-22T05:35:45Z WARN  lockbud::callbacks] " + json.dumps(records, indent=2) + "\n"


class ReportsTest(unittest.TestCase):
    def test_exact_reviewed_reports(self):
        records = [item["record"] for item in POLICY["findings"]]
        self.assertEqual(REPORTS.check(log(records), POLICY), 12)

    def test_moved_registry_directory_is_only_normalization(self):
        record = copy.deepcopy(POLICY["findings"][0]["record"])
        for key in ("first_lock_span", "second_lock_span"):
            record["DoubleLock"]["diagnosis"][key] = "/new/cargo/registry/src/example/" + record["DoubleLock"]["diagnosis"][key]
        self.assertEqual(REPORTS.check(log([record]), POLICY), 1)

    def test_changed_span_type_or_callchain_fails(self):
        for key, value in (("first_lock_span", "new.rs:1:1"), ("second_lock_type", "NewMutex"), ("callchains", [[[]]])):
            record = copy.deepcopy(POLICY["findings"][0]["record"])
            record["DoubleLock"]["diagnosis"][key] = value
            with self.assertRaisesRegex(ValueError, "Unreviewed"):
                REPORTS.check(log([record]), POLICY)

    def test_unknown_double_lock_and_truncated_output_fail(self):
        for output in (log([{"DoubleLock": {"bug_kind": "DoubleLock"}}]), log([POLICY["findings"][0]["record"]])[:-20], '{"bug_kind": "DoubleLock"}'):
            with self.assertRaises(ValueError):
                REPORTS.check(output, POLICY)

    def test_clean_output(self):
        self.assertEqual(REPORTS.check("Compiling lighthouse\nFinished", POLICY), 0)


if __name__ == "__main__":
    unittest.main()

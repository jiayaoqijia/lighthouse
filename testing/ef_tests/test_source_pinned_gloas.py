#!/usr/bin/env python3
import io
import json
from pathlib import Path
import shutil
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import install_source_pinned_gloas as subject


class BundleTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.bundle = self.root / "bundle"
        shutil.copytree(subject.BUNDLE, self.bundle)
        self.output = self.root / "output"

    def repack(self, mutation):
        manifest_path = self.bundle / "manifest.json"
        manifest = json.loads(manifest_path.read_text())
        archive_path = self.bundle / manifest["vector_archive"]["name"]
        with tarfile.open(archive_path, "r:gz") as archive:
            files = {item.name: archive.extractfile(item).read() for item in archive.getmembers()}
        mutation(files)
        with tarfile.open(archive_path, "w:gz") as archive:
            for name, data in files.items():
                item = tarfile.TarInfo(name)
                item.size = len(data)
                archive.addfile(item, io.BytesIO(data))
        manifest["vector_archive"]["sha256"] = subject.digest(archive_path.read_bytes())
        manifest_path.write_text(json.dumps(manifest))

    def test_actual_bundle_has_both_presets_and_all_cases(self):
        subject.install(self.bundle, self.output)
        manifest = json.loads((self.bundle / "manifest.json").read_text())
        for preset, expected in manifest["case_counts"].items():
            self.assertEqual(len(list((self.output / "tests" / preset).rglob("meta.yaml"))), expected)

    def test_changed_source_or_vector_archive_rejects_before_changes(self):
        manifest = json.loads((self.bundle / "manifest.json").read_text())
        for entry in (manifest["source_archive"], manifest["vector_archive"]):
            path = self.bundle / entry["name"]
            original = path.read_bytes()
            path.write_bytes(original + b"corrupted")
            with self.assertRaisesRegex(ValueError, "Archive digest mismatch"):
                subject.install(self.bundle, self.output)
            self.assertFalse(self.output.exists())
            path.write_bytes(original)

    def test_changed_state_rejects(self):
        self.repack(lambda files: files.__setitem__(next(name for name in files if name.endswith("/post.ssz_snappy")), b"wrong state"))
        with self.assertRaisesRegex(ValueError, "Vector digest mismatch"):
            subject.install(self.bundle, self.output)
        self.assertFalse(self.output.exists())

    def test_missing_member_rejects(self):
        self.repack(lambda files: files.pop(next(iter(files))))
        with self.assertRaisesRegex(ValueError, "Missing vector members"):
            subject.install(self.bundle, self.output)
        self.assertFalse(self.output.exists())

    def test_unexpected_path_rejects(self):
        self.repack(lambda files: files.__setitem__("../outside", b"bad"))
        with self.assertRaisesRegex(ValueError, "Unexpected"):
            subject.install(self.bundle, self.output)
        self.assertFalse((self.root / "outside").exists())

    def test_second_preset_install_failure_restores_all_previous_trees(self):
        for preset in ("minimal", "mainnet"):
            for runner in ("fork", "transition"):
                previous = self.output / "tests" / preset / "gloas" / runner
                previous.mkdir(parents=True)
                (previous / "previous").write_text(preset)
        rename = Path.rename
        def fail_second(source, target):
            if "mainnet" in source.parts and ".gloas-stage-" in str(source) and str(target).startswith(str(self.output)):
                raise OSError("injected second-preset publication failure")
            return rename(source, target)
        with patch.object(Path, "rename", fail_second):
            with self.assertRaisesRegex(OSError, "injected"):
                subject.install(self.bundle, self.output)
        for preset in ("minimal", "mainnet"):
            for runner in ("fork", "transition"):
                previous = self.output / "tests" / preset / "gloas" / runner
                self.assertEqual((previous / "previous").read_text(), preset)
                self.assertEqual(len(list(previous.iterdir())), 1)


if __name__ == "__main__":
    unittest.main()

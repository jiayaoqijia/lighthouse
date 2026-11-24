#!/usr/bin/env python3
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch
import install_source_pinned_fcr as subject

class FcrBundleTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bundle = self.root / "bundle"
        shutil.copytree(subject.BUNDLE, self.bundle)
        self.output = self.root / "output"
    def test_complete_source_bundle_installs(self):
        subject.install(self.bundle, self.output)
        self.assertEqual(len(list(self.output.rglob("manifest.yaml"))),196)
    def test_corruption_rejects_before_publication(self):
        manifest = json.loads((self.bundle / "manifest.json").read_text())
        path = self.bundle / manifest["vector_archive"]["name"]
        path.write_bytes(path.read_bytes()+b"wrong")
        with self.assertRaisesRegex(ValueError,"Archive digest"):
            subject.install(self.bundle,self.output)
        self.assertFalse(self.output.exists())
    def test_missing_member_rejects_before_publication(self):
        manifest_path=self.bundle / "manifest.json"
        manifest=json.loads(manifest_path.read_text())
        manifest["files"]["tests/minimal/gloas/fast_confirmation/missing/pyspec_tests/case/manifest.yaml"]="missing"
        manifest_path.write_text(json.dumps(manifest))
        with self.assertRaisesRegex(ValueError,"Missing vector members"):
            subject.install(self.bundle,self.output)
        self.assertFalse(self.output.exists())
    def test_failed_replacement_restores_previous_suite(self):
        target=self.output / "tests/minimal/gloas/fast_confirmation"
        target.mkdir(parents=True)
        (target / "previous").write_text("retained")
        original=Path.rename
        def fail(source,dest):
            if source.name=="fast_confirmation" and ".fcr-stage-" in str(source):
                raise OSError("injected publication failure")
            return original(source,dest)
        with patch.object(Path,"rename",fail):
            with self.assertRaisesRegex(OSError,"injected"):
                subject.install(self.bundle,self.output)
        self.assertEqual((target / "previous").read_text(),"retained")
if __name__=="__main__":
    unittest.main()

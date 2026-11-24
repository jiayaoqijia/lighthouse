import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
import sys

sys.dont_write_bytecode = True

spec = importlib.util.spec_from_file_location('current_reference', Path(__file__).with_name('current_reference.py'))
current = importlib.util.module_from_spec(spec)
spec.loader.exec_module(current)


class CurrentReferenceAdmission(unittest.TestCase):
    def test_receipts_reject_missing_duplicate_waived_and_wrong_crypto(self):
        record = {'path': '/qualification/one', 'source_revision': 'a' * 40,
            'source_dirty': False, 'crypto': 'real', 'fork': 'gloas',
            'passed': True, 'error': None}
        expected = {Path(record['path']).resolve()}
        current.validate_results([record], expected, 'a' * 40, 'real', 'gloas')
        for records in ([], [record, record], [dict(record, passed=False)],
                [dict(record, error='SkippedBls')], [dict(record, crypto='fixture_fake')],
                [dict(record, source_dirty=True)], [dict(record, source_revision='b' * 40)]):
            with self.assertRaises(ValueError):
                current.validate_results(records, expected, 'a' * 40, 'real', 'gloas')

    def test_archive_rejects_corruption_missing_and_escaping_members(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for index, name in enumerate(('test', '../escape', '/absolute')):
                archive = root / f'{index}.tar.xz'
                with tarfile.open(archive, 'w:xz') as output:
                    member = tarfile.TarInfo(name)
                    member.size = 3
                    output.addfile(member, io.BytesIO(b'bad'))
                with self.assertRaises(ValueError):
                    current.extract(archive, root / f'out-{index}', {'test': '0' * 64})
            archive = root / 'empty.tar.xz'
            with tarfile.open(archive, 'w:xz'):
                pass
            with self.assertRaises(ValueError):
                current.extract(archive, root / 'empty', {'missing': '0' * 64})
            self.assertFalse((root.parent / 'escape').exists())


if __name__ == '__main__':
    unittest.main()

"""Regressions for source-notice selection; no package manager is modified."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('native_notices', Path(__file__).with_name('native-notices.py'))
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)


class NoticeSelection(unittest.TestCase):
    def test_document_names_cannot_attribute_unrelated_packages(self):
        with tempfile.TemporaryDirectory(prefix='luma-notices-test-') as folder:
            root = Path(folder)
            library = root / 'usr/lib/libexample.so.1'
            library.parent.mkdir(parents=True)
            library.write_bytes(b'\x7fELFexample')
            document = root / 'usr/share/licenses/qt/copyright'
            document.parent.mkdir(parents=True)
            document.write_text('Qt notices already retained separately', encoding='utf-8')
            names = native.bundled_native_names(root)
            self.assertEqual(names, {'libexample.so.1'})
            inventory = '\n'.join(['libexample:amd64: /usr/lib/libexample.so.1',
                                   'apache2: /usr/share/doc/apache2/copyright',
                                   'unrelated: /usr/share/doc/unrelated/copyright'])
            self.assertEqual(native.inventory_owners(inventory, names), {'libexample:amd64'})

    def test_executables_and_multi_owner_records_remain_covered(self):
        with tempfile.TemporaryDirectory(prefix='luma-notices-test-') as folder:
            root = Path(folder)
            binary = root / 'usr/libexec/helper'
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b'\x7fELFexecutable')
            names = native.bundled_native_names(root)
            inventory = 'runtime-a:amd64, runtime-b: /usr/libexec/helper\ndiversion by other: /usr/libexec/helper'
            self.assertEqual(native.inventory_owners(inventory, names), {'runtime-a:amd64', 'runtime-b'})

    def test_empty_or_non_native_stage_fails_instead_of_silently_dropping_notices(self):
        with tempfile.TemporaryDirectory(prefix='luma-notices-test-') as folder:
            root = Path(folder)
            (root / 'usr').mkdir()
            (root / 'usr/README').write_text('No native program', encoding='utf-8')
            with self.assertRaises(ValueError):
                native.bundled_native_names(root)


if __name__ == '__main__':
    unittest.main()

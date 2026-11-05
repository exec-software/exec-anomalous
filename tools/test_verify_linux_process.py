"""Regressions for the independent oracle's resource and interval checks."""
from pathlib import Path
import tempfile
import unittest

from verify_linux_process import check_disjoint, read_bounded


class linux_oracle_tests(unittest.TestCase):
    def test_empty_and_adjacent_ranges_do_not_alias(self):
        check_disjoint([(4096, 8192), (7000, 7000), (8192, 12288)])
        check_disjoint([(7000, 7000), (4096, 8192)])
        with self.assertRaises(ValueError):
            check_disjoint([(4096, 8192), (8191, 12288)])
        with self.assertRaises(ValueError):
            check_disjoint([(8192, 4096)])

    def test_source_read_obeys_explicit_budget(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'source'
            path.write_bytes(b'1234')
            self.assertEqual(read_bounded(path, 4), b'1234')
            with self.assertRaises(ValueError):
                read_bounded(path, 3)


if __name__ == '__main__':
    unittest.main()

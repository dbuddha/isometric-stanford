#!/usr/bin/env python3

import importlib.util
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("measure_prototype.py")
SPEC = importlib.util.spec_from_file_location("measure_prototype", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class PrototypeMeasurementTests(unittest.TestCase):
    def test_parses_linux_and_macos_peak_rss(self) -> None:
        self.assertEqual(
            MODULE.parse_peak_rss_bytes(
                "Maximum resident set size (kbytes): 12345\n", "Linux"
            ),
            12345 * 1024,
        )
        self.assertEqual(
            MODULE.parse_peak_rss_bytes(
                "  987654 maximum resident set size\n", "Darwin"
            ),
            987654,
        )

    def test_directory_hash_covers_paths_and_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "a").mkdir()
            (root / "a" / "tile").write_bytes(b"one")
            first = MODULE.directory_sha256(root)
            (root / "a" / "tile").write_bytes(b"two")
            self.assertNotEqual(first, MODULE.directory_sha256(root))

    def test_release_metrics_counts_only_maximum_level_tiles(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "release.json").write_text(
                """{
                  "style_id": "stanford_v1.candidate_c.1",
                  "dzi": {
                    "max_level": 2,
                    "width": 8,
                    "height": 4,
                    "tile_count": 3,
                    "encoded_bytes": 99,
                    "tile_set_sha256": "abc"
                  },
                  "tiles": [{"level": 1}, {"level": 2}, {"level": 2}]
                }"""
            )
            metrics = MODULE.release_metrics(root, 30.0)
            self.assertEqual(metrics["maximum_level_tile_count"], 2)
            self.assertEqual(metrics["maximum_level_tiles_per_minute"], 4.0)


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Tests for fail-closed prototype preview assembly."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("assemble_preview.py")
SPEC = importlib.util.spec_from_file_location("assemble_preview", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


class PreviewAssemblyTests(unittest.TestCase):
    def fixture(self, root: Path) -> tuple[Path, Path, Path]:
        viewer = root / "viewer"
        viewer.mkdir()
        (viewer / "index.html").write_text("preview")
        artifact = root / "artifact"
        tile_directory = artifact / "hero_files" / "0"
        tile_directory.mkdir(parents=True)
        descriptor = b"descriptor"
        tile = b"tile"
        (artifact / "hero.dzi").write_bytes(descriptor)
        (tile_directory / "0_0.webp").write_bytes(tile)
        tile_entry = {
            "level": 0,
            "column": 0,
            "row": 0,
            "width": 1,
            "height": 1,
            "canonical_sha256": "d" * 64,
            "webp_sha256": digest(tile),
            "encoded_bytes": len(tile),
        }
        release = {
            "schema": "isometric-release/v1",
            "status": "artifact-candidate",
            "qualified": False,
            "style_id": "stanford_v1.candidate_c.1",
            "style_sha256": MODULE.sha256_file(
                MODULE_PATH.parent.parent / "styles" / "stanford_v1" / "candidate_c.toml"
            ),
            "world_sha256": "a" * 64,
            "dzi": {
                "descriptor": "hero.dzi",
                "width": 1,
                "height": 1,
                "tile_size": 512,
                "overlap": 0,
                "format": "webp",
                "max_level": 0,
                "world_mm_per_half_step": 250,
                "tile_count": 1,
                "encoded_bytes": len(tile),
                "descriptor_sha256": digest(descriptor),
                "tile_set_sha256": MODULE.tile_set_sha256([tile_entry]),
                "canonical_directory": "canonical",
                "tile_directory": "hero_files",
            },
            "tiles": [tile_entry],
        }
        (artifact / "release.json").write_text(json.dumps(release))
        world = root / "world.manifest.json"
        world.write_text(
            json.dumps(
                {
                    "region_id": "stanford-hero-v1",
                    "world_sha256": "a" * 64,
                    "unknown_fraction_ppm": 5_202,
                }
            )
        )
        return viewer, artifact, world

    def test_assembles_only_verified_web_assets(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            viewer, artifact, world = self.fixture(root)
            output = root / "preview"
            preview_path = MODULE.assemble_preview(
                viewer, artifact, world, output, MODULE_PATH.parent.parent
            )
            preview = json.loads(preview_path.read_text())
            self.assertEqual(preview["status"], "unqualified-engineering-preview")
            self.assertFalse(preview["published_release"])
            self.assertEqual(preview["unknown_fraction_ppm"], 5_202)
            self.assertTrue((output / "art" / "hero_files" / "0" / "0_0.webp").is_file())
            self.assertFalse((output / "canonical").exists())

    def test_rejects_stale_world_and_corrupted_tile(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            viewer, artifact, world = self.fixture(root)
            manifest = json.loads(world.read_text())
            manifest["world_sha256"] = "f" * 64
            world.write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "current fused world"):
                MODULE.validate_preview_inputs(viewer, artifact, world)

            manifest["world_sha256"] = "a" * 64
            world.write_text(json.dumps(manifest))
            (artifact / "hero_files" / "0" / "0_0.webp").write_bytes(b"corrupt")
            with self.assertRaisesRegex(ValueError, "encoded byte count"):
                MODULE.validate_preview_inputs(viewer, artifact, world)

    def test_rejects_artwork_that_claims_qualification(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            viewer, artifact, world = self.fixture(root)
            release_path = artifact / "release.json"
            release = json.loads(release_path.read_text())
            release["qualified"] = True
            release_path.write_text(json.dumps(release))
            with self.assertRaisesRegex(ValueError, "explicit unqualified"):
                MODULE.validate_preview_inputs(viewer, artifact, world)

    def test_rejects_stale_style(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            viewer, artifact, world = self.fixture(root)
            release_path = artifact / "release.json"
            release = json.loads(release_path.read_text())
            release["style_sha256"] = "f" * 64
            release_path.write_text(json.dumps(release))
            with self.assertRaisesRegex(ValueError, "current Candidate C style"):
                MODULE.validate_preview_inputs(
                    viewer,
                    artifact,
                    world,
                    MODULE_PATH.parent.parent
                    / "styles"
                    / "stanford_v1"
                    / "candidate_c.toml",
                )

    def test_rejects_corrupted_tile_set_hash(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            viewer, artifact, world = self.fixture(root)
            release_path = artifact / "release.json"
            release = json.loads(release_path.read_text())
            release["dzi"]["tile_set_sha256"] = "f" * 64
            release_path.write_text(json.dumps(release))
            with self.assertRaisesRegex(ValueError, "tile-set hash"):
                MODULE.validate_preview_inputs(viewer, artifact, world)

    def test_rejects_viewer_distribution_with_stale_artwork(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            viewer, artifact, world = self.fixture(root)
            (viewer / "art").mkdir()
            with self.assertRaisesRegex(ValueError, "stale pre-staged"):
                MODULE.validate_preview_inputs(viewer, artifact, world)


if __name__ == "__main__":
    unittest.main()

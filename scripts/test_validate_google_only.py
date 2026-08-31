#!/usr/bin/env python3
"""Tests for the Google-only production-boundary validator."""

from __future__ import annotations

import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from validate_google_only import ROOT, validate_google_only  # noqa: E402


class GoogleOnlyPolicyTests(unittest.TestCase):
    def fixture(self) -> Path:
        fixture = Path(tempfile.mkdtemp(prefix="isometric-google-only-"))
        self.addCleanup(shutil.rmtree, fixture)
        shutil.copy(ROOT / "reference-policy.json", fixture)
        shutil.copy(ROOT / "source.lock.json", fixture)
        for crate in (
            "isometric-mask",
            "isometric-publish",
            "isometric-reference",
            "isometric-style",
        ):
            crate_path = fixture / "crates" / crate
            crate_path.mkdir(parents=True)
            (crate_path / "Cargo.toml").write_text(
                f'[package]\nname = "{crate}"\nversion = "0.1.0"\n\n[dependencies]\n'
            )
        return fixture

    def test_accepts_google_only_active_boundary(self) -> None:
        self.assertEqual(validate_google_only(self.fixture()), [])

    def test_rejects_active_non_google_dependency(self) -> None:
        fixture = self.fixture()
        manifest = fixture / "crates/isometric-reference/Cargo.toml"
        manifest.write_text(
            manifest.read_text()
            + 'isometric-world = { path = "../isometric-world", version = "0.1.0" }\n'
        )

        self.assertTrue(
            any("isometric-world" in error for error in validate_google_only(fixture))
        )

    def test_rejects_transitive_non_google_dependency(self) -> None:
        fixture = self.fixture()
        reference = fixture / "crates/isometric-reference/Cargo.toml"
        reference.write_text(
            reference.read_text()
            + 'bridge = { path = "../bridge", version = "0.1.0" }\n'
        )
        bridge = fixture / "crates" / "bridge"
        bridge.mkdir()
        (bridge / "Cargo.toml").write_text(
            '[package]\nname = "bridge"\nversion = "0.1.0"\n\n[dependencies]\n'
            'isometric-source = { path = "../isometric-source", version = "0.1.0" }\n'
        )

        self.assertTrue(
            any("isometric-source" in error for error in validate_google_only(fixture))
        )

    def test_rejects_aliased_non_google_dependency(self) -> None:
        fixture = self.fixture()
        reference = fixture / "crates/isometric-reference/Cargo.toml"
        reference.write_text(
            reference.read_text()
            + 'world-alias = { package = "isometric-world", path = "../world" }\n'
        )

        self.assertTrue(
            any("isometric-world" in error for error in validate_google_only(fixture))
        )

    def test_rejects_active_open_data_status_and_qwen_pixels(self) -> None:
        fixture = self.fixture()
        source_lock = json.loads((fixture / "source.lock.json").read_text())
        source_lock["status"] = "prototype-sources-locked"
        (fixture / "source.lock.json").write_text(json.dumps(source_lock))
        policy = json.loads((fixture / "reference-policy.json").read_text())
        policy["processing"]["qwen_final_pixels"] = True
        (fixture / "reference-policy.json").write_text(json.dumps(policy))

        errors = validate_google_only(fixture)
        self.assertTrue(any("historical comparison" in error for error in errors))
        self.assertTrue(any("Qwen" in error for error in errors))


if __name__ == "__main__":
    unittest.main()

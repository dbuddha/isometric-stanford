from __future__ import annotations

import pytest

from isometric_perception import BenchmarkPatch, Bounds, validate_patch


def fixture_patch(**overrides: object) -> BenchmarkPatch:
    values: dict[str, object] = {
        "patch_id": "synthetic-001",
        "bounds": Bounds(west=-122.18, east=-122.17, south=37.42, north=37.43),
        "classes": ("building", "transient_mask"),
        "source_record_ids": ("original-synthetic-fixture",),
        "license": "CC-BY-4.0",
        "sha256": "a" * 64,
    }
    values.update(overrides)
    return BenchmarkPatch(**values)  # type: ignore[arg-type]


def test_valid_patch_passes() -> None:
    validate_patch(fixture_patch())


def test_final_person_class_is_rejected() -> None:
    with pytest.raises(ValueError, match="unsupported benchmark classes"):
        validate_patch(fixture_patch(classes=("person",)))


def test_invalid_hash_is_rejected() -> None:
    with pytest.raises(ValueError, match="SHA-256"):
        validate_patch(fixture_patch(sha256="pending"))

"""Fail-closed benchmark metadata before any model dependency is admitted."""

from __future__ import annotations

from dataclasses import dataclass

ALLOWED_CLASSES = frozenset(
    {
        "terrain",
        "water",
        "road",
        "path",
        "athletic_surface",
        "parking",
        "building",
        "vegetation",
        "construction_review",
        "transient_mask",
        "unknown",
    }
)


@dataclass(frozen=True)
class Bounds:
    """Geographic bounds in WGS84 degrees."""

    west: float
    east: float
    south: float
    north: float

    def validate(self) -> None:
        """Reject inverted or out-of-range bounds."""
        if not (-180.0 <= self.west < self.east <= 180.0):
            raise ValueError("longitude bounds are invalid")
        if not (-90.0 <= self.south < self.north <= 90.0):
            raise ValueError("latitude bounds are invalid")


@dataclass(frozen=True)
class BenchmarkPatch:
    """Metadata for one original or redistributable benchmark patch."""

    patch_id: str
    bounds: Bounds
    classes: tuple[str, ...]
    source_record_ids: tuple[str, ...]
    license: str
    sha256: str


def validate_patch(patch: BenchmarkPatch) -> None:
    """Validate metadata without loading or interpreting imagery."""
    patch.bounds.validate()
    if not patch.patch_id or not patch.source_record_ids:
        raise ValueError("patch identity and source records are required")
    unknown_classes = set(patch.classes) - ALLOWED_CLASSES
    if unknown_classes:
        raise ValueError(f"unsupported benchmark classes: {sorted(unknown_classes)}")
    if len(patch.sha256) != 64 or any(
        character not in "0123456789abcdef" for character in patch.sha256
    ):
        raise ValueError("patch hash must be lowercase SHA-256")
    if not patch.license:
        raise ValueError("patch license is required")

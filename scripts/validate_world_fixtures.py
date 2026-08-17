#!/usr/bin/env python3
"""Validate portable world contract fixtures without third-party packages."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
VALID_PATH = ROOT / "fixtures/world/representative.json"
INVALID_ROOT = ROOT / "fixtures/world/invalid"
SOURCE_LOCK_PATH = ROOT / "source.lock.json"
CLASSES = {
    "terrain",
    "water",
    "road",
    "path",
    "athletic-surface",
    "parking",
    "building",
    "vegetation",
    "unknown",
}


class FixtureError(ValueError):
    """A stable fixture contract failure."""

    def __init__(self, code: str, detail: str) -> None:
        super().__init__(f"{code}: {detail}")
        self.code = code


def load(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise FixtureError("invalid_root", "fixture root must be an object")
    return value


def validate_ring(ring: Any) -> None:
    if not isinstance(ring, list) or len(ring) < 4:
        raise FixtureError("invalid_ring", "rings require at least four points")
    for point in ring:
        if (
            not isinstance(point, list)
            or len(point) != 3
            or any(not isinstance(coordinate, int) for coordinate in point)
        ):
            raise FixtureError(
                "invalid_point", "points must be three integer millimeters"
            )
    if ring[0] != ring[-1]:
        raise FixtureError("open_ring", "rings must be closed")
    area_twice = sum(
        ring[index][0] * ring[index + 1][1] - ring[index + 1][0] * ring[index][1]
        for index in range(len(ring) - 1)
    )
    if area_twice == 0:
        raise FixtureError("zero_area", "rings must have nonzero projected area")


def validate_geometry(geometry: Any) -> None:
    if not isinstance(geometry, dict):
        raise FixtureError("invalid_geometry", "geometry must be an object")
    geometry_type = geometry.get("type")
    if geometry_type == "polygon":
        rings = geometry.get("rings")
        if not isinstance(rings, list) or not rings:
            raise FixtureError("invalid_polygon", "polygons require rings")
        for ring in rings:
            validate_ring(ring)
    elif geometry_type == "multipolygon":
        polygons = geometry.get("polygons")
        if not isinstance(polygons, list) or not polygons:
            raise FixtureError("invalid_multipolygon", "multipolygons require polygons")
        for polygon in polygons:
            validate_geometry({"type": "polygon", "rings": polygon.get("rings")})
    else:
        raise FixtureError("invalid_geometry_type", "unsupported geometry type")


def validate_fixture(fixture: dict[str, Any]) -> None:
    if fixture.get("license") != "CC-BY-4.0":
        raise FixtureError("invalid_license", "fixture must declare CC-BY-4.0")
    if fixture.get("contains_source_pixels") is not False:
        raise FixtureError("source_pixels", "fixture must not contain source pixels")
    if fixture.get("contains_transients") is not False:
        raise FixtureError("transients", "fixture must not contain transients")

    sources = fixture.get("sources")
    if not isinstance(sources, list):
        raise FixtureError("invalid_sources", "sources must be a list")
    source_ids = {
        source.get("id")
        for source in sources
        if isinstance(source, dict)
        and isinstance(source.get("id"), str)
        and isinstance(source.get("license"), str)
        and isinstance(source.get("attribution"), str)
    }
    if len(source_ids) != len(sources):
        raise FixtureError(
            "invalid_source", "every source requires identity and rights"
        )

    features = fixture.get("features")
    if not isinstance(features, list) or not features:
        raise FixtureError("invalid_features", "features must be a nonempty list")
    identifiers: set[int] = set()
    for feature in features:
        if not isinstance(feature, dict):
            raise FixtureError("invalid_feature", "features must be objects")
        identifier = feature.get("id")
        if (
            not isinstance(identifier, int)
            or identifier <= 0
            or identifier in identifiers
        ):
            raise FixtureError(
                "invalid_id", "feature IDs must be unique positive integers"
            )
        identifiers.add(identifier)
        if feature.get("class") not in CLASSES:
            raise FixtureError("invalid_class", "feature class is unsupported")
        confidence = feature.get("confidence_bp")
        if not isinstance(confidence, int) or not 0 <= confidence <= 10000:
            raise FixtureError(
                "invalid_confidence", "confidence must use integer basis points"
            )
        references = feature.get("source_ids")
        if not isinstance(references, list) or not references:
            raise FixtureError("missing_source", "features require source references")
        if any(reference not in source_ids for reference in references):
            raise FixtureError(
                "unknown_source", "feature references an undeclared source"
            )
        validate_geometry(feature.get("geometry"))

    for feature in features:
        parent = feature.get("parent_id")
        if parent is not None and parent not in identifiers:
            raise FixtureError(
                "unknown_parent", "building part references an unknown parent"
            )


def validate_contract_suite() -> None:
    valid = load(VALID_PATH)
    if valid.get("schema") != "isometric-world-fixture/v1":
        raise FixtureError("invalid_schema", "representative fixture schema is wrong")
    validate_fixture(valid)

    source_lock = load(SOURCE_LOCK_PATH)
    approved_sources = {
        source["id"]: source
        for source in source_lock.get("sources", [])
        if isinstance(source, dict) and source.get("approved") is True
    }
    for source in valid["sources"]:
        locked = approved_sources.get(source["id"])
        if locked is None:
            raise FixtureError(
                "unlocked_source", f"{source['id']} is absent from source.lock.json"
            )
        if source["license"] != locked.get("license") or source[
            "attribution"
        ] != locked.get("attribution"):
            raise FixtureError(
                "rights_mismatch", f"{source['id']} rights differ from source.lock.json"
            )

    classes = {feature["class"] for feature in valid["features"]}
    geometry_types = {feature["geometry"]["type"] for feature in valid["features"]}
    has_hole = any(
        feature["geometry"]["type"] == "polygon"
        and len(feature["geometry"]["rings"]) > 1
        for feature in valid["features"]
    )
    has_part = any("parent_id" in feature for feature in valid["features"])
    if not {"building", "road", "vegetation", "unknown"}.issubset(classes):
        raise FixtureError("missing_coverage", "required semantic classes are absent")
    if "multipolygon" not in geometry_types or not has_hole or not has_part:
        raise FixtureError(
            "missing_coverage", "holes, multipolygons, or parts are absent"
        )

    failures = sorted(INVALID_ROOT.glob("*.json"))
    if not failures:
        raise FixtureError("missing_failures", "negative fixtures are required")
    for path in failures:
        fixture = load(path)
        expected = fixture.get("expected_error")
        try:
            validate_fixture(fixture)
        except FixtureError as error:
            if error.code != expected:
                raise FixtureError(
                    "wrong_failure",
                    f"{path.name} expected {expected}, received {error.code}",
                ) from error
        else:
            raise FixtureError("unexpected_pass", f"{path.name} must fail validation")


if __name__ == "__main__":
    validate_contract_suite()
    print("world fixture contracts passed")

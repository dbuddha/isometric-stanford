#!/usr/bin/env python3
"""Validate the bootstrap manifest chain without third-party dependencies."""

from __future__ import annotations

import json
import hashlib
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
EXPECTED_SCHEMAS = {
    "source.lock.json": "isometric-source-lock/v1",
    "perception.lock.json": "isometric-perception-lock/v1",
    "world.manifest.json": "isometric-world-manifest/v1",
    "style.lock.json": "isometric-style-lock/v1",
    "render.manifest.json": "isometric-render-manifest/v1",
    "release.json": "isometric-release/v1",
}


def load(name: str) -> dict[str, Any]:
    with (ROOT / name).open(encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"{name} must contain a JSON object")
    return value


def validate() -> None:
    manifests = {name: load(name) for name in EXPECTED_SCHEMAS}
    for name, schema in EXPECTED_SCHEMAS.items():
        if manifests[name].get("schema") != schema:
            raise ValueError(f"{name} must use schema {schema}")

    bounds = manifests["source.lock.json"].get("slice")
    expected = {
        "west": -122.1900,
        "east": -122.1580,
        "south": 37.4195,
        "north": 37.4375,
    }
    if not isinstance(bounds, dict) or any(bounds.get(key) != value for key, value in expected.items()):
        raise ValueError("source lock vertical-slice bounds do not match the accepted plan")

    source_lock = manifests["source.lock.json"]
    if source_lock.get("google_content_permitted") is not False:
        raise ValueError("Google content must remain disabled without an approved rights exception")

    style_lock = manifests["style.lock.json"]
    style_path = style_lock.get("style_path")
    if not isinstance(style_path, str):
        raise ValueError("style lock must identify its style path")
    style_bytes = (ROOT / style_path).read_bytes()
    style_hash = hashlib.sha256(style_bytes).hexdigest()
    if style_lock.get("style_sha256") != style_hash:
        raise ValueError("style lock hash does not match the style pack")

    release = manifests["release.json"]
    if release.get("qualified") is not False or release.get("status") != "not-published":
        raise ValueError("bootstrap release must remain unqualified and unpublished")


if __name__ == "__main__":
    validate()
    print("manifest chain passed")

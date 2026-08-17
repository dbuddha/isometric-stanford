#!/usr/bin/env python3
"""Validate the bootstrap manifest chain without third-party dependencies."""

from __future__ import annotations

import json
import hashlib
import re
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
        "west": -122.1722,
        "east": -122.1653,
        "south": 37.4245,
        "north": 37.4299,
        "guard_meters": 50,
    }
    if not isinstance(bounds, dict) or any(
        bounds.get(key) != value for key, value in expected.items()
    ):
        raise ValueError("source lock prototype bounds do not match the accepted plan")

    if manifests["source.lock.json"].get("region_id") != "stanford-hero-v1":
        raise ValueError("source lock must identify the accepted prototype region")

    epoch_policy = bounds.get("epoch_policy")
    if not isinstance(epoch_policy, str) or "2026-08-17" not in epoch_policy:
        raise ValueError("source lock must record the accepted prototype epoch")

    source_lock = manifests["source.lock.json"]
    if source_lock.get("google_content_permitted") is not False:
        raise ValueError(
            "Google content must remain disabled without an approved rights exception"
        )

    sources = source_lock.get("sources")
    if not isinstance(sources, list) or not sources:
        raise ValueError("source lock must contain approved prototype artifacts")
    source_ids = [source.get("id") for source in sources if isinstance(source, dict)]
    if len(source_ids) != len(sources) or source_ids != sorted(set(source_ids)):
        raise ValueError("source records must be uniquely sorted by id")
    for source in sources:
        required = (
            "kind",
            "role",
            "release",
            "source_date",
            "acquired_at",
            "license",
            "attribution",
            "metadata_url",
        )
        if any(
            not isinstance(source.get(field), str) or not source[field]
            for field in required
        ):
            raise ValueError(f"source {source['id']} has incomplete provenance")
        if (
            source.get("approved") is not True
            or source.get("raw_content_in_final_output") is not False
        ):
            raise ValueError(
                f"source {source['id']} is not approved or permits raw final content"
            )
        digest = source.get("sha256")
        if not isinstance(digest, str) or re.fullmatch(r"[0-9a-f]{64}", digest) is None:
            raise ValueError(f"source {source['id']} has an invalid SHA-256")
        if not isinstance(source.get("size_bytes"), int) or source["size_bytes"] <= 0:
            raise ValueError(f"source {source['id']} has an invalid byte length")
        acquisition = source.get("acquisition")
        if not isinstance(acquisition, dict) or acquisition.get("method") not in {
            "https",
            "local",
        }:
            raise ValueError(f"source {source['id']} has an invalid acquisition method")
        if acquisition["method"] == "https":
            url = acquisition.get("url")
            if not isinstance(url, str) or not url.startswith("https://"):
                raise ValueError(f"source {source['id']} must use HTTPS")
            if "google." in url or "googleapis." in url:
                raise ValueError(
                    f"source {source['id']} attempts prohibited Google retrieval"
                )
        else:
            local_path = acquisition.get("path")
            if (
                not isinstance(local_path, str)
                or Path(local_path).is_absolute()
                or ".." in Path(local_path).parts
            ):
                raise ValueError(f"source {source['id']} has an unsafe local path")
            artifact = ROOT / local_path
            if not artifact.is_file():
                raise ValueError(f"source {source['id']} local artifact is missing")
            if artifact.stat().st_size != source["size_bytes"]:
                raise ValueError(
                    f"source {source['id']} local byte length does not match"
                )
            if hashlib.sha256(artifact.read_bytes()).hexdigest() != digest:
                raise ValueError(f"source {source['id']} local SHA-256 does not match")

    style_lock = manifests["style.lock.json"]
    style_path = style_lock.get("style_path")
    if not isinstance(style_path, str):
        raise ValueError("style lock must identify its style path")
    style_bytes = (ROOT / style_path).read_bytes()
    style_hash = hashlib.sha256(style_bytes).hexdigest()
    if style_lock.get("style_sha256") != style_hash:
        raise ValueError("style lock hash does not match the style pack")

    release = manifests["release.json"]
    if (
        release.get("qualified") is not False
        or release.get("status") != "not-published"
    ):
        raise ValueError("bootstrap release must remain unqualified and unpublished")


if __name__ == "__main__":
    validate()
    print("manifest chain passed")

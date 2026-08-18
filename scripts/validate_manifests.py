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
    if source_lock.get("google_content_permitted") is not True:
        raise ValueError("Google reference capture must retain its owner-approved state")

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
            etag = acquisition.get("etag")
            if etag is not None and (
                not isinstance(etag, str)
                or len(etag) < 3
                or not etag.startswith('"')
                or not etag.endswith('"')
                or any(ord(character) < 0x21 or ord(character) > 0x7E for character in etag)
            ):
                raise ValueError(f"source {source['id']} has an invalid HTTPS entity tag")
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

    perception = manifests["perception.lock.json"]
    if (
        perception.get("status") != "compiled-prototype"
        or perception.get("models") != []
    ):
        raise ValueError("perception lock must identify the model-free prototype compiler")
    runtime = perception.get("runtime")
    if (
        not isinstance(runtime, dict)
        or runtime.get("compiler") != "rust-naip-lidar-consensus-v1"
        or runtime.get("lidar_chunk_points") != 250_000
    ):
        raise ValueError("perception runtime contract is invalid")
    perception_artifacts = perception.get("artifacts")
    if not isinstance(perception_artifacts, list) or len(perception_artifacts) != 1:
        raise ValueError("perception lock must contain one frozen prototype artifact")
    perception_record = perception_artifacts[0]
    perception_path = perception_record.get("path")
    if (
        not isinstance(perception_path, str)
        or Path(perception_path).is_absolute()
        or ".." in Path(perception_path).parts
    ):
        raise ValueError("perception artifact path is unsafe")
    perception_bytes = (ROOT / perception_path).read_bytes()
    perception_hash = hashlib.sha256(perception_bytes).hexdigest()
    if (
        perception_record.get("sha256") != perception_hash
        or perception_record.get("contains_source_pixels") is not False
        or perception_record.get("contains_transients") is not False
        or perception_record.get("qualified") is not False
    ):
        raise ValueError("perception artifact lock is invalid")
    evidence = json.loads(perception_bytes)
    expected_perception_hashes = {
        source["id"]: source["sha256"]
        for source in sources
        if source["kind"] in {"imagery", "lidar"}
    }
    if (
        evidence.get("schema") != "isometric-perception-evidence/v1"
        or evidence.get("region_id") != "stanford-hero-v1"
        or evidence.get("status") != "compiled-prototype-evidence"
        or evidence.get("compiler") != "rust-naip-lidar-consensus-v1"
        or evidence.get("contains_source_pixels") is not False
        or evidence.get("contains_transients") is not False
        or evidence.get("source_sha256") != expected_perception_hashes
        or evidence.get("evidence_cell_count") != 372
        or evidence.get("vector_masked_cell_count") != 589
        or len(evidence.get("cells", [])) != 372
    ):
        raise ValueError("frozen perception evidence is incomplete or unprovenanced")
    if sum(cell.get("class") == "unknown" for cell in evidence["cells"]) > 19:
        raise ValueError("perception evidence exceeds the two-percent unknown budget")

    world = manifests["world.manifest.json"]
    if (
        world.get("status") != "prototype-semantic-world"
        or world.get("region_id") != "stanford-hero-v1"
        or world.get("semantic_version") != "0.3.0"
    ):
        raise ValueError("world manifest must describe the compiled prototype vector world")
    if not isinstance(world.get("object_count"), int) or world["object_count"] <= 0:
        raise ValueError("world manifest must contain accepted objects")
    if not isinstance(world.get("partition_count"), int) or world["partition_count"] <= 0:
        raise ValueError("world manifest must contain spatial partitions")
    unknown_fraction = world.get("unknown_fraction_ppm")
    if not isinstance(unknown_fraction, int) or not 0 <= unknown_fraction < 20_000:
        raise ValueError("world unknown coverage must be an integer fraction in ppm")
    if world.get("landmarks") != ["Hoover Tower", "Main Quad", "Memorial Church"]:
        raise ValueError("world manifest lacks the required prototype landmark evidence")
    world_hash = world.get("world_sha256")
    if not isinstance(world_hash, str) or re.fullmatch(r"[0-9a-f]{64}", world_hash) is None:
        raise ValueError("world artifact hash is invalid")
    source_hashes = world.get("source_sha256")
    expected_source_hashes = {source["id"]: source["sha256"] for source in sources}
    if source_hashes != expected_source_hashes:
        raise ValueError("world source hashes do not match the approved source lock")
    deferred = world.get("deferred_source_ids")
    if deferred != []:
        raise ValueError("world manifest does not explicitly account for deferred sources")
    if world.get("perception_sha256") != perception_hash:
        raise ValueError("world manifest does not pin the frozen perception artifact")

    style_lock = manifests["style.lock.json"]
    style_path = style_lock.get("style_path")
    if not isinstance(style_path, str):
        raise ValueError("style lock must identify its style path")
    style_bytes = (ROOT / style_path).read_bytes()
    style_hash = hashlib.sha256(style_bytes).hexdigest()
    if style_lock.get("style_sha256") != style_hash:
        raise ValueError("style lock hash does not match the style pack")

    render = manifests["render.manifest.json"]
    outputs = render.get("outputs")
    if (
        render.get("status") != "hero-semantic-preview"
        or not isinstance(outputs, list)
        or len(outputs) != 1
    ):
        raise ValueError("render manifest must describe exactly one hero semantic preview")
    output = outputs[0]
    if (
        output.get("id") != "hero-semantic-preview"
        or output.get("format") != "ppm-p6"
        or output.get("width") != 1954
        or output.get("height") != 880
        or output.get("palette_only") is not True
        or output.get("contains_source_pixels") is not False
        or output.get("contains_transients") is not False
        or output.get("qualified") is not False
    ):
        raise ValueError("hero semantic preview metadata violates the render contract")
    render_hash = output.get("sha256")
    if not isinstance(render_hash, str) or re.fullmatch(r"[0-9a-f]{64}", render_hash) is None:
        raise ValueError("hero semantic preview SHA-256 is invalid")

    release = manifests["release.json"]
    if (
        release.get("qualified") is not False
        or release.get("status") != "not-published"
    ):
        raise ValueError("release must remain unqualified and unpublished")


if __name__ == "__main__":
    validate()
    print("manifest chain passed")

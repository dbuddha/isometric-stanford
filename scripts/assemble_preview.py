#!/usr/bin/env python3
"""Assemble a portable, explicitly unqualified prototype preview bundle."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

EXPECTED_STYLE_ID = "stanford_v1.candidate_c.1"
EXPECTED_TILE_SIZE = 512
EXPECTED_FORMAT = "webp"


def sha256_file(path: Path) -> str:
    """Return the lowercase SHA-256 digest of one file."""
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_json(path: Path) -> dict[str, Any]:
    """Read one JSON object or fail with a useful error."""
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return value


def tile_set_sha256(entries: list[dict[str, Any]]) -> str:
    """Match the canonical Rust digest over sorted WebP paths and hashes."""
    digest = hashlib.sha256()
    for entry in entries:
        path = f"{entry.get('level')}/{entry.get('column')}_{entry.get('row')}.webp"
        digest.update(path.encode("utf-8"))
        digest.update(b"\0")
        digest.update(str(entry.get("webp_sha256")).encode("utf-8"))
        digest.update(b"\n")
    return digest.hexdigest()


def validate_preview_inputs(
    viewer_dist: Path,
    artifact: Path,
    world_manifest_path: Path,
    style_definition_path: Path | None = None,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Validate that the browser and DZI represent the current fused world."""
    if not (viewer_dist / "index.html").is_file():
        raise ValueError("viewer distribution lacks index.html")
    if (viewer_dist / "art").exists():
        raise ValueError("viewer distribution contains stale pre-staged artwork")
    release_path = artifact / "release.json"
    descriptor_path = artifact / "hero.dzi"
    tile_root = artifact / "hero_files"
    if not release_path.is_file() or not descriptor_path.is_file() or not tile_root.is_dir():
        raise ValueError("DZI artifact is incomplete")

    release = read_json(release_path)
    world = read_json(world_manifest_path)
    dzi = release.get("dzi")
    tiles = release.get("tiles")
    if not isinstance(dzi, dict) or not isinstance(tiles, list):
        raise ValueError("release manifest lacks DZI metadata or tiles")
    if (
        release.get("schema") != "isometric-release/v1"
        or release.get("status") != "artifact-candidate"
        or release.get("qualified") is not False
        or release.get("style_id") != EXPECTED_STYLE_ID
    ):
        raise ValueError("preview must be the explicit unqualified Candidate C artifact")
    if release.get("world_sha256") != world.get("world_sha256"):
        raise ValueError("preview artifact does not represent the current fused world")
    if style_definition_path is not None and (
        not style_definition_path.is_file()
        or release.get("style_sha256") != sha256_file(style_definition_path)
    ):
        raise ValueError("preview artifact does not represent the current Candidate C style")
    if (
        dzi.get("descriptor") != "hero.dzi"
        or dzi.get("tile_directory") != "hero_files"
        or dzi.get("tile_size") != EXPECTED_TILE_SIZE
        or dzi.get("overlap") != 0
        or dzi.get("format") != EXPECTED_FORMAT
        or not isinstance(dzi.get("width"), int)
        or not isinstance(dzi.get("height"), int)
        or dzi["width"] <= 0
        or dzi["height"] <= 0
    ):
        raise ValueError("preview DZI contract is invalid")
    if sha256_file(descriptor_path) != dzi.get("descriptor_sha256"):
        raise ValueError("preview descriptor hash does not match its manifest")

    webp_files = sorted(tile_root.rglob("*.webp"))
    if len(webp_files) != dzi.get("tile_count") or len(tiles) != dzi.get("tile_count"):
        raise ValueError("preview tile count is incomplete")
    try:
        sorted_tiles = sorted(
            tiles,
            key=lambda entry: (entry["level"], entry["row"], entry["column"]),
        )
    except (KeyError, TypeError):
        raise ValueError("preview tile entry is malformed") from None
    if sorted_tiles != tiles or tile_set_sha256(sorted_tiles) != dzi.get("tile_set_sha256"):
        raise ValueError("preview tile-set hash does not match its manifest")
    encoded_bytes = sum(path.stat().st_size for path in webp_files)
    if encoded_bytes != dzi.get("encoded_bytes"):
        raise ValueError("preview encoded byte count does not match its manifest")
    for entry in tiles:
        if not isinstance(entry, dict):
            raise ValueError("preview tile entry is malformed")
        tile_path = tile_root / str(entry.get("level")) / (
            f"{entry.get('column')}_{entry.get('row')}.webp"
        )
        if (
            not tile_path.is_file()
            or tile_path.stat().st_size != entry.get("encoded_bytes")
            or sha256_file(tile_path) != entry.get("webp_sha256")
        ):
            raise ValueError(f"preview tile does not match its manifest: {tile_path}")
    return release, world


def git_commit(repository: Path) -> str:
    """Return the exact source commit used for the preview."""
    return subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def assemble_preview(
    viewer_dist: Path,
    artifact: Path,
    world_manifest_path: Path,
    output: Path,
    repository: Path,
) -> Path:
    """Atomically assemble browser files and verified web-only DZI assets."""
    release, world = validate_preview_inputs(
        viewer_dist,
        artifact,
        world_manifest_path,
        repository / "styles" / "stanford_v1" / "candidate_c.toml",
    )
    if output.exists():
        raise ValueError(f"preview output already exists: {output}")
    staging = output.with_name(f".{output.name}.staging")
    if staging.exists():
        raise ValueError(f"preview staging path already exists: {staging}")
    output.parent.mkdir(parents=True, exist_ok=True)

    try:
        shutil.copytree(viewer_dist, staging)
        art = staging / "art"
        art.mkdir()
        shutil.copy2(artifact / "hero.dzi", art / "hero.dzi")
        shutil.copy2(artifact / "release.json", art / "release.json")
        shutil.copytree(artifact / "hero_files", art / "hero_files")
        preview = {
            "schema": "isometric-preview/v1",
            "status": "unqualified-engineering-preview",
            "published_release": False,
            "commit": git_commit(repository),
            "region_id": world.get("region_id"),
            "world_sha256": release["world_sha256"],
            "style_id": release["style_id"],
            "style_sha256": release["style_sha256"],
            "unknown_fraction_ppm": world.get("unknown_fraction_ppm"),
            "dzi": release["dzi"],
        }
        (staging / "preview.json").write_text(
            json.dumps(preview, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        staging.rename(output)
    except Exception:
        if staging.exists():
            shutil.rmtree(staging)
        raise
    return output / "preview.json"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--viewer-dist", type=Path, required=True)
    parser.add_argument("--dzi-artifact", type=Path, required=True)
    parser.add_argument("--world-manifest", type=Path, default=Path("world.manifest.json"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    repository = Path(__file__).resolve().parent.parent
    try:
        preview = assemble_preview(
            args.viewer_dist.resolve(),
            args.dzi_artifact.resolve(),
            args.world_manifest.resolve(),
            args.output.resolve(),
            repository,
        )
    except (OSError, ValueError, json.JSONDecodeError, subprocess.SubprocessError) as error:
        print(f"preview assembly failed: {error}", file=sys.stderr)
        return 1
    print(preview)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

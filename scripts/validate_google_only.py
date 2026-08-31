#!/usr/bin/env python3
"""Fail closed when the active reference pipeline gains non-Google geodata."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
POLICY_PATH = ROOT / "reference-policy.json"
SOURCE_LOCK_PATH = ROOT / "source.lock.json"
FORBIDDEN_LOCAL_DEPENDENCIES = {
    "isometric-source",
    "isometric-perception",
    "isometric-world",
}
ACTIVE_CRATES = {
    "isometric-mask",
    "isometric-reference",
    "isometric-style",
    "isometric-stylize",
}
FUTURE_CRATES = {"isometric-stylize"}


def dependency_names(manifest: str) -> set[str]:
    """Extract dependency keys from ordinary and target-specific TOML tables."""

    dependencies: set[str] = set()
    in_dependencies = False
    detailed_dependency: str | None = None
    for raw_line in manifest.splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if line.startswith("[") and line.endswith("]"):
            table = line[1:-1].strip()
            dependency_table = re.fullmatch(
                r"(?:target\..+\.)?(?:build-|dev-)?dependencies", table
            )
            detailed_table = re.fullmatch(
                r"(?:target\..+\.)?(?:build-|dev-)?dependencies\.([A-Za-z0-9_-]+)",
                table,
            )
            in_dependencies = dependency_table is not None
            detailed_dependency = (
                detailed_table.group(1) if detailed_table is not None else None
            )
            if detailed_dependency is not None:
                dependencies.add(detailed_dependency)
            continue
        if in_dependencies and "=" in line:
            name = line.split("=", 1)[0].strip().strip('"\'')
            if name:
                dependencies.add(name)
            package = re.search(r"\bpackage\s*=\s*['\"]([^'\"]+)['\"]", line)
            if package is not None:
                dependencies.add(package.group(1))
        elif detailed_dependency is not None and line.startswith("package"):
            package = re.fullmatch(r"package\s*=\s*['\"]([^'\"]+)['\"]", line)
            if package is not None:
                dependencies.add(package.group(1))
    return dependencies


def local_dependency_graph(root: Path) -> tuple[dict[str, set[str]], list[str]]:
    """Read local crate edges without resolving or executing Cargo metadata."""

    graph: dict[str, set[str]] = {}
    errors: list[str] = []
    crates_path = root / "crates"
    if not crates_path.is_dir():
        return graph, ["crates directory is missing"]

    for manifest_path in sorted(crates_path.glob("*/Cargo.toml")):
        crate = manifest_path.parent.name
        try:
            manifest = manifest_path.read_text()
        except OSError as error:
            errors.append(f"crate manifest cannot be read: {error}")
            continue
        graph[crate] = dependency_names(manifest)
    return graph, errors


def reachable_dependencies(root: str, graph: dict[str, set[str]]) -> set[str]:
    """Return all local dependency names reachable from ``root``."""

    reachable: set[str] = set()
    pending = list(graph.get(root, set()))
    while pending:
        dependency = pending.pop()
        if dependency in reachable:
            continue
        reachable.add(dependency)
        pending.extend(graph.get(dependency, set()))
    return reachable


def validate_google_only(root: Path = ROOT) -> list[str]:
    """Return every production-boundary violation under ``root``."""

    errors: list[str] = []
    try:
        policy = json.loads((root / POLICY_PATH.name).read_text())
    except (OSError, json.JSONDecodeError) as error:
        return [f"reference policy cannot be read: {error}"]

    expected = {
        "schema": "isometric-reference-policy/v1",
        "geographic_source": "google-photorealistic-3d-tiles",
        "other_geographic_sources": "prohibited",
    }
    for key, value in expected.items():
        if policy.get(key) != value:
            errors.append(f"reference policy {key} must be {value}")

    processing = policy.get("processing", {})
    if processing.get("qwen_final_pixels") is not False:
        errors.append("reference policy must prohibit Qwen final pixels")
    if not all(
        processing.get(key) is True
        for key in (
            "open_source_libraries",
            "pretrained_cv_weights",
            "original_non_geographic_art_assets",
        )
    ):
        errors.append("reference policy must record the accepted processing boundary")

    authorization = policy.get("authorization", {})
    if authorization.get("internal_processing") != "owner-asserted":
        errors.append("reference policy must record owner-asserted internal processing")
    if (
        authorization.get("public_release")
        != "blocked-pending-recorded-publication-permission"
    ):
        errors.append("reference policy must keep public release permission-gated")

    try:
        source_lock = json.loads((root / SOURCE_LOCK_PATH.name).read_text())
    except (OSError, json.JSONDecodeError) as error:
        errors.append(f"legacy source lock cannot be read: {error}")
    else:
        if source_lock.get("status") != "legacy-procedural-baseline-only":
            errors.append("open-data source lock must be historical comparison only")

    graph, graph_errors = local_dependency_graph(root)
    errors.extend(graph_errors)
    for crate in sorted(ACTIVE_CRATES):
        if crate not in graph:
            if crate in FUTURE_CRATES:
                continue
            errors.append(f"active crate manifest is missing: crates/{crate}/Cargo.toml")
            continue
        forbidden = sorted(
            reachable_dependencies(crate, graph) & FORBIDDEN_LOCAL_DEPENDENCIES
        )
        if forbidden:
            errors.append(
                f"{crate} reaches prohibited geographic source crates: "
                f"{', '.join(forbidden)}"
            )

    return errors


def main() -> int:
    errors = validate_google_only()
    if errors:
        for error in errors:
            print(f"google-only policy error: {error}")
        return 1
    print("google-only production boundary passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

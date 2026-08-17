#!/usr/bin/env python3
"""Measure repeatable Candidate C publication without publishing a release."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

MAX_PEAK_RSS_BYTES = 512 * 1024 * 1024
MAX_PUBLISH_SECONDS = 20 * 60
MIN_MAX_LEVEL_TILES_PER_MINUTE = 100.0
EXPECTED_STYLE_ID = "stanford_v1.candidate_c.1"


def parse_peak_rss_bytes(stderr: str, system: str) -> int:
    """Parse maximum RSS emitted by BSD or GNU time."""
    if system == "Darwin":
        match = re.search(r"^\s*(\d+)\s+maximum resident set size\s*$", stderr, re.MULTILINE)
        multiplier = 1
    else:
        match = re.search(
            r"^\s*Maximum resident set size \(kbytes\):\s*(\d+)\s*$",
            stderr,
            re.MULTILINE,
        )
        multiplier = 1024
    if match is None:
        raise RuntimeError("could not parse maximum resident set size from time output")
    return int(match.group(1)) * multiplier


def directory_sha256(root: Path) -> str:
    """Hash sorted paths and bytes so directory equality has one stable digest."""
    digest = hashlib.sha256()
    for path in sorted(candidate for candidate in root.rglob("*") if candidate.is_file()):
        relative = path.relative_to(root).as_posix().encode()
        digest.update(relative)
        digest.update(b"\0")
        with path.open("rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(chunk)
        digest.update(b"\n")
    return digest.hexdigest()


def measured_run(command: list[str], cwd: Path) -> dict[str, Any]:
    """Run one command under the platform time implementation."""
    system = platform.system()
    if system == "Darwin":
        timed_command = ["/usr/bin/time", "-l", *command]
    elif system == "Linux":
        timed_command = ["/usr/bin/time", "-v", *command]
    else:
        raise RuntimeError(f"unsupported measurement platform: {system}")

    started = time.perf_counter()
    completed = subprocess.run(
        timed_command,
        cwd=cwd,
        check=False,
        capture_output=True,
        text=True,
    )
    duration_seconds = time.perf_counter() - started
    if completed.returncode != 0:
        raise RuntimeError(
            f"command failed ({completed.returncode}): {' '.join(command)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return {
        "command": command,
        "duration_seconds": round(duration_seconds, 6),
        "peak_rss_bytes": parse_peak_rss_bytes(completed.stderr, system),
        "stdout": completed.stdout.strip(),
    }


def release_metrics(path: Path, duration_seconds: float) -> dict[str, Any]:
    manifest = json.loads((path / "release.json").read_text())
    maximum_level = manifest["dzi"]["max_level"]
    maximum_level_tiles = sum(
        1 for tile in manifest["tiles"] if tile["level"] == maximum_level
    )
    throughput = maximum_level_tiles / duration_seconds * 60
    return {
        "style_id": manifest["style_id"],
        "width": manifest["dzi"]["width"],
        "height": manifest["dzi"]["height"],
        "tile_count": manifest["dzi"]["tile_count"],
        "maximum_level_tile_count": maximum_level_tiles,
        "encoded_bytes": manifest["dzi"]["encoded_bytes"],
        "tile_set_sha256": manifest["dzi"]["tile_set_sha256"],
        "maximum_level_tiles_per_minute": round(throughput, 3),
    }


def git_commit(cwd: Path) -> str:
    return subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=cwd,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    repository = Path(__file__).resolve().parent.parent
    binary = args.binary.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)

    world = measured_run(
        [str(binary), "world", "compile", "artifacts/world"], repository
    )
    publications = []
    directory_hashes = []
    for index in range(1, 4):
        destination = output / f"run-{index}"
        measurement = measured_run(
            [str(binary), "publish", "dzi", str(destination), "candidate-c"],
            repository,
        )
        subprocess.run(
            [str(binary), "validate", "release", str(destination)],
            cwd=repository,
            check=True,
        )
        metrics = release_metrics(destination, measurement["duration_seconds"])
        measurement["release"] = metrics
        publications.append(measurement)
        directory_hashes.append(directory_sha256(destination))

    checks = {
        "three_runs_byte_identical": len(set(directory_hashes)) == 1,
        "candidate_c_identity": all(
            run["release"]["style_id"] == EXPECTED_STYLE_ID for run in publications
        ),
        "peak_rss_at_most_512_mib": all(
            run["peak_rss_bytes"] <= MAX_PEAK_RSS_BYTES for run in publications
        ),
        "publication_at_most_20_minutes": all(
            run["duration_seconds"] <= MAX_PUBLISH_SECONDS for run in publications
        ),
        "throughput_at_least_100_max_level_tiles_per_minute": all(
            run["release"]["maximum_level_tiles_per_minute"]
            >= MIN_MAX_LEVEL_TILES_PER_MINUTE
            for run in publications
        ),
    }
    report = {
        "schema": "isometric-prototype-performance/v1",
        "commit": git_commit(repository),
        "machine": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
            "logical_cpu_count": os.cpu_count(),
            "python": platform.python_version(),
        },
        "world_compile": world,
        "publications": publications,
        "directory_sha256": directory_hashes,
        "checks": checks,
        "passed": all(checks.values()),
    }
    report_path = output / "prototype-performance.json"
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(report_path)
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())

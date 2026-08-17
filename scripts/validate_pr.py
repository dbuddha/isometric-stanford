#!/usr/bin/env python3
"""Validate the pull request contract from a GitHub event payload."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any

REQUIRED_SECTIONS = (
    "## Context",
    "## Evidence",
    "## Risk and scope",
    "## Test plan",
)
TITLE_PATTERN = re.compile(
    r"^(build|chore|ci|docs|feat|fix|perf|refactor|revert|style|test)"
    r"(?:\([a-z0-9-]+\))?!?: .+"
)
ISSUE_PATTERN = re.compile(
    r"(?im)\b(?:close[sd]?|fix(?:e[sd])?|ref(?:s)?|relates to)\s+#\d+\b"
)


def validate_pull_request(pull_request: dict[str, Any]) -> list[str]:
    """Return every pull request contract violation."""
    errors: list[str] = []
    title = pull_request.get("title") or ""
    body = pull_request.get("body") or ""
    labels = [label.get("name", "") for label in pull_request.get("labels", [])]
    release_labels = [label for label in labels if label.startswith("release:")]

    if not TITLE_PATTERN.fullmatch(title):
        errors.append("title must use the repository Conventional Commit form")
    if len(release_labels) != 1:
        errors.append(
            "pull request must carry exactly one release:* label; "
            f"found {len(release_labels)}"
        )
    for section in REQUIRED_SECTIONS:
        if section not in body:
            errors.append(f"pull request body is missing required section: {section}")
    if not ISSUE_PATTERN.search(body):
        errors.append("pull request body must link an issue with Closes, Fixes, Refs, or Relates to")

    return errors


def load_pull_request(event_path: Path) -> dict[str, Any]:
    event = json.loads(event_path.read_text(encoding="utf-8"))
    pull_request = event.get("pull_request")
    if not isinstance(pull_request, dict):
        raise ValueError("event payload does not contain a pull_request object")
    return pull_request


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: validate_pr.py GITHUB_EVENT_PATH", file=sys.stderr)
        return 2

    try:
        pull_request = load_pull_request(Path(argv[1]))
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"unable to validate pull request event: {error}", file=sys.stderr)
        return 2

    errors = validate_pull_request(pull_request)
    if errors:
        for error in errors:
            print(f"pull request policy: {error}", file=sys.stderr)
        return 1

    print("pull request contract passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))

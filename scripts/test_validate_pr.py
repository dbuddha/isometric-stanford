#!/usr/bin/env python3
"""Tests for the pull request contract validator."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from validate_pr import validate_pull_request  # noqa: E402


def pull_request() -> dict[str, object]:
    return {
        "title": "ci(policy): enforce pull request contracts",
        "body": (
            "## Context\n\nRefs #4.\n\n"
            "## Evidence\n\nPolicy tests pass.\n\n"
            "## Risk and scope\n\nCI policy only.\n\n"
            "## Test plan\n\nRun scripts/check.sh.\n"
        ),
        "labels": [{"name": "release:none"}, {"name": "area:infra"}],
    }


class PullRequestContractTests(unittest.TestCase):
    def test_accepts_complete_contract(self) -> None:
        self.assertEqual(validate_pull_request(pull_request()), [])

    def test_rejects_missing_and_duplicate_release_labels(self) -> None:
        missing = pull_request()
        missing["labels"] = [{"name": "area:infra"}]
        duplicate = pull_request()
        duplicate["labels"] = [{"name": "release:none"}, {"name": "release:fix"}]

        self.assertIn("found 0", " ".join(validate_pull_request(missing)))
        self.assertIn("found 2", " ".join(validate_pull_request(duplicate)))

    def test_rejects_missing_sections_and_issue_link(self) -> None:
        invalid = pull_request()
        invalid["body"] = "## Context\n\nNo task linkage."

        errors = validate_pull_request(invalid)
        self.assertTrue(any("## Evidence" in error for error in errors))
        self.assertTrue(any("link an issue" in error for error in errors))

    def test_rejects_nonconventional_title(self) -> None:
        invalid = pull_request()
        invalid["title"] = "Improve CI"

        self.assertTrue(any("Conventional Commit" in error for error in validate_pull_request(invalid)))


if __name__ == "__main__":
    unittest.main()

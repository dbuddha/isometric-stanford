#!/usr/bin/env python3
"""Generate small deterministic OpenCV differential fixtures."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import cv2
import numpy as np


def flattened(values: np.ndarray) -> list[int]:
    """Return row-major built-in integers for portable JSON."""
    return [int(value) for value in values.reshape(-1)]


def build_fixture() -> dict[str, object]:
    """Build the complete versioned oracle fixture."""
    depth = np.array(
        [[1000 if x < 3 else 2000 for x in range(7)] for _ in range(5)],
        dtype=np.float64,
    )
    gradient_x = cv2.Scharr(depth, cv2.CV_64F, 1, 0, borderType=cv2.BORDER_CONSTANT)
    gradient_y = cv2.Scharr(depth, cv2.CV_64F, 0, 1, borderType=cv2.BORDER_CONSTANT)
    magnitude = np.abs(gradient_x).astype(np.int64) + np.abs(gradient_y).astype(
        np.int64
    )
    magnitude[[0, -1], :] = 0
    magnitude[:, [0, -1]] = 0

    binary = np.array(
        [
            [0, 0, 0, 0, 0, 0, 0],
            [0, 0, 255, 0, 0, 0, 0],
            [0, 0, 255, 0, 0, 0, 0],
            [0, 0, 255, 255, 255, 0, 0],
            [0, 0, 0, 0, 255, 0, 0],
            [0, 0, 0, 0, 255, 0, 0],
            [0, 0, 0, 0, 0, 0, 0],
        ],
        dtype=np.uint8,
    )
    kernel = np.ones((3, 3), dtype=np.uint8)
    dilated = cv2.dilate(binary, kernel, borderType=cv2.BORDER_CONSTANT, borderValue=0)
    eroded = cv2.erode(binary, kernel, borderType=cv2.BORDER_CONSTANT, borderValue=0)
    opened = cv2.morphologyEx(
        binary,
        cv2.MORPH_OPEN,
        kernel,
        borderType=cv2.BORDER_CONSTANT,
        borderValue=0,
    )
    closed = cv2.morphologyEx(
        binary,
        cv2.MORPH_CLOSE,
        kernel,
        borderType=cv2.BORDER_CONSTANT,
        borderValue=0,
    )
    component_count, component_labels = cv2.connectedComponents(
        binary,
        connectivity=8,
        ltype=cv2.CV_32S,
    )

    return {
        "schema": "isometric-opencv-geometry-oracle/v1",
        "opencv_version": cv2.__version__,
        "numpy_version": np.__version__,
        "scharr": {
            "width": 7,
            "height": 5,
            "depth": flattened(depth),
            "l1_magnitude": flattened(magnitude),
        },
        "morphology": {
            "width": 7,
            "height": 7,
            "input": flattened(binary),
            "radius": 1,
            "dilate": flattened(dilated),
            "erode": flattened(eroded),
            "open": flattened(opened),
            "close": flattened(closed),
        },
        "components": {
            "connectivity": 8,
            "foreground_components": int(component_count - 1),
            "labels": flattened(component_labels),
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    arguments = parser.parse_args()
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(
        json.dumps(build_fixture(), indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()

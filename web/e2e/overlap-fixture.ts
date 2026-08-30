import { createHash } from "node:crypto";
import type { Page } from "@playwright/test";

import { encodePng } from "../../capture/src/node/png.js";

const REPORT_ROUTE = "**/fixture/overlap/overlap-report.json";
const IMAGE_ROUTE = (path: string) => `**/fixture/overlap/comparison/${path}`;
const CORE_WIDTH = 128;
const CORE_HEIGHT = 64;
const OVERLAP_WIDTH = 32;
const OVERLAP_HEIGHT = 80;

interface FixtureOptions {
  corruptHash?: boolean;
  partialSourcePass?: boolean;
}

function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function scene(width: number, height: number, heatmap = false): Buffer {
  const pixels = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const offset = (y * width + x) * 4;
      const tower = x > width * 0.42 && x < width * 0.57 && y > 4 && y < height * 0.72;
      const roof = y > height * 0.66 && x > width * 0.24 && x < width * 0.76;
      const seam = Math.abs(x - width / 2) < 1;
      const color: [number, number, number] = heatmap
        ? seam
          ? [20, 2, 4]
          : [0, 0, 0]
        : tower
          ? [206, 153, 104]
          : roof
            ? [176, 74, 48]
            : (x + y) % 11 === 0
              ? [58, 93, 60]
              : [80, 116, 69];
      pixels.set([...color, 255], offset);
    }
  }
  return encodePng(pixels, width, height, 6);
}

function difference(pixelsCompared: number) {
  return {
    exact_mismatch_pixels: 0,
    maximum_absolute_difference: 0,
    mean_absolute_difference_microunits: 0,
    passed: true,
    pixels_above_tolerance: 0,
    pixels_above_tolerance_ppm: 0,
    pixels_compared: pixelsCompared,
  };
}

function layer(maximum: number, ppm: number) {
  return {
    gate: {
      maximum_absolute_difference: maximum,
      maximum_above_tolerance_ppm: ppm,
    },
    joined_vs_monolithic_core: difference(CORE_WIDTH * CORE_HEIGHT),
    joined_boundary_vs_monolithic: difference(64 * CORE_HEIGHT),
    left_vs_monolithic_overlap: difference(OVERLAP_WIDTH * OVERLAP_HEIGHT),
    left_vs_right_overlap: difference(OVERLAP_WIDTH * OVERLAP_HEIGHT),
    left_vs_right_seam_corridor: difference(64 * CORE_HEIGHT),
    right_vs_monolithic_overlap: difference(OVERLAP_WIDTH * OVERLAP_HEIGHT),
  };
}

function fixture(options: FixtureOptions) {
  const artifacts = new Map<string, Buffer>([
    ["joined-core.png", scene(CORE_WIDTH, CORE_HEIGHT)],
    ["monolithic-core.png", scene(CORE_WIDTH, CORE_HEIGHT)],
    ["core-oracle-heatmap.png", scene(CORE_WIDTH, CORE_HEIGHT, true)],
    ["overlap-left.png", scene(OVERLAP_WIDTH, OVERLAP_HEIGHT)],
    ["overlap-right.png", scene(OVERLAP_WIDTH, OVERLAP_HEIGHT)],
    ["overlap-monolithic.png", scene(OVERLAP_WIDTH, OVERLAP_HEIGHT)],
    ["overlap-heatmap.png", scene(OVERLAP_WIDTH, OVERLAP_HEIGHT, true)],
  ]);
  const images = Object.fromEntries(
    Array.from(artifacts, ([path, bytes]) => {
      const core = path.includes("core");
      return [
        path.slice(0, -4),
        {
          byte_length: bytes.length,
          height_px: core ? CORE_HEIGHT : OVERLAP_HEIGHT,
          path,
          sha256:
            options.corruptHash && path === "joined-core.png" ? "0".repeat(64) : sha256(bytes),
          width_px: core ? CORE_WIDTH : OVERLAP_WIDTH,
        },
      ];
    }),
  );
  const report = {
    schema: "isometric-reference-overlap-experiment/v1",
    cameraRegistration: {
      fixedWorldMatrix: true,
      horizontalPixelsPerMeter: 4,
      maximumScaleErrorPixelsPerMeter: 0,
      projectionCenterX: { left: 0.8, monolithic: 0, right: -0.8 },
      verticalPixelsPerMeter: 4,
      worldMatrixSha256: "a".repeat(64),
    },
    candidates: ["monolithic", "left", "right"].map((candidateId, index) => ({
      candidateId,
      evidence: {
        coreCoverageBasisPoints: 10_000,
        elapsedMs: 1_250 + index * 100,
        visibleTiles: 71 + index,
      },
    })),
    comparison: {
      schema: "isometric-reference-overlap-report/v1",
      boundary_structural_edge_pixels: 42,
      failure_classifications: options.partialSourcePass
        ? ["monolithic-oracle-level-of-detail", "shadow-phase"]
        : [],
      gates: {
        all_relations: !options.partialSourcePass,
        lighting_seam: !options.partialSourcePass,
        source: {
          independent_seam: true,
          monolithic_seam: !options.partialSourcePass,
        },
      },
      images,
      layers: {
        color: layer(24, 5_000),
        coverage: layer(0, 0),
        "fixed-shadow": layer(16, 1_000),
        "linear-depth": layer(250, 100),
        "view-normal": layer(2, 100),
        whitebox: layer(3, 250),
      },
      passed: !options.partialSourcePass,
      registration_search: {
        baseline_above_tolerance_ppm: 0,
        best_above_tolerance_ppm: 0,
        best_dx_px: 0,
        best_dy_px: 0,
        observations_compared: 10_240,
        radius_px: 2,
      },
    },
    grid: {
      cameraScreenRightBearingMillidegrees: 60_000,
      checkedSavedPixelCenters: 2_097_152,
      maximumPixelCenterErrorPixels: 0.0004,
    },
    network: {
      attempted: 282,
      billableRootRequests: 1,
      blocked: 0,
      completed: 278,
      failed: 4,
      formats: { glb: 256, json: 26 },
      requestLimit: 450,
      responseBodyBytes: 16_137_216,
      statuses: { "200": 278 },
    },
    runtime: {
      ingestWorkerMaxRssBytes: 89_128_960,
      nodeMaxRssBytes: 82_837_504,
      processTree: { peak: { treeBytes: 918_552_576 } },
      workerEnvelopeBytes: 67_108_864,
    },
  };
  return { artifacts, report };
}

export async function installOverlapFixture(page: Page, options: FixtureOptions = {}) {
  const { artifacts, report } = fixture(options);
  const reportBytes = Buffer.from(`${JSON.stringify(report)}\n`);
  await page.route(REPORT_ROUTE, (route) =>
    route.fulfill({
      body: reportBytes,
      contentType: "application/json",
      headers: { "content-length": String(reportBytes.length) },
      status: 200,
    }),
  );
  for (const [path, artifact] of artifacts) {
    await page.route(IMAGE_ROUTE(path), (route) =>
      route.fulfill({
        body: artifact,
        contentType: "image/png",
        headers: { "content-length": String(artifact.length) },
        status: 200,
      }),
    );
  }
}

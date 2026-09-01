import { createHash } from "node:crypto";
import type { Page } from "@playwright/test";
import { encodePng } from "../../capture/src/node/png.js";

const REPORT_ROUTE = "**/fixture/repair/repair-review.json";
const IMAGE_ROUTE = (path: string) => `**/fixture/repair/${path}`;
const IDS = [
  "source-logical",
  "candidate-a-rgb",
  "candidate-b-geometry",
  "candidate-c-canopy-repair",
  "canopy-mask",
  "structural-edges",
] as const;

function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}
function scene(kind: string): Buffer {
  const size = 128;
  const pixels = new Uint8Array(size * size * 4);
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const offset = (y * size + x) * 4;
      const tower = x > 54 && x < 72 && y > 26 && y < 94;
      const canopy = (x - 38) ** 2 + (y - 42) ** 2 < 22 ** 2;
      const edge = x === 54 || x === 71 || y === 26 || y === 93;
      const mask = kind === "canopy-mask" ? canopy : kind === "structural-edges" ? edge : false;
      const color: [number, number, number] = kind.endsWith("mask") || kind === "structural-edges"
        ? mask ? [255, 255, 255] : [0, 0, 0]
        : tower
          ? [214, 194, 153]
          : canopy
            ? kind === "candidate-c-canopy-repair" ? [64, 84, 61] : [45, 68, 44]
            : [181, 91, 61];
      pixels.set([...color, 255], offset);
    }
  }
  return encodePng(pixels, size, size, 6);
}

export async function installRepairFixture(page: Page, corruptHash = false) {
  const artifacts = new Map(IDS.map((id) => [`${id}.png`, scene(id)]));
  const images = IDS.map((id, index) => {
    const bytes = artifacts.get(`${id}.png`)!;
    return {
      byte_length: bytes.length,
      height_px: 128,
      id,
      label: [
        "Google source at logical scale",
        "Candidate A: RGB-only abstraction",
        "Candidate B: geometry-guided abstraction",
        "Candidate C: filtered architecture plus canopy repair",
        "High-confidence canopy repair mask",
        "Depth and normal structural edges",
      ][index],
      path: `${id}.png`,
      sha256: corruptHash && id === "candidate-c-canopy-repair" ? "0".repeat(64) : sha256(bytes),
      width_px: 128,
    };
  });
  const report = {
    algorithm: "reference-repair-rust/v1",
    blocking_findings: [
      "construction-region-lacks-an-accepted-instance-mask",
      "candidate-c-repairs-only-high-confidence-canopy",
      "visual-style-approval-is-human-owned",
    ],
    camera_azimuth_millidegrees: 330_000,
    camera_elevation_millidegrees: 42_000,
    candidates: [
      ["candidate-a-rgb", 164_645, 8_333, 33, 188_935],
      ["candidate-b-geometry", 192_837, 9_238, 47, 207_199],
      ["candidate-c-canopy-repair", 104_147, 9_730, 40, 169_685],
    ].map(([candidate_id, canopy, recall, colors, noise]) => ({
      candidate_id,
      canopy_interior_edge_ppm: canopy,
      changed_from_source_ppm: 999_900,
      colors_used: colors,
      mean_luminance_microunits: 75_000_000,
      non_structural_edge_ppm: noise,
      structural_edge_recall_basis_points: recall,
    })),
    canopy_pixels: 496_276,
    estimated_peak_working_bytes: 69_216_256,
    gates: {
      canopy_fragmentation_improved: true,
      deterministic_post_capture: true,
      palette_bound: true,
      passenger_cars_preserved_by_policy: true,
      qualified_for_expansion: false,
      structural_edge_recall: true,
    },
    images,
    logical_millimeters_per_pixel: 250,
    schema: "isometric-reference-repair-review/v1",
    source_bundle_id: "synthetic-hoover-repair",
    source_manifest_sha256: "a".repeat(64),
    source_millimeters_per_pixel: 125,
    structural_edge_pixels: 141_807,
  };
  const reportBytes = Buffer.from(`${JSON.stringify(report)}\n`);
  await page.route(REPORT_ROUTE, (route) => route.fulfill({
    body: reportBytes,
    contentType: "application/json",
    headers: { "content-length": String(reportBytes.length) },
    status: 200,
  }));
  for (const [path, bytes] of artifacts) {
    await page.route(IMAGE_ROUTE(path), (route) => route.fulfill({
      body: bytes,
      contentType: "image/png",
      headers: { "content-length": String(bytes.length) },
      status: 200,
    }));
  }
}

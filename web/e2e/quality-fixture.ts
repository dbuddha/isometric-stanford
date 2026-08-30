import { createHash } from "node:crypto";
import type { Page } from "@playwright/test";
import { encodePng } from "../../capture/src/node/png.js";

const REPORT_ROUTE = "**/fixture/quality/quality-review.json";
const IMAGE_ROUTE = (path: string) => `**/fixture/quality/${path}`;
const IDS = [
  "baseline-sse20-250mm",
  "lod-sse8-250mm",
  "lod-sse4-250mm",
  "sample-sse8-125mm",
  "maximum-sse4-125mm",
] as const;

function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function scene(size: number, detailed: boolean): Buffer {
  const pixels = new Uint8Array(size * size * 4);
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const offset = (y * size + x) * 4;
      const tower = x > size * 0.43 && x < size * 0.56 && y > size * 0.25 && y < size * 0.72;
      const tree = (x - size * 0.36) ** 2 + (y - size * 0.3) ** 2 < (size * 0.12) ** 2;
      const color: [number, number, number] = tower
        ? [201, 169, 126]
        : tree
          ? detailed && (x + y) % 7 < 2
            ? [48, 88, 45]
            : [38, 68, 39]
          : [165, 71, 45];
      pixels.set([...color, 255], offset);
    }
  }
  return encodePng(pixels, size, size, 6);
}

export async function installQualityFixture(page: Page, corruptHash = false) {
  const baseline = scene(1_024, false);
  const detailed = scene(1_024, true);
  const sampled = scene(2_048, true);
  const artifacts = new Map<string, Buffer>();
  const attempts = [266, 784, 784, 784, 784];
  const candidates = IDS.map((id, index) => {
    const highSampling = index >= 3;
    const bytes = index === 0 ? baseline : highSampling ? sampled : detailed;
    const path = `candidates/${id}/core.png`;
    artifacts.set(path, bytes);
    const errorTarget = index === 0 ? 20 : index === 2 || index === 4 ? 4 : 8;
    return {
      evidence: {
        coreCoverageBasisPoints: 9_999,
        diagnostics: {
          cachedBytes: index === 0 ? 154_261_894 : 408_005_115,
          errorTarget,
          triangles: index === 0 ? 237_150 : 1_370_554,
          visibleTileDepthMaximum: index === 0 ? 23 : 25,
        },
        elapsedMs: index === 0 ? 6_135 : index === 1 ? 4_891 : 2_001,
        networkAfterCandidate: { attempted: attempts[index] },
        visibleTiles: index === 0 ? 73 : 224,
      },
      image: {
        byteLength: bytes.length,
        candidateId: id,
        heightPx: highSampling ? 2_048 : 1_024,
        path,
        requestDelta: index === 0 ? 266 : index === 1 ? 518 : 0,
        sha256: corruptHash && index === 0 ? "0".repeat(64) : sha256(bytes),
        widthPx: highSampling ? 2_048 : 1_024,
      },
      label: [
        "Current baseline",
        "Finer Google LOD",
        "Aggressive Google LOD",
        "Two-times raster sampling",
        "Maximum bounded detail",
      ][index],
      request: {
        camera: {
          azimuthMillidegrees: 330_000,
          elevationMillidegrees: 42_000,
          orthographicHeightMm: 320_000,
          orthographicWidthMm: 320_000,
        },
        quality: {
          maxScreenSpaceErrorPx: errorTarget,
          maximumTileCacheMiB: 2_048,
          minimumTileCacheMiB: 512,
          textureMipmaps: false,
        },
        tile: {
          coreHeightPx: highSampling ? 2_048 : 1_024,
          coreWidthPx: highSampling ? 2_048 : 1_024,
          guardPx: highSampling ? 256 : 128,
          millimetersPerPixel: highSampling ? 125 : 250,
        },
      },
    };
  });
  const report = {
    candidates,
    conclusions: {
      deepestSourceLod: "sse8-250mm",
      historicalImagerySelectorAvailable: false,
      sourceLodPlateau: true,
      supersamplingAddsSourceGeometry: false,
    },
    network: {
      attempted: 784,
      billableRootRequests: 1,
      blocked: 0,
      completed: 784,
      failed: 0,
      requestLimit: 1_000,
    },
    runtime: { processTree: { peak: { treeBytes: 1_819_934_720 } } },
    schema: "isometric-reference-quality-review/v1",
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

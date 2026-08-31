import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { REQUIRED_LAYER_NAMES, cameraFingerprint } from "../src/contracts.js";
import type { CaptureRequest, ProbeCandidateEvidence } from "../src/contracts.js";
import {
  assertAtlasCameraRegistration,
  readAtlasCaptureSpec,
} from "../src/node/atlas-capture-runner.js";
import { deriveRegisteredAtlasRequests } from "../src/node/registered-grid.js";

function evidence(
  candidateId: string,
  request: CaptureRequest,
  center: { x: number; y: number },
  cameraWorldMatrix: number[],
): ProbeCandidateEvidence {
  const widthMeters = request.camera.orthographicWidthMm / 1_000;
  const heightMeters = request.camera.orthographicHeightMm / 1_000;
  const projectionMatrix = Array<number>(16).fill(0);
  projectionMatrix[0] = 2 / widthMeters;
  projectionMatrix[5] = 2 / heightMeters;
  projectionMatrix[10] = -1;
  projectionMatrix[12] = -2 * center.x / (request.tile.coreWidthPx + 2 * request.tile.guardPx);
  projectionMatrix[13] = -2 * center.y / (request.tile.coreHeightPx + 2 * request.tile.guardPx);
  projectionMatrix[15] = 1;
  return {
    attributions: ["Google Maps", "fixture:synthetic-camera"],
    cameraFingerprint: cameraFingerprint(request),
    cameraWorldMatrix: [...cameraWorldMatrix],
    candidateId,
    complete: true,
    coreCoverageBasisPoints: 10_000,
    diagnostics: {
      cachedBytes: 1,
      cachedTiles: 1,
      errorTarget: 8,
      geometries: 1,
      maxCachedBytes: 1,
      textures: 1,
      triangles: 1,
      visibleTileDepthMaximum: 1,
      visibleTileDepthMedian: 1,
      visibleTileDepthMinimum: 1,
      visibleTileErrorMaximumMillipixels: 8_000,
      visibleTileErrorMedianMillipixels: 8_000,
      visibleTileErrorP95Millipixels: 8_000,
    },
    elapsedMs: 1,
    layerOrder: [...REQUIRED_LAYER_NAMES],
    networkAfterCandidate: {
      attempted: 1,
      rootTilesetRequests: 1,
      blocked: 0,
      completed: 1,
      failed: 0,
      formats: { json: 1 },
      requestLimit: 4_000,
      responseBodyBytes: 1,
      rootTilesetSha256: "0".repeat(64),
      statuses: { "200": 1 },
    },
    projectionMatrix,
    stableFrames: 2,
    visibleTiles: 1,
  };
}

describe("one-session Hoover atlas capture contract", () => {
  it("reads only the approved live profile", async () => {
    const spec = await readAtlasCaptureSpec(resolve("specs/hoover-atlas-capture.json"));
    expect(spec).toMatchObject({
      atlasId: "hoover-google-reference-atlas",
      requestLimit: 4_000,
      schema: "isometric-reference-atlas-capture/v1",
      workerEnvelopeMiB: 2_048,
    });
  });

  it("proves one fixed camera and the exact off-axis centers", async () => {
    const raw = JSON.parse(
      await readFile(resolve("specs/hoover-atlas-capture.json"), "utf8"),
    ) as { capture: unknown };
    const requests = deriveRegisteredAtlasRequests(raw.capture);
    const cameraWorldMatrix = Array.from({ length: 16 }, (_, index) =>
      index % 5 === 0 ? 1 : 0,
    );
    const candidates = requests.ordered.map((request, index) => {
      const grid = requests.grid.candidates[index]!;
      return evidence(grid.candidateId, request, grid.actualCenterOffsetPixels, cameraWorldMatrix);
    });
    const report = assertAtlasCameraRegistration(candidates, requests);
    expect(report.fixedWorldMatrix).toBe(true);
    expect(report.horizontalPixelsPerMeter).toBe(8);
    expect(report.verticalPixelsPerMeter).toBe(8);
    expect(report.maximumProjectionCenterErrorPixels).toBeLessThanOrEqual(1e-6);

    candidates[3]!.cameraWorldMatrix[12] = 1;
    expect(() => assertAtlasCameraRegistration(candidates, requests)).toThrow(
      "moved or reordered",
    );
  });
});

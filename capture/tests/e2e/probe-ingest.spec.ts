import { mkdtemp, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { expect, test } from "@playwright/test";
import { REQUIRED_LAYER_NAMES, cameraFingerprint } from "../../src/contracts.js";
import type { LayerName, ProbeCandidateEvidence } from "../../src/contracts.js";
import { startProbeIngest } from "../../src/node/probe-ingest-client.js";
import { syntheticRequest } from "../fixtures.js";

test("credential-free ingest worker promotes one complete isolated probe bundle", async () => {
  const root = await mkdtemp(resolve(tmpdir(), "isometric-probe-ingest-e2e-"));
  const request = syntheticRequest();
  request.bundleId = "probe-ingest-e2e";
  request.provider = "google-photorealistic-3d-tiles";
  request.tile.coreWidthPx = 1_024;
  request.tile.coreHeightPx = 1_024;
  request.tile.guardPx = 128;
  request.camera.orthographicWidthMm = 2_560_000;
  request.camera.orthographicHeightMm = 2_560_000;
  const width = 1_280;
  const height = 1_280;
  const rgba = new Uint8Array(width * height * 4).fill(127);
  const gray = new Uint8Array(width * height).fill(255);
  const depth = new Uint8Array(16 + width * height * 4);
  depth.set(new TextEncoder().encode("ISOD32V1"));
  const depthView = new DataView(depth.buffer);
  depthView.setUint32(8, width, true);
  depthView.setUint32(12, height, true);
  const ingest = await startProbeIngest(root, [{ candidateId: "isolated", request }]);
  try {
    const target = ingest.targets[0];
    expect(target?.candidateId).toBe("isolated");
    if (target === undefined) {
      throw new Error("probe ingest target is missing");
    }
    const layers: Array<[LayerName, Uint8Array, "gray8" | "rgba8" | "u32le-millimeters"]> = [
      ["color", rgba, "rgba8"],
      ["whitebox", rgba, "rgba8"],
      ["linear-depth", depth, "u32le-millimeters"],
      ["view-normal", rgba, "rgba8"],
      ["fixed-shadow", gray, "gray8"],
      ["coverage", gray, "gray8"],
    ];
    for (const [name, bytes, pixelFormat] of layers) {
      const response = await fetch(`${target.upload.url}/layer/${name}`, {
        body: Buffer.from(bytes),
        headers: {
          "content-type": "application/octet-stream",
          "x-capture-height": String(height),
          "x-capture-pixel-format": pixelFormat,
          "x-capture-token": target.upload.token,
          "x-capture-width": String(width),
        },
        method: "POST",
      });
      expect(response.status).toBe(204);
    }
    const evidence: ProbeCandidateEvidence = {
      attributions: ["Google Maps", "copyright:synthetic-e2e"],
      cameraFingerprint: cameraFingerprint(request),
      cameraWorldMatrix: Array.from({ length: 16 }, (_, index) => (index % 5 === 0 ? 1 : 0)),
      candidateId: "isolated",
      complete: true,
      coreCoverageBasisPoints: 10_000,
      diagnostics: {
        cachedBytes: 1,
        cachedTiles: 1,
        errorTarget: 20,
        geometries: 1,
        maxCachedBytes: 1,
        textures: 1,
        triangles: 1,
        visibleTileDepthMaximum: 9,
        visibleTileDepthMedian: 8,
        visibleTileDepthMinimum: 7,
        visibleTileErrorMaximumMillipixels: 20_000,
        visibleTileErrorMedianMillipixels: 12_000,
        visibleTileErrorP95Millipixels: 18_000,
      },
      elapsedMs: 1,
      layerOrder: [...REQUIRED_LAYER_NAMES],
      networkAfterCandidate: {
        attempted: 2,
        billableRootRequests: 1,
        blocked: 0,
        completed: 2,
        failed: 0,
        formats: { glb: 1, json: 1 },
        requestLimit: 10,
        responseBodyBytes: 0,
        statuses: { "200": 2 },
      },
      projectionMatrix: Array.from({ length: 16 }, (_, index) => (index % 5 === 0 ? 1 : 0)),
      stableFrames: 3,
      visibleTiles: 1,
    };
    const finalized = await ingest.finalize([evidence]);
    expect(finalized.results).toHaveLength(1);
    expect(finalized.results[0]?.artifacts.mismatchPixels).toBe(0);
    expect(finalized.workerMaxRssBytes).toBeLessThan(384 * 1_024 * 1_024);
    expect((await stat(resolve(root, "bundles/isolated/reference.manifest.json"))).size).toBeGreaterThan(0);
  } finally {
    await ingest.abort();
    await rm(root, { force: true, recursive: true });
  }
});

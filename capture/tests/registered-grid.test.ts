import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { readFile } from "node:fs/promises";
import {
  deriveRegisteredAtlasRequests,
  deriveRegisteredOverlapRequests,
} from "../src/node/registered-grid.js";

describe("registered Hoover overlap grid", () => {
  it("derives both neighboring targets from one anchor and round-trips every saved pixel center", async () => {
    const spec = JSON.parse(
      await readFile(resolve("specs/hoover-overlap-probe.json"), "utf8"),
    ) as { capture: unknown; requestLimit: number; schema: string };
    expect(spec.schema).toBe("isometric-reference-overlap-probe/v1");
    expect(spec.requestLimit).toBe(450);
    const requests = deriveRegisteredOverlapRequests(spec.capture);
    expect(requests.ordered.map((request) => request.bundleId)).toEqual([
      "hoover-overlap-monolithic",
      "hoover-overlap-left",
      "hoover-overlap-right",
    ]);
    expect(requests.monolithic.tile).toMatchObject({
      coreHeightPx: 1024,
      coreWidthPx: 2048,
      guardPx: 128,
    });
    expect(requests.left.tile).toMatchObject({
      coreHeightPx: 1024,
      coreWidthPx: 1024,
      guardPx: 128,
    });
    expect(requests.right.tile).toMatchObject({
      coreHeightPx: 1024,
      coreWidthPx: 1024,
      guardPx: 128,
    });
    expect(requests.left.camera.orthographicWidthMm).toBe(320_000);
    expect(requests.right.camera.orthographicWidthMm).toBe(320_000);
    expect(requests.grid.cameraScreenRightBearingMillidegrees).toBe(60_000);
    expect(requests.grid.checkedSavedPixelCenters).toBe(2 * 1024 * 1024);
    expect(requests.grid.maximumPixelCenterErrorPixels).toBeLessThanOrEqual(0.5);
    expect(requests.left.tile.centerLongitudeE7).not.toBe(
      requests.monolithic.tile.centerLongitudeE7,
    );
    expect(requests.right.tile.centerLongitudeE7).not.toBe(
      requests.monolithic.tile.centerLongitudeE7,
    );
  });

  it("rejects an unapproved scale or pilot layout", async () => {
    const spec = JSON.parse(
      await readFile(resolve("specs/hoover-overlap-probe.json"), "utf8"),
    ) as {
      capture: {
        camera: { orthographicHeightMm: number; orthographicWidthMm: number };
        tile: { millimetersPerPixel: number };
      };
    };
    spec.capture.tile.millimetersPerPixel = 251;
    spec.capture.camera.orthographicWidthMm = 2_304 * 251;
    spec.capture.camera.orthographicHeightMm = 1_280 * 251;
    expect(() => deriveRegisteredOverlapRequests(spec.capture)).toThrow(
      "approved 2048 by 1024 Hoover grid",
    );
  });
});

describe("registered Hoover atlas grid", () => {
  it("derives four row-major fixed-camera cells on one 125 millimeter grid", async () => {
    const spec = JSON.parse(
      await readFile(resolve("specs/hoover-atlas-capture.json"), "utf8"),
    ) as { capture: unknown; requestLimit: number; schema: string; workerEnvelopeMiB: number };
    expect(spec.schema).toBe("isometric-reference-atlas-capture/v1");
    expect(spec.requestLimit).toBe(1_000);
    expect(spec.workerEnvelopeMiB).toBe(2_048);
    const requests = deriveRegisteredAtlasRequests(spec.capture);
    expect(requests.ordered.map((request) => request.bundleId)).toEqual([
      "hoover-atlas-r0c0",
      "hoover-atlas-r0c1",
      "hoover-atlas-r1c0",
      "hoover-atlas-r1c1",
    ]);
    expect(
      requests.ordered.map((request) => [request.tile.row, request.tile.column]),
    ).toEqual([
      [0, 0],
      [0, 1],
      [1, 0],
      [1, 1],
    ]);
    for (const request of requests.ordered) {
      expect(request.tile).toMatchObject({
        coreHeightPx: 2_048,
        coreWidthPx: 2_048,
        guardPx: 256,
        millimetersPerPixel: 125,
      });
      expect(request.camera).toMatchObject({
        azimuthMillidegrees: 330_000,
        elevationMillidegrees: 42_000,
        orthographicHeightMm: 320_000,
        orthographicWidthMm: 320_000,
      });
      expect(request.quality.maxScreenSpaceErrorPx).toBe(8);
    }
    expect(requests.grid.candidates.map(({ expectedCenterOffsetPixels }) =>
      expectedCenterOffsetPixels,
    )).toEqual([
      { x: 0, y: 0 },
      { x: 2_048, y: 0 },
      { x: 0, y: -2_048 },
      { x: 2_048, y: -2_048 },
    ]);
    expect(requests.grid.checkedSavedPixelCenters).toBe(4 * 2_048 * 2_048);
    expect(requests.grid.maximumPixelCenterErrorPixels).toBeLessThanOrEqual(0.5);
  });

  it("rejects a changed source scale or Google LOD", async () => {
    const spec = JSON.parse(
      await readFile(resolve("specs/hoover-atlas-capture.json"), "utf8"),
    ) as {
      capture: {
        camera: { orthographicHeightMm: number; orthographicWidthMm: number };
        quality: { maxScreenSpaceErrorPx: number };
        tile: { millimetersPerPixel: number };
      };
    };
    spec.capture.quality.maxScreenSpaceErrorPx = 9;
    expect(() => deriveRegisteredAtlasRequests(spec.capture)).toThrow(
      "approved 2048 square Hoover profile",
    );
    spec.capture.quality.maxScreenSpaceErrorPx = 8;
    spec.capture.tile.millimetersPerPixel = 126;
    spec.capture.camera.orthographicWidthMm = 2_560 * 126;
    spec.capture.camera.orthographicHeightMm = 2_560 * 126;
    expect(() => deriveRegisteredAtlasRequests(spec.capture)).toThrow(
      "approved 2048 square Hoover profile",
    );
  });
});

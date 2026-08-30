import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { readFile } from "node:fs/promises";
import { deriveRegisteredOverlapRequests } from "../src/node/registered-grid.js";

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

import { CAPTURE_SCHEMA } from "../src/contracts.js";
import type { CaptureRequest } from "../src/contracts.js";

export function syntheticRequest(): CaptureRequest {
  return {
    schema: CAPTURE_SCHEMA,
    bundleId: "synthetic-0000-0000",
    provider: "synthetic",
    sourceEpoch: "2026-08-18T00:00:00Z",
    tile: {
      regionId: "synthetic-pilot",
      column: 0,
      row: 0,
      coreWidthPx: 64,
      coreHeightPx: 64,
      guardPx: 8,
      millimetersPerPixel: 2_000,
      centerLongitudeE7: -1_221_697_000,
      centerLatitudeE7: 374_275_000,
    },
    camera: {
      projection: "orthographic",
      azimuthMillidegrees: 315_000,
      elevationMillidegrees: 42_000,
      targetAltitudeMm: 20_000,
      nearMm: 1_000,
      farMm: 5_000_000,
      orthographicWidthMm: 160_000,
      orthographicHeightMm: 160_000,
      cameraDistanceMm: 500_000,
    },
    lighting: {
      sunAzimuthMillidegrees: 315_000,
      sunElevationMillidegrees: 42_000,
    },
    readiness: {
      timeoutMs: 10_000,
      stableFrames: 3,
      stableDurationMs: 100,
      minimumVisibleTiles: 1,
    },
  };
}

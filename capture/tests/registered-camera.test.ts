import { describe, expect, it } from "vitest";

import { registeredOrthographicFrustum } from "../src/browser/registered-camera.js";

describe("fixed-camera registered orthographic frustum", () => {
  it("keeps shared world pixels invariant while moving only the frustum center", () => {
    const millimetersPerPixel = 250;
    const monolithic = registeredOrthographicFrustum(576, 320, 0, 0);
    const left = registeredOrthographicFrustum(320, 320, -128, 0);
    const right = registeredOrthographicFrustum(320, 320, 128, 0);
    const pixel = (worldX: number, frustumLeft: number) =>
      (worldX - frustumLeft) * 1_000 / millimetersPerPixel - 0.5;
    for (const worldX of [-32, -0.125, 0.125, 31.875]) {
      expect(pixel(worldX, left.left) - pixel(worldX, monolithic.left)).toBeCloseTo(0, 10);
      expect(pixel(worldX, right.left) - pixel(worldX, monolithic.left)).toBeCloseTo(-1_024, 10);
    }
  });

  it("rejects non-finite and non-positive frusta", () => {
    expect(() => registeredOrthographicFrustum(0, 320, 0, 0)).toThrow(/finite positive/);
    expect(() => registeredOrthographicFrustum(320, Number.NaN, 0, 0)).toThrow(/finite positive/);
  });
});

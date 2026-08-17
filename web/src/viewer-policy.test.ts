import { describe, expect, it } from "vitest";

import { viewerPolicy } from "./viewer-policy";

describe("viewerPolicy", () => {
  it("keeps a phone below the decoded memory budget without deviceMemory", () => {
    const policy = viewerPolicy(390);
    expect(policy).toEqual({
      constrainedDevice: true,
      decodedBudgetBytes: 96 * 1_024 * 1_024,
      maxImageCacheCount: 48,
      initialZoomFactor: 2.25,
    });
    expect(policy.maxImageCacheCount * 512 * 512 * 4).toBeLessThanOrEqual(
      policy.decodedBudgetBytes / 2,
    );
  });

  it("treats low-memory wide devices as constrained", () => {
    expect(viewerPolicy(1_024, 4).maxImageCacheCount).toBe(48);
  });

  it("treats wide coarse-pointer devices as constrained without memory reporting", () => {
    const policy = viewerPolicy(900, undefined, true);
    expect(policy.constrainedDevice).toBe(true);
    expect(policy.maxImageCacheCount).toBe(48);
    expect(policy.initialZoomFactor).toBe(1);
  });

  it("uses the desktop budget and neutral initial zoom on capable screens", () => {
    const policy = viewerPolicy(1_280, 8);
    expect(policy.maxImageCacheCount).toBe(128);
    expect(policy.decodedBudgetBytes).toBe(256 * 1_024 * 1_024);
    expect(policy.initialZoomFactor).toBe(1);
  });
});

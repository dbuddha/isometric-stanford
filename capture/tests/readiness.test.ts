import { describe, expect, it } from "vitest";
import { TileReadiness, waitForStableReadiness } from "../src/readiness.js";

describe("tile readiness", () => {
  it("requires root load, load completion, stable frames, and stable duration", () => {
    const readiness = new TileReadiness({
      minimumVisibleTiles: 2,
      stableDurationMs: 100,
      stableFrames: 3,
    });
    expect(readiness.observe("", 0, 0).complete).toBe(false);
    readiness.rootLoaded();
    readiness.loadEnded();
    expect(readiness.observe("2:2:1", 2, 10).complete).toBe(false);
    expect(readiness.observe("2:2:1", 2, 60).complete).toBe(false);
    expect(readiness.observe("2:2:1", 2, 110).complete).toBe(true);
  });

  it("resets stability when visible content changes and never recovers from an error", () => {
    const readiness = new TileReadiness({
      minimumVisibleTiles: 1,
      stableDurationMs: 10,
      stableFrames: 2,
    });
    readiness.rootLoaded();
    readiness.loadEnded();
    readiness.observe("first", 1, 0);
    expect(readiness.observe("second", 1, 20).stableFrames).toBe(1);
    readiness.loadFailed();
    expect(readiness.observe("second", 1, 40).complete).toBe(false);
    expect(readiness.snapshot(40).errors).toBe(1);
  });

  it("fails closed when stable readiness exceeds its deadline", async () => {
    let now = 0;
    await expect(
      waitForStableReadiness({
        nextFrame: async () => {
          now += 60;
        },
        now: () => now,
        sample: () => ({
          complete: false,
          errors: 0,
          loading: true,
          rootLoaded: false,
          stableFrames: 0,
          visibleTiles: 0,
        }),
        timeoutMs: 100,
      }),
    ).rejects.toThrow(/timed out/);
  });
});

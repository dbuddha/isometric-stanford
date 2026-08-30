import { describe, expect, it } from "vitest";
import {
  captureWorkerEnvelopeBytes,
  deriveCaptureWorkerCount,
} from "../src/node/capture-memory-policy.js";

const GIBIBYTE = 1_024 * 1_024 * 1_024;

describe("capture memory policy", () => {
  it("uses conservative measured envelopes for approved grids", () => {
    expect(captureWorkerEnvelopeBytes(1_280)).toBe(GIBIBYTE);
    expect(captureWorkerEnvelopeBytes(2_560)).toBe((5 * GIBIBYTE) / 4);
    expect(captureWorkerEnvelopeBytes(2_560, 2 * GIBIBYTE)).toBe(2 * GIBIBYTE);
    expect(() => captureWorkerEnvelopeBytes(2_561)).toThrow(/no measured envelope/);
  });

  it("reserves host memory and caps acquisition concurrency", () => {
    expect(deriveCaptureWorkerCount(2 * GIBIBYTE, 1_280)).toBe(0);
    expect(deriveCaptureWorkerCount(4 * GIBIBYTE, 1_280)).toBe(2);
    expect(deriveCaptureWorkerCount(4 * GIBIBYTE, 2_560)).toBe(1);
    expect(deriveCaptureWorkerCount(8 * GIBIBYTE, 1_280)).toBe(4);
    expect(deriveCaptureWorkerCount(32 * GIBIBYTE, 2_560)).toBe(4);
    expect(deriveCaptureWorkerCount(8 * GIBIBYTE, 2_560, 2 * GIBIBYTE)).toBe(3);
  });
});

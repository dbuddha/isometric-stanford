import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { cameraFingerprint } from "../src/contracts.js";
import { readProbeSpec } from "../src/node/probe-runner.js";

describe("Hoover camera probe", () => {
  it("pins three repeatable orthographic cameras under a measured request cap", async () => {
    const spec = await readProbeSpec(resolve("specs/hoover-camera-probe.json"));
    expect(spec.requestLimit).toBe(400);
    expect(spec.capture.tile.centerLatitudeE7).toBe(374276111);
    expect(spec.capture.tile.centerLongitudeE7).toBe(-1221670000);
    expect(spec.candidates.map((candidate) => candidate.id)).toEqual([
      "cannoneyed-345-45",
      "stanford-330-42",
      "diagonal-315-42",
    ]);
    const fingerprints = spec.candidates.map((candidate) => {
      const request = structuredClone(spec.capture);
      request.camera.azimuthMillidegrees = candidate.azimuthMillidegrees;
      request.camera.elevationMillidegrees = candidate.elevationMillidegrees;
      return cameraFingerprint(request);
    });
    expect(new Set(fingerprints).size).toBe(3);
    expect(fingerprints).toEqual(
      spec.candidates.map((candidate) => {
        const request = structuredClone(spec.capture);
        request.camera.azimuthMillidegrees = candidate.azimuthMillidegrees;
        request.camera.elevationMillidegrees = candidate.elevationMillidegrees;
        return cameraFingerprint(request);
      }),
    );
  });
});

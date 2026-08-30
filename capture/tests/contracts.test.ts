import { describe, expect, it } from "vitest";
import {
  REQUIRED_LAYER_NAMES,
  cameraFingerprint,
  redactSecrets,
  validateCaptureRequest,
} from "../src/contracts.js";
import { syntheticRequest } from "./fixtures.js";

describe("capture contracts", () => {
  it("accepts one exact registered request", () => {
    const request = syntheticRequest();
    expect(() => validateCaptureRequest(request)).not.toThrow();
    expect(cameraFingerprint(request).split(":")).toHaveLength(13);
    expect(REQUIRED_LAYER_NAMES).toEqual([
      "color",
      "whitebox",
      "linear-depth",
      "view-normal",
      "fixed-shadow",
      "coverage",
    ]);
  });

  it("rejects a camera span that does not match the pixel grid", () => {
    const request = syntheticRequest();
    request.camera.orthographicWidthMm += 1;
    expect(() => validateCaptureRequest(request)).toThrow(/orthographic span/);
  });

  it("redacts direct credentials and URL query credentials", () => {
    const secret = "test-secret-value";
    const message = `failed ${secret} at https://tiles.test/root?key=${secret}&session=session-value`;
    const redacted = redactSecrets(message, [secret]);
    expect(redacted).not.toContain(secret);
    expect(redacted).not.toContain("session-value");
    expect(redacted).toContain("key=[REDACTED]");
    expect(redacted).toContain("session=[REDACTED]");
  });
});

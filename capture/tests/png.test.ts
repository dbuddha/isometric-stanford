import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import { encodePng } from "../src/node/png.js";

describe("portable PNG encoder", () => {
  it("emits deterministic RGBA and grayscale IHDR contracts", () => {
    const rgba = encodePng(new Uint8Array([255, 0, 0, 255]), 1, 1, 6);
    const gray = encodePng(new Uint8Array([127]), 1, 1, 0);
    expect(rgba.subarray(1, 4).toString()).toBe("PNG");
    expect(rgba[24]).toBe(8);
    expect(rgba[25]).toBe(6);
    expect(gray[25]).toBe(0);
    expect(createHash("sha256").update(rgba).digest("hex")).toBe(
      createHash("sha256").update(encodePng(new Uint8Array([255, 0, 0, 255]), 1, 1, 6)).digest("hex"),
    );
  });
});

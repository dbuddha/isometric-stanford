import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { encodePng, writePngFile, writePngFileFromRaw } from "../src/node/png.js";

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

  it("streams deterministic byte-valid files with matching evidence", async () => {
    const directory = await mkdtemp(resolve(tmpdir(), "isometric-png-"));
    try {
      const pixels = new Uint8Array(128 * 64 * 4);
      for (let index = 0; index < pixels.length; index += 1) {
        pixels[index] = index % 251;
      }
      const firstPath = resolve(directory, "first.png");
      const secondPath = resolve(directory, "second.png");
      const rawPath = resolve(directory, "pixels.raw");
      const rawPngPath = resolve(directory, "raw.png");
      const first = await writePngFile(firstPath, pixels, 128, 64, 6);
      const second = await writePngFile(secondPath, pixels, 128, 64, 6);
      await writeFile(rawPath, pixels);
      const raw = await writePngFileFromRaw(rawPngPath, rawPath, 128, 64, 6);
      const firstBytes = await readFile(firstPath);
      const secondBytes = await readFile(secondPath);
      const rawBytes = await readFile(rawPngPath);
      expect(firstBytes.equals(secondBytes)).toBe(true);
      expect(firstBytes.equals(rawBytes)).toBe(true);
      expect(firstBytes.subarray(1, 4).toString()).toBe("PNG");
      expect(first.byteLength).toBe(firstBytes.length);
      expect(first.sha256).toBe(createHash("sha256").update(firstBytes).digest("hex"));
      expect(second).toEqual(first);
      expect(raw).toEqual(first);
    } finally {
      await rm(directory, { recursive: true });
    }
  });
});

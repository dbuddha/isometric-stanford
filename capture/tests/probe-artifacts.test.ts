import { mkdtemp, readFile, rm, stat, truncate, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { ProbeArtifactWriter } from "../src/node/probe-artifacts.js";
import { syntheticRequest } from "./fixtures.js";

const roots: string[] = [];
const RUST_PROCESS_TEST_TIMEOUT_MS = 15_000;

afterEach(async () => {
  await Promise.all(roots.splice(0).map(async (root) => rm(root, { force: true, recursive: true })));
});

describe("probe cell evidence", () => {
  it("splits and rejoins two exact 512 pixel cells from one guarded core", async () => {
    const root = await mkdtemp(resolve(tmpdir(), "isometric-probe-cells-"));
    roots.push(root);
    const request = syntheticRequest();
    request.tile.coreWidthPx = 1_024;
    request.tile.coreHeightPx = 1_024;
    request.tile.guardPx = 128;
    request.camera.orthographicWidthMm = 2_560_000;
    request.camera.orthographicHeightMm = 2_560_000;
    const width = 1_280;
    const height = 1_280;
    const pixels = new Uint8Array(width * height * 4);
    for (let offset = 0; offset < pixels.length; offset += 4) {
      const pixel = offset / 4;
      pixels[offset] = pixel % 251;
      pixels[offset + 1] = Math.floor(pixel / width) % 241;
      pixels[offset + 2] = Math.floor(pixel / 97) % 239;
      pixels[offset + 3] = 255;
    }
    const writer = new ProbeArtifactWriter(resolve(root, "candidate"), request);
    await writer.accept("color", pixels, width, height, "rgba8");
    const evidence = writer.finalize();
    expect(evidence.mismatchPixels).toBe(0);
    expect(evidence.assembledRawSha256).toBe(evidence.sourceRawSha256);
    for (const filename of [
      "core.png",
      "cell-0-0.png",
      "cell-1-0.png",
      "joined-top.png",
    ]) {
      expect((await stat(resolve(root, "candidate", filename))).size).toBeGreaterThan(0);
    }
  });

  it("streams raw probe crops through bounded Rust processes", async () => {
    const root = await mkdtemp(resolve(tmpdir(), "isometric-probe-raw-cells-"));
    roots.push(root);
    const request = syntheticRequest();
    request.tile.coreWidthPx = 1_024;
    request.tile.coreHeightPx = 1_024;
    request.tile.guardPx = 128;
    request.camera.orthographicWidthMm = 2_560_000;
    request.camera.orthographicHeightMm = 2_560_000;
    const width = 1_280;
    const height = 1_280;
    const raw = resolve(root, "color.raw");
    const pixels = new Uint8Array(width * height * 4).fill(127);
    await writeFile(raw, pixels);
    const writer = new ProbeArtifactWriter(resolve(root, "candidate"), request);
    await writer.acceptFile("color", raw, pixels.length, width, height, "rgba8");
    const evidence = writer.finalize();
    expect(evidence.mismatchPixels).toBe(0);
    expect(evidence.assembledRawSha256).toBe(evidence.sourceRawSha256);
    for (const filename of [
      "core.png",
      "cell-0-0.png",
      "cell-1-0.png",
      "joined-top.png",
    ]) {
      expect((await stat(resolve(root, "candidate", filename))).size).toBeGreaterThan(0);
    }
  }, RUST_PROCESS_TEST_TIMEOUT_MS);

  it("streams the 2560-pixel pilot supertile without a full Node raster", async () => {
    const root = await mkdtemp(resolve(tmpdir(), "isometric-probe-pilot-cells-"));
    roots.push(root);
    const request = syntheticRequest();
    request.tile.coreWidthPx = 2_048;
    request.tile.coreHeightPx = 2_048;
    request.tile.guardPx = 256;
    request.camera.orthographicWidthMm = 640_000;
    request.camera.orthographicHeightMm = 640_000;
    const width = 2_560;
    const height = 2_560;
    const byteLength = width * height * 4;
    const raw = resolve(root, "color.raw");
    await writeFile(raw, new Uint8Array());
    await truncate(raw, byteLength);
    const writer = new ProbeArtifactWriter(resolve(root, "candidate"), request);
    await writer.acceptFile("color", raw, byteLength, width, height, "rgba8");
    const evidence = writer.finalize();
    expect(evidence.mismatchPixels).toBe(0);
    expect(evidence.assembledRawSha256).toBe(evidence.sourceRawSha256);
    const core = await readFile(resolve(root, "candidate", "core.png"));
    expect(core.readUInt32BE(16)).toBe(2_048);
    expect(core.readUInt32BE(20)).toBe(2_048);
  });
});

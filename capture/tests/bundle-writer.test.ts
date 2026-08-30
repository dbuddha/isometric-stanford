import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { REQUIRED_LAYER_NAMES, cameraFingerprint } from "../src/contracts.js";
import type { LayerName } from "../src/contracts.js";
import { BundleWriter } from "../src/node/bundle-writer.js";
import { syntheticRequest } from "./fixtures.js";

function payload(name: LayerName, width: number, height: number): Uint8Array {
  if (name === "linear-depth") {
    const bytes = new Uint8Array(16 + width * height * 4);
    bytes.set(new TextEncoder().encode("ISOD32V1"));
    const view = new DataView(bytes.buffer);
    view.setUint32(8, width, true);
    view.setUint32(12, height, true);
    return bytes;
  }
  const channels = name === "fixed-shadow" || name === "coverage" ? 1 : 4;
  return new Uint8Array(width * height * channels).fill(255);
}

describe("atomic bundle writer", () => {
  it("promotes only a complete ordered registered bundle", async () => {
    const root = await mkdtemp(resolve(tmpdir(), "isometric-capture-writer-"));
    const output = resolve(root, "bundle");
    const request = syntheticRequest();
    const width = request.tile.coreWidthPx + 2 * request.tile.guardPx;
    const height = request.tile.coreHeightPx + 2 * request.tile.guardPx;
    const writer = new BundleWriter(output, request);
    await writer.start();
    for (const name of REQUIRED_LAYER_NAMES) {
      const format =
        name === "linear-depth"
          ? "u32le-millimeters"
          : name === "fixed-shadow" || name === "coverage"
            ? "gray8"
            : "rgba8";
      await writer.accept(name, payload(name, width, height), width, height, format);
    }
    await writer.finalize(
      {
        attributions: ["fixture:synthetic"],
        cameraFingerprint: cameraFingerprint(request),
        complete: true,
        coreCoverageBasisPoints: 10_000,
        elapsedMs: 1,
        layerOrder: [...REQUIRED_LAYER_NAMES],
        stableFrames: 3,
        visibleTiles: 1,
      },
      async () => undefined,
    );
    const manifest = JSON.parse(await readFile(resolve(output, "reference.manifest.json"), "utf8"));
    expect(manifest.layers).toHaveLength(6);
    expect(manifest.capture.provider).toBe("synthetic");
    expect(manifest.layers.map((layer: { path: string }) => layer.path)).toEqual([
      "color.png",
      "whitebox.png",
      "depth.bin",
      "normal.png",
      "fixed-shadow.png",
      "coverage.png",
    ]);
    await rm(root, { force: true, recursive: true });
  });

  it("encodes a streamed raw layer through the bounded Rust encoder", async () => {
    const root = await mkdtemp(resolve(tmpdir(), "isometric-capture-rust-png-"));
    const request = syntheticRequest();
    const width = request.tile.coreWidthPx + 2 * request.tile.guardPx;
    const height = request.tile.coreHeightPx + 2 * request.tile.guardPx;
    const raw = resolve(root, "color.raw");
    const output = resolve(root, "bundle");
    await writeFile(raw, new Uint8Array(width * height * 4).fill(127));
    const writer = new BundleWriter(output, request);
    await writer.start();
    await writer.acceptFile("color", raw, width * height * 4, width, height, "rgba8");
    const signature = await readFile(resolve(writer.stagingDirectory, "color.png"));
    expect(signature.subarray(1, 4).toString()).toBe("PNG");
    await writer.abort();
    await rm(root, { force: true, recursive: true });
  });
});

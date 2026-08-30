import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { RawLayerArchive } from "../src/node/raw-layer-archive.js";
import { syntheticRequest } from "./fixtures.js";

const roots: string[] = [];

afterEach(async () => {
  await Promise.all(roots.splice(0).map(async (root) => rm(root, { recursive: true, force: true })));
});

describe("raw registered layer archive", () => {
  it("retains an exact private raw layer for bounded overlap comparison", async () => {
    const root = await mkdtemp(resolve(tmpdir(), "raw-layer-archive-"));
    roots.push(root);
    const request = syntheticRequest();
    const width = request.tile.coreWidthPx + request.tile.guardPx * 2;
    const height = request.tile.coreHeightPx + request.tile.guardPx * 2;
    const bytes = Buffer.alloc(width * height, 173);
    const source = resolve(root, "source.raw");
    await writeFile(source, bytes);
    const archive = new RawLayerArchive(resolve(root, "archive"), request);
    await archive.acceptFile("coverage", source, bytes.length, width, height, "gray8");
    expect(await readFile(resolve(root, "archive", "coverage.gray8"))).toEqual(bytes);
  });

  it("rejects dimensions outside the registered request", async () => {
    const root = await mkdtemp(resolve(tmpdir(), "raw-layer-archive-"));
    roots.push(root);
    const request = syntheticRequest();
    const source = resolve(root, "source.raw");
    await writeFile(source, Buffer.alloc(4));
    const archive = new RawLayerArchive(resolve(root, "archive"), request);
    await expect(
      archive.acceptFile("coverage", source, 4, 2, 2, "gray8"),
    ).rejects.toThrow("registered grid");
  });
});

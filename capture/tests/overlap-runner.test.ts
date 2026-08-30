import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { readOverlapSpec } from "../src/node/overlap-runner.js";

const roots: string[] = [];

afterEach(async () => {
  await Promise.all(roots.splice(0).map(async (root) => rm(root, { force: true, recursive: true })));
});

describe("registered overlap runner", () => {
  it("accepts only the exact one-session Hoover experiment contract", async () => {
    const spec = await readOverlapSpec(resolve("specs/hoover-overlap-probe.json"));
    expect(spec.requestLimit).toBe(450);
    expect(spec.capture.bundleId).toBe("hoover-overlap");
    expect(spec.capture.camera).toMatchObject({
      azimuthMillidegrees: 330_000,
      elevationMillidegrees: 42_000,
      cameraDistanceMm: 2_000_000,
    });
  });

  it("rejects a higher request ceiling before browser startup", async () => {
    const source = JSON.parse(
      await readFile(resolve("specs/hoover-overlap-probe.json"), "utf8"),
    ) as { requestLimit: number };
    source.requestLimit = 451;
    const root = await mkdtemp(resolve(tmpdir(), "invalid-overlap-spec-"));
    roots.push(root);
    const path = resolve(root, "invalid.json");
    await writeFile(path, JSON.stringify(source));
    await expect(readOverlapSpec(path)).rejects.toThrow("request ceiling");
  });
});

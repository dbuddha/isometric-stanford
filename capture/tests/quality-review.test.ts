import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { writeQualityReviewReport } from "../src/node/quality-review.js";
import { encodePng } from "../src/node/png.js";

const IDS = [
  "baseline-sse20-250mm",
  "lod-sse8-250mm",
  "lod-sse4-250mm",
  "sample-sse8-125mm",
  "maximum-sse4-125mm",
] as const;

describe("quality review evidence", () => {
  it("hashes bounded candidate images and proves an observed LOD plateau", async () => {
    const root = await mkdtemp(resolve(tmpdir(), "isometric-quality-"));
    try {
      const low = encodePng(new Uint8Array(4 * 4 * 4).fill(80), 4, 4, 6);
      const high = encodePng(new Uint8Array(8 * 8 * 4).fill(120), 8, 8, 6);
      const attempts = [10, 24, 24, 24, 24];
      const candidates = [];
      for (const [index, id] of IDS.entries()) {
        const directory = resolve(root, "candidates", id);
        await mkdir(directory, { recursive: true });
        await writeFile(resolve(directory, "core.png"), index < 3 ? low : high);
        candidates.push({
          artifacts: { coreFile: "core.png" },
          candidateId: id,
          evidence: {
            coreCoverageBasisPoints: 10_000,
            diagnostics: {
              cachedBytes: index === 0 ? 10 : 24,
              errorTarget: index === 0 ? 20 : index === 2 || index === 4 ? 4 : 8,
              triangles: index === 0 ? 10 : 24,
              visibleTileDepthMaximum: index === 0 ? 23 : 25,
              visibleTileDepthMedian: index === 0 ? 23 : 25,
              visibleTileDepthMinimum: index === 0 ? 23 : 25,
            },
            elapsedMs: 1,
            networkAfterCandidate: { attempted: attempts[index] },
            visibleTiles: 1,
          },
          label: id,
          request: {
            camera: { azimuthMillidegrees: 330_000, elevationMillidegrees: 42_000 },
            quality: { maxScreenSpaceErrorPx: index === 0 ? 20 : index === 2 || index === 4 ? 4 : 8 },
            tile: { millimetersPerPixel: index < 3 ? 250 : 125 },
          },
        });
      }
      await writeFile(
        resolve(root, "report.json"),
        JSON.stringify({
          candidates,
          network: { attempted: 24 },
          runtime: { processTree: { peak: { treeBytes: 1 } } },
          schema: "isometric-reference-probe-report/v1",
        }),
      );
      const output = await writeQualityReviewReport(root);
      const report = JSON.parse(await readFile(output, "utf8")) as {
        candidates: Array<{ image: { requestDelta: number } }>;
        conclusions: { deepestSourceLod: string; sourceLodPlateau: boolean };
      };
      expect(report.conclusions).toEqual(
        expect.objectContaining({ deepestSourceLod: "sse8-250mm", sourceLodPlateau: true }),
      );
      expect(report.candidates.map((candidate) => candidate.image.requestDelta)).toEqual([
        10, 14, 0, 0, 0,
      ]);
    } finally {
      await rm(root, { force: true, recursive: true });
    }
  });
});

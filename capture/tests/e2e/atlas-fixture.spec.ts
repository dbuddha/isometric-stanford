import { mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { expect, test } from "@playwright/test";
import type { CaptureRequest } from "../../src/contracts.js";
import { BundleWriter } from "../../src/node/bundle-writer.js";
import {
  compileRustAtlas,
  inspectRustAtlas,
  validateRustBundle,
} from "../../src/node/rust-reference.js";
import { startUploadServer } from "../../src/node/upload-server.js";
import type { UploadServer } from "../../src/node/upload-server.js";
import { syntheticRequest } from "../fixtures.js";

function fixtureRequest(row: number, column: number): CaptureRequest {
  const request = syntheticRequest();
  request.bundleId = `atlas-fixture-r${row}c${column}`;
  request.provider = "google-photorealistic-3d-tiles";
  request.sourceEpoch = "synthetic-google-contract-fixture";
  request.tile.regionId = "atlas-contract-fixture";
  request.tile.row = row;
  request.tile.column = column;
  request.tile.coreWidthPx = 32;
  request.tile.coreHeightPx = 32;
  request.tile.guardPx = 8;
  request.tile.millimetersPerPixel = 2_000;
  request.camera.orthographicWidthMm = 96_000;
  request.camera.orthographicHeightMm = 96_000;
  return request;
}

test("four original synthetic browser cells compile into a Google-only atlas contract", async ({ page }) => {
  const root = await mkdtemp(resolve(tmpdir(), "isometric-atlas-fixture-e2e-"));
  const writers: BundleWriter[] = [];
  const uploads: UploadServer[] = [];
  try {
    await page.goto("http://127.0.0.1:4317/");
    await page.waitForFunction(() => window.ISOMETRIC_CAPTURE?.ready === true);
    const bundleDirectories: string[] = [];
    for (const [row, column] of [
      [0, 0],
      [0, 1],
      [1, 0],
      [1, 1],
    ] as const) {
      const request = fixtureRequest(row, column);
      const output = resolve(root, "bundles", `r${row}c${column}`);
      const writer = new BundleWriter(output, request);
      writers.push(writer);
      await writer.start();
      const upload = await startUploadServer(writer);
      uploads.push(upload);
      const browserRequest = structuredClone(request);
      browserRequest.provider = "synthetic";
      const evidence = await page.evaluate(
        async ({ captureRequest, uploadTarget }) => {
          if (window.ISOMETRIC_CAPTURE === undefined) {
            throw new Error("capture runtime missing");
          }
          return window.ISOMETRIC_CAPTURE.capture(captureRequest, uploadTarget);
        },
        {
          captureRequest: browserRequest,
          uploadTarget: { token: upload.token, url: upload.url },
        },
      );
      await upload.close();
      await writer.finalize(evidence, async (path) => validateRustBundle(path));
      bundleDirectories.push(`bundles/r${row}c${column}`);
    }

    const atlasRequest = resolve(root, "atlas-request.json");
    await writeFile(
      atlasRequest,
      `${JSON.stringify(
        {
          atlas_id: "synthetic-google-atlas-fixture",
          bundle_directories: bundleDirectories,
          schema: "isometric-reference-atlas-request/v1",
          source_session: {
            expires_at: "2026-08-30T03:00:00.000Z",
            root_tileset_sha256: "1".repeat(64),
            session_id: "synthetic-google-root",
            started_at: "2026-08-30T00:00:00.000Z",
          },
        },
        null,
        2,
      )}\n`,
      { flag: "wx" },
    );
    const atlas = resolve(root, "atlas");
    expect(compileRustAtlas(atlasRequest, atlas)).toContain("4 tiles");
    expect(inspectRustAtlas(atlas)).toContain("4 tiles");
    const manifest = JSON.parse(
      await readFile(resolve(atlas, "reference-atlas.manifest.json"), "utf8"),
    ) as {
      grid: { columns: number; height_px: number; rows: number; width_px: number };
      layer_tiles: Array<{ path: string }>;
      provider: string;
      sources: unknown[];
    };
    expect(manifest.provider).toBe("google-photorealistic-3d-tiles");
    expect(manifest.grid).toMatchObject({ columns: 2, height_px: 64, rows: 2, width_px: 64 });
    expect(manifest.sources).toHaveLength(4);
    expect(manifest.layer_tiles).not.toHaveLength(0);
    expect((await stat(resolve(atlas, manifest.layer_tiles[0]!.path))).size).toBeGreaterThan(0);
  } finally {
    await Promise.all(uploads.map(async (upload) => upload.close().catch(() => undefined)));
    await Promise.all(writers.map(async (writer) => writer.abort().catch(() => undefined)));
    await rm(root, { force: true, recursive: true });
  }
});

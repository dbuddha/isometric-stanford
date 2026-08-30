import { mkdtemp, readFile, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { expect, test } from "@playwright/test";
import { cameraFingerprint } from "../../src/contracts.js";
import { BundleWriter } from "../../src/node/bundle-writer.js";
import { startUploadServer } from "../../src/node/upload-server.js";
import { syntheticRequest } from "../fixtures.js";

test("synthetic browser produces six registered layers and atomically promotes them", async ({ page }) => {
  const root = await mkdtemp(resolve(tmpdir(), "isometric-capture-e2e-"));
  const output = resolve(root, "bundle");
  const request = syntheticRequest();
  const writer = new BundleWriter(output, request);
  await writer.start();
  const upload = await startUploadServer(writer);
  try {
    await page.goto("http://127.0.0.1:4317/");
    await page.waitForFunction(() => window.ISOMETRIC_CAPTURE?.ready === true);
    const evidence = await page.evaluate(
      async ({ captureRequest, uploadTarget }) => {
        if (window.ISOMETRIC_CAPTURE === undefined) {
          throw new Error("capture runtime missing");
        }
        return window.ISOMETRIC_CAPTURE.capture(captureRequest, uploadTarget);
      },
      { captureRequest: request, uploadTarget: { token: upload.token, url: upload.url } },
    );
    expect(evidence.attributions).toEqual(["fixture:synthetic"]);
    expect(evidence.cameraFingerprint).toBe(cameraFingerprint(request));
    expect(evidence.coreCoverageBasisPoints).toBeGreaterThanOrEqual(9_950);
    await upload.close();
    await writer.finalize(evidence, async () => undefined);
    for (const filename of [
      "color.png",
      "whitebox.png",
      "depth.bin",
      "normal.png",
      "fixed-shadow.png",
      "coverage.png",
      "reference.manifest.json",
    ]) {
      expect((await stat(resolve(output, filename))).size).toBeGreaterThan(0);
    }
    expect((await readFile(resolve(output, "depth.bin"))).subarray(0, 8).toString()).toBe("ISOD32V1");
    for (const filename of [
      "color.png",
      "whitebox.png",
      "normal.png",
      "fixed-shadow.png",
      "coverage.png",
    ]) {
      const bytes = await readFile(resolve(output, filename));
      const dimensions = await page.evaluate(
        async (url) => {
          const image = new Image();
          image.src = url;
          await image.decode();
          return [image.naturalWidth, image.naturalHeight];
        },
        `data:image/png;base64,${bytes.toString("base64")}`,
      );
      expect(dimensions).toEqual([80, 80]);
    }
  } finally {
    await upload.close().catch(() => undefined);
    await writer.abort();
    await rm(root, { force: true, recursive: true });
  }
});

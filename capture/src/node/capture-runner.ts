import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";
import type { Browser } from "@playwright/test";
import type { CaptureEvidence, CaptureRequest } from "../contracts.js";
import { redactSecrets, validateCaptureRequest } from "../contracts.js";
import { BundleWriter } from "./bundle-writer.js";
import { GoogleRequestBudget, installGoogleRequestBudget } from "./request-budget.js";
import { startStaticRendererServer } from "./static-renderer-server.js";
import type { StaticRendererServer } from "./static-renderer-server.js";
import { startUploadServer } from "./upload-server.js";

const CAPTURE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const REPOSITORY_ROOT = resolve(CAPTURE_ROOT, "..");
const LIVE_CAPTURE_REQUEST_LIMIT = 1_000;

export async function readCaptureRequest(path: string): Promise<CaptureRequest> {
  const parsed: unknown = JSON.parse(await readFile(path, "utf8"));
  validateCaptureRequest(parsed);
  return parsed;
}

export async function validateRustBundle(stagingDirectory: string): Promise<void> {
  const environment = { ...process.env };
  delete environment.GOOGLE_MAP_TILES_API_KEY;
  const result = spawnSync(
    "cargo",
    ["run", "--quiet", "--locked", "--", "reference", "inspect", stagingDirectory],
    { cwd: REPOSITORY_ROOT, encoding: "utf8", env: environment },
  );
  if (result.status !== 0) {
    throw new Error(`Rust reference validation failed: ${result.stderr.trim()}`);
  }
}

export async function captureBundle(
  request: CaptureRequest,
  outputDirectory: string,
  apiKey: string,
): Promise<string> {
  if (request.provider !== "google-photorealistic-3d-tiles") {
    throw new Error("only Google reference requests may be promoted as durable bundles");
  }
  const writer = new BundleWriter(outputDirectory, request);
  const budget = new GoogleRequestBudget(LIVE_CAPTURE_REQUEST_LIMIT);
  await writer.start();
  let browser: Browser | undefined;
  let upload: Awaited<ReturnType<typeof startUploadServer>> | undefined;
  let rendererServer: StaticRendererServer | undefined;
  try {
    upload = await startUploadServer(writer);
    rendererServer = await startStaticRendererServer(resolve(CAPTURE_ROOT, "dist"));
    browser = await chromium.launch({
      args: ["--disable-dev-shm-usage", "--use-gl=swiftshader"],
      headless: true,
    });
    const context = await browser.newContext({ deviceScaleFactor: 1 });
    const observations = await installGoogleRequestBudget(context, budget);
    await context.addInitScript((googleApiKey: string) => {
      window.__CAPTURE_SECRETS__ = { googleApiKey };
    }, apiKey);
    const page = await context.newPage();
    await page.goto(rendererServer.url, { waitUntil: "domcontentloaded" });
    await page.waitForFunction(() => window.ISOMETRIC_CAPTURE?.ready === true);
    const evidence = await page.evaluate(
      async ({ captureRequest, uploadTarget }): Promise<CaptureEvidence> => {
        if (window.ISOMETRIC_CAPTURE === undefined) {
          throw new Error("capture runtime was not installed");
        }
        return window.ISOMETRIC_CAPTURE.capture(captureRequest, uploadTarget);
      },
      {
        captureRequest: request,
        uploadTarget: { token: upload.token, url: upload.url },
      },
    );
    await page.close();
    await context.close();
    await Promise.all(observations);
    await upload.close();
    if (budget.snapshot().blocked !== 0) {
      throw new Error("capture exhausted its Google request budget");
    }
    return await writer.finalize(evidence, validateRustBundle);
  } catch (error) {
    await writer.abort();
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(
      redactSecrets(
        `${message}; Google request telemetry: ${JSON.stringify(budget.snapshot())}`,
        [apiKey, upload?.token ?? ""],
      ),
    );
  } finally {
    await upload?.close();
    await browser?.close();
    await rendererServer?.close();
  }
}

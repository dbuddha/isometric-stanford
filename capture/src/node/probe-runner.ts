import { randomBytes } from "node:crypto";
import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";
import type { Browser, BrowserContext, Page } from "@playwright/test";
import type {
  CaptureRequest,
  ProbeCandidate,
  ProbeCandidateEvidence,
} from "../contracts.js";
import { redactSecrets, validateCaptureRequest } from "../contracts.js";
import { BundleWriter } from "./bundle-writer.js";
import { validateRustBundle } from "./capture-runner.js";
import { ProbeArtifactWriter } from "./probe-artifacts.js";
import type { ProbeJoinEvidence } from "./probe-artifacts.js";
import { GoogleRequestBudget, installGoogleRequestBudget } from "./request-budget.js";
import type { GoogleNetworkTelemetry } from "./request-budget.js";
import { startStaticRendererServer } from "./static-renderer-server.js";
import type { StaticRendererServer } from "./static-renderer-server.js";
import { startUploadServer } from "./upload-server.js";
import type { LayerSink, UploadServer } from "./upload-server.js";

const PROBE_SCHEMA = "isometric-reference-probe/v1";
const CAPTURE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");

interface CameraCandidateSpec {
  azimuthMillidegrees: number;
  elevationMillidegrees: number;
  id: string;
  label: string;
}

interface ProbeSpec {
  candidates: CameraCandidateSpec[];
  capture: CaptureRequest;
  requestLimit: number;
  schema: typeof PROBE_SCHEMA;
}

interface RuntimeMetrics {
  jsHeapSizeLimitBytes: number | null;
  jsHeapTotalBytes: number | null;
  jsHeapUsedBytes: number | null;
  nodeMaxRssBytes: number;
}

interface CandidateReport {
  artifacts: ProbeJoinEvidence;
  bundle: string;
  candidateId: string;
  evidence: ProbeCandidateEvidence;
  label: string;
  request: CaptureRequest;
}

export interface ProbeReport {
  candidates: CandidateReport[];
  network: GoogleNetworkTelemetry;
  runtime: RuntimeMetrics;
  schema: "isometric-reference-probe-report/v1";
}

function safeIdentifier(value: unknown): value is string {
  return typeof value === "string" && /^[a-z0-9-]{1,64}$/.test(value);
}

export async function readProbeSpec(path: string): Promise<ProbeSpec> {
  const parsed: unknown = JSON.parse(await readFile(path, "utf8"));
  if (typeof parsed !== "object" || parsed === null) {
    throw new Error("capture probe spec must be an object");
  }
  const value = parsed as Partial<ProbeSpec>;
  if (
    value.schema !== PROBE_SCHEMA ||
    !Number.isSafeInteger(value.requestLimit) ||
    Number(value.requestLimit) < 1 ||
    Number(value.requestLimit) > 500 ||
    !Array.isArray(value.candidates) ||
    value.candidates.length < 1 ||
    value.candidates.length > 8 ||
    value.capture === undefined
  ) {
    throw new Error("capture probe identity, budget, or candidate count is invalid");
  }
  validateCaptureRequest(value.capture);
  if (
    value.capture.provider !== "google-photorealistic-3d-tiles" ||
    value.capture.tile.coreWidthPx !== 1_024 ||
    value.capture.tile.coreHeightPx !== 1_024
  ) {
    throw new Error("capture probe requires one Google 1024 by 1024 registered core");
  }
  const ids = new Set<string>();
  for (const candidate of value.candidates) {
    if (
      !safeIdentifier(candidate.id) ||
      ids.has(candidate.id) ||
      typeof candidate.label !== "string" ||
      candidate.label.length < 1 ||
      candidate.label.length > 128 ||
      !Number.isSafeInteger(candidate.azimuthMillidegrees) ||
      candidate.azimuthMillidegrees < 0 ||
      candidate.azimuthMillidegrees > 359_999 ||
      !Number.isSafeInteger(candidate.elevationMillidegrees) ||
      candidate.elevationMillidegrees < 1_000 ||
      candidate.elevationMillidegrees > 89_999
    ) {
      throw new Error("capture probe camera candidate is invalid");
    }
    ids.add(candidate.id);
  }
  return value as ProbeSpec;
}

function requestForCandidate(base: CaptureRequest, candidate: CameraCandidateSpec): CaptureRequest {
  const request = structuredClone(base);
  request.bundleId = `${base.bundleId}-${candidate.id}`;
  request.camera.azimuthMillidegrees = candidate.azimuthMillidegrees;
  request.camera.elevationMillidegrees = candidate.elevationMillidegrees;
  validateCaptureRequest(request);
  return request;
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function reportHtml(report: ProbeReport): string {
  const cards = report.candidates
    .map((candidate) => {
      const camera = candidate.request.camera;
      const attribution = candidate.evidence.attributions.map(escapeHtml).join(" | ");
      return `<article>
  <h2>${escapeHtml(candidate.label)}</h2>
  <p>${camera.azimuthMillidegrees / 1_000} degrees azimuth, ${camera.elevationMillidegrees / 1_000} degrees elevation, ${camera.orthographicWidthMm / 1_000} meter span</p>
  <div class="image-frame"><img src="candidates/${candidate.candidateId}/core.png" alt="${escapeHtml(candidate.label)} Hoover Tower Google Maps render"></div>
  <p>Coverage ${(candidate.evidence.coreCoverageBasisPoints / 100).toFixed(2)}%, ${candidate.evidence.visibleTiles} visible tiles, ${(candidate.evidence.diagnostics.cachedBytes / 1_048_576).toFixed(1)} MiB renderer cache</p>
  <details><summary>Two-cell join</summary><img src="candidates/${candidate.candidateId}/joined-top.png" alt="Two exact adjacent cells for ${escapeHtml(candidate.label)}"><p>Mismatch pixels: ${candidate.artifacts.mismatchPixels}</p></details>
  <p class="attribution">${attribution}</p>
</article>`;
    })
    .join("\n");
  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Hoover camera probe</title>
<style>body{font-family:system-ui,sans-serif;margin:0;background:#141414;color:#f4f1e8}main{max-width:1180px;margin:auto;padding:24px}article{background:#222;border:1px solid #444;border-radius:12px;margin:24px 0;padding:18px}.image-frame{position:relative;max-width:1024px}.image-frame:after{content:"";position:absolute;inset:0;background:linear-gradient(90deg,transparent calc(50% - .5px),#ff3b30 calc(50% - .5px),#ff3b30 calc(50% + .5px),transparent calc(50% + .5px)),linear-gradient(0deg,transparent calc(50% - .5px),#ff3b30 calc(50% - .5px),#ff3b30 calc(50% + .5px),transparent calc(50% + .5px));pointer-events:none}img{display:block;max-width:100%;height:auto;image-rendering:auto}.attribution{font-size:12px;color:#ddd}pre{white-space:pre-wrap;overflow-wrap:anywhere;background:#090909;padding:16px;border-radius:8px}</style></head>
<body><main><h1>Hoover Tower orthographic camera probe</h1><p>One Google root session, ${report.network.attempted}/${report.network.requestLimit} total Google requests, ${report.network.billableRootRequests} billable root request.</p>${cards}<h2>Machine-readable evidence</h2><pre>${escapeHtml(JSON.stringify(report, null, 2))}</pre></main></body></html>\n`;
}

async function assertAbsent(path: string): Promise<void> {
  try {
    await stat(path);
    throw new Error("probe output already exists; evidence is immutable");
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
      throw error;
    }
  }
}

async function pageMemory(page: Page): Promise<Omit<RuntimeMetrics, "nodeMaxRssBytes">> {
  return await page.evaluate(() => {
    const memory = (
      performance as Performance & {
        memory?: { jsHeapSizeLimit: number; totalJSHeapSize: number; usedJSHeapSize: number };
      }
    ).memory;
    return {
      jsHeapSizeLimitBytes: memory?.jsHeapSizeLimit ?? null,
      jsHeapTotalBytes: memory?.totalJSHeapSize ?? null,
      jsHeapUsedBytes: memory?.usedJSHeapSize ?? null,
    };
  });
}

export async function runProbe(
  spec: ProbeSpec,
  outputDirectory: string,
  apiKey: string,
): Promise<string> {
  if (apiKey.length < 6) {
    throw new Error("Google tile credential is missing");
  }
  const output = resolve(outputDirectory);
  await mkdir(dirname(output), { mode: 0o700, recursive: true });
  await assertAbsent(output);
  const staging = resolve(dirname(output), `.probe-${randomBytes(8).toString("hex")}`);
  await mkdir(staging, { mode: 0o700, recursive: false });
  const budget = new GoogleRequestBudget(spec.requestLimit);
  const writers: BundleWriter[] = [];
  const artifactWriters: ProbeArtifactWriter[] = [];
  const uploads: UploadServer[] = [];
  const requests: CaptureRequest[] = [];
  let browser: Browser | undefined;
  let context: BrowserContext | undefined;
  let page: Page | undefined;
  let rendererServer: StaticRendererServer | undefined;
  try {
    const browserCandidates: ProbeCandidate[] = [];
    for (const candidate of spec.candidates) {
      const request = requestForCandidate(spec.capture, candidate);
      requests.push(request);
      const writer = new BundleWriter(resolve(staging, "bundles", candidate.id), request);
      const artifactWriter = new ProbeArtifactWriter(
        resolve(staging, "candidates", candidate.id),
        request,
      );
      await writer.start();
      const sink: LayerSink = {
        async accept(name, pixels, width, height, pixelFormat): Promise<void> {
          await artifactWriter.accept(name, pixels, width, height, pixelFormat);
          await writer.accept(name, pixels, width, height, pixelFormat);
        },
      };
      const upload = await startUploadServer(sink);
      writers.push(writer);
      artifactWriters.push(artifactWriter);
      uploads.push(upload);
      browserCandidates.push({
        candidateId: candidate.id,
        request,
        upload: { token: upload.token, url: upload.url },
      });
    }
    rendererServer = await startStaticRendererServer(resolve(CAPTURE_ROOT, "dist"));
    browser = await chromium.launch({
      args: ["--disable-dev-shm-usage", "--enable-webgl", "--ignore-gpu-blocklist", "--use-gl=angle"],
      headless: true,
    });
    context = await browser.newContext({ deviceScaleFactor: 1 });
    const observations = await installGoogleRequestBudget(context, budget);
    await context.addInitScript((googleApiKey: string) => {
      window.__CAPTURE_SECRETS__ = { googleApiKey };
    }, apiKey);
    page = await context.newPage();
    await page.goto(rendererServer.url, { waitUntil: "domcontentloaded" });
    await page.waitForFunction(() => window.ISOMETRIC_CAPTURE?.ready === true);
    const evidence = await page.evaluate(
      async (candidates): Promise<ProbeCandidateEvidence[]> => {
        if (window.ISOMETRIC_CAPTURE === undefined) {
          throw new Error("capture probe runtime was not installed");
        }
        return await window.ISOMETRIC_CAPTURE.probe(candidates);
      },
      browserCandidates,
    );
    const browserMemory = await pageMemory(page);
    await page.close();
    page = undefined;
    await context.close();
    context = undefined;
    await Promise.all(observations);
    await Promise.all(uploads.map(async (upload) => upload.close()));
    if (budget.snapshot().blocked !== 0) {
      throw new Error("capture probe exhausted its Google request budget");
    }
    const reports: CandidateReport[] = [];
    for (let index = 0; index < spec.candidates.length; index += 1) {
      const candidate = spec.candidates[index];
      const writer = writers[index];
      const artifactWriter = artifactWriters[index];
      const candidateEvidence = evidence[index];
      const request = requests[index];
      if (
        candidate === undefined ||
        writer === undefined ||
        artifactWriter === undefined ||
        candidateEvidence === undefined ||
        request === undefined ||
        candidateEvidence.candidateId !== candidate.id
      ) {
        throw new Error("capture probe evidence ordering is incomplete");
      }
      await writer.finalize(candidateEvidence, validateRustBundle);
      reports.push({
        artifacts: artifactWriter.finalize(),
        bundle: relative(staging, resolve(staging, "bundles", candidate.id)),
        candidateId: candidate.id,
        evidence: candidateEvidence,
        label: candidate.label,
        request,
      });
    }
    const report: ProbeReport = {
      candidates: reports,
      network: budget.snapshot(),
      runtime: {
        ...browserMemory,
        nodeMaxRssBytes: process.resourceUsage().maxRSS * 1_024,
      },
      schema: "isometric-reference-probe-report/v1",
    };
    await writeFile(resolve(staging, "report.json"), `${JSON.stringify(report, null, 2)}\n`, {
      flag: "wx",
      mode: 0o600,
    });
    await writeFile(resolve(staging, "index.html"), reportHtml(report), {
      flag: "wx",
      mode: 0o600,
    });
    await rename(staging, output);
    return output;
  } catch (error) {
    await Promise.all(writers.map(async (writer) => writer.abort().catch(() => undefined)));
    const message = error instanceof Error ? error.message : String(error);
    const network = budget.snapshot();
    throw new Error(
      redactSecrets(
        `${message}; Google request telemetry: ${JSON.stringify(network)}`,
        [apiKey, ...uploads.map((upload) => upload.token)],
      ),
    );
  } finally {
    await Promise.all(uploads.map(async (upload) => upload.close().catch(() => undefined)));
    await page?.close().catch(() => undefined);
    await context?.close().catch(() => undefined);
    await browser?.close().catch(() => undefined);
    await rendererServer?.close().catch(() => undefined);
    await rm(staging, { force: true, recursive: true });
  }
}

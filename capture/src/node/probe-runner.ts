import { randomBytes } from "node:crypto";
import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { totalmem } from "node:os";
import type {
  BrowserMemoryMetrics,
  CaptureRequest,
  GoogleNetworkTelemetry,
  ProbeBrowserResult,
  ProbeCandidate,
  ProbeCandidateEvidence,
} from "../contracts.js";
import {
  redactSecrets,
  totalHeight,
  totalWidth,
  validateCaptureRequest,
} from "../contracts.js";
import type { ProbeJoinEvidence } from "./probe-artifacts.js";
import { startProbeIngest } from "./probe-ingest-client.js";
import type { ProbeIngestClient } from "./probe-ingest-client.js";
import { ProcessMemorySampler } from "./process-memory.js";
import type { ProcessMemoryReport } from "./process-memory.js";
import { runDirectChromiumProbe } from "./headless-probe.js";
import { startProbeCoordinator } from "./probe-coordinator.js";
import type { ProbeCoordinator } from "./probe-coordinator.js";
import { startStaticRendererServer } from "./static-renderer-server.js";
import type { StaticRendererServer } from "./static-renderer-server.js";
import {
  captureWorkerEnvelopeBytes,
  deriveCaptureWorkerCount,
} from "./capture-memory-policy.js";

const PROBE_SCHEMA = "isometric-reference-probe/v1";
const CAPTURE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");

interface CameraCandidateSpec {
  azimuthMillidegrees: number;
  coreHeightPx?: number;
  coreWidthPx?: number;
  elevationMillidegrees: number;
  guardPx?: number;
  id: string;
  label: string;
  maxScreenSpaceErrorPx?: number;
  millimetersPerPixel?: number;
}

interface ProbeSpec {
  candidates: CameraCandidateSpec[];
  capture: CaptureRequest;
  requestLimit: number;
  schema: typeof PROBE_SCHEMA;
  workerEnvelopeMiB?: number;
}

interface RuntimeMetrics extends BrowserMemoryMetrics {
  hostTotalMemoryBytes: number;
  nodeArrayBuffersBytes: number;
  nodeExternalBytes: number;
  nodeHeapTotalBytes: number;
  nodeHeapUsedBytes: number;
  ingestWorkerMaxRssBytes: number;
  jsHeapSizeLimitBytes: number | null;
  jsHeapTotalBytes: number | null;
  jsHeapUsedBytes: number | null;
  nodeMaxRssBytes: number;
  processTree: ProcessMemoryReport;
  recommendedParallelWorkers: number;
  workerEnvelopeBytes: number;
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
    Number(value.requestLimit) > 1_000 ||
    (value.workerEnvelopeMiB !== undefined &&
      (!Number.isSafeInteger(value.workerEnvelopeMiB) ||
        value.workerEnvelopeMiB < 512 ||
        value.workerEnvelopeMiB > 4_096)) ||
    !Array.isArray(value.candidates) ||
    value.candidates.length < 1 ||
    value.candidates.length > 8 ||
    value.capture === undefined
  ) {
    throw new Error("capture probe identity, budget, or candidate count is invalid");
  }
  validateCaptureRequest(value.capture);
  const registeredGrid =
    (value.capture.tile.coreWidthPx === 1_024 &&
      value.capture.tile.coreHeightPx === 1_024 &&
      value.capture.tile.guardPx === 128) ||
    (value.capture.tile.coreWidthPx === 2_048 &&
      value.capture.tile.coreHeightPx === 2_048 &&
      value.capture.tile.guardPx === 256);
  if (
    value.capture.provider !== "google-photorealistic-3d-tiles" ||
    !registeredGrid
  ) {
    throw new Error("capture probe requires an approved Google registered grid");
  }
  const ids = new Set<string>();
  const physicalWidthMm = totalWidth(value.capture) * value.capture.tile.millimetersPerPixel;
  const physicalHeightMm = totalHeight(value.capture) * value.capture.tile.millimetersPerPixel;
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
      candidate.elevationMillidegrees > 89_999 ||
      (candidate.coreWidthPx !== undefined &&
        (!Number.isSafeInteger(candidate.coreWidthPx) ||
          candidate.coreWidthPx < 1 ||
          candidate.coreWidthPx > 3_584)) ||
      (candidate.coreHeightPx !== undefined &&
        (!Number.isSafeInteger(candidate.coreHeightPx) ||
          candidate.coreHeightPx < 1 ||
          candidate.coreHeightPx > 3_584)) ||
      (candidate.guardPx !== undefined &&
        (!Number.isSafeInteger(candidate.guardPx) ||
          candidate.guardPx < 1 ||
          candidate.guardPx > 1_024)) ||
      (candidate.millimetersPerPixel !== undefined &&
        (!Number.isSafeInteger(candidate.millimetersPerPixel) ||
          candidate.millimetersPerPixel < 1 ||
          candidate.millimetersPerPixel > 100_000)) ||
      (candidate.maxScreenSpaceErrorPx !== undefined &&
        (!Number.isSafeInteger(candidate.maxScreenSpaceErrorPx) ||
          candidate.maxScreenSpaceErrorPx < 1 ||
          candidate.maxScreenSpaceErrorPx > 64))
    ) {
      throw new Error("capture probe camera candidate is invalid");
    }
    const request = requestForCandidate(value.capture, candidate);
    if (
      totalWidth(request) * request.tile.millimetersPerPixel !== physicalWidthMm ||
      totalHeight(request) * request.tile.millimetersPerPixel !== physicalHeightMm
    ) {
      throw new Error("capture probe candidates must preserve the physical comparison footprint");
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
  request.tile.coreWidthPx = candidate.coreWidthPx ?? request.tile.coreWidthPx;
  request.tile.coreHeightPx = candidate.coreHeightPx ?? request.tile.coreHeightPx;
  request.tile.guardPx = candidate.guardPx ?? request.tile.guardPx;
  request.tile.millimetersPerPixel =
    candidate.millimetersPerPixel ?? request.tile.millimetersPerPixel;
  request.quality.maxScreenSpaceErrorPx =
    candidate.maxScreenSpaceErrorPx ?? request.quality.maxScreenSpaceErrorPx;
  request.camera.orthographicWidthMm = totalWidth(request) * request.tile.millimetersPerPixel;
  request.camera.orthographicHeightMm = totalHeight(request) * request.tile.millimetersPerPixel;
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
  <p>${camera.azimuthMillidegrees / 1_000} degrees azimuth, ${camera.elevationMillidegrees / 1_000} degrees elevation, ${camera.orthographicWidthMm / 1_000} meter span, ${candidate.request.tile.millimetersPerPixel} mm/px, ${candidate.request.quality.maxScreenSpaceErrorPx} px screen-space error</p>
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
  const requests: CaptureRequest[] = [];
  const uploadSecrets: string[] = [];
  const memorySampler = new ProcessMemorySampler();
  let ingest: ProbeIngestClient | undefined;
  let coordinator: ProbeCoordinator | undefined;
  let rendererServer: StaticRendererServer | undefined;
  memorySampler.start();
  try {
    for (const candidate of spec.candidates) {
      const request = requestForCandidate(spec.capture, candidate);
      requests.push(request);
    }
    ingest = await startProbeIngest(
      staging,
      spec.candidates.map((candidate, index) => ({
        candidateId: candidate.id,
        request: requests[index] as CaptureRequest,
      })),
    );
    uploadSecrets.push(...ingest.targets.map((target) => target.upload.token));
    const targets = new Map(ingest.targets.map((target) => [target.candidateId, target.upload]));
    const browserCandidates: ProbeCandidate[] = spec.candidates.map((candidate, index) => {
      const request = requests[index];
      const upload = targets.get(candidate.id);
      if (request === undefined || upload === undefined) {
        throw new Error("probe ingest worker returned incomplete upload targets");
      }
      return { candidateId: candidate.id, request, upload };
    });
    rendererServer = await startStaticRendererServer(resolve(CAPTURE_ROOT, "dist"));
    coordinator = await startProbeCoordinator({
      apiKey,
      candidates: browserCandidates,
      requestLimit: spec.requestLimit,
    });
    uploadSecrets.push(coordinator.token);
    memorySampler.setStage("capture-and-encode");
    const execution = await runDirectChromiumProbe(
      rendererServer.url,
      coordinator,
      spec.capture.readiness.timeoutMs * spec.candidates.length + 60_000,
    );
    const browserResult: ProbeBrowserResult = execution.probe;
    const evidence = browserResult.candidates;
    if (browserResult.network.blocked !== 0) {
      throw new Error("capture probe exhausted its Google request budget");
    }
    memorySampler.setStage("validate-bundles");
    const ingested = await ingest.finalize(evidence);
    const resultByCandidate = new Map(
      ingested.results.map((result) => [result.candidateId, result.artifacts]),
    );
    const reports: CandidateReport[] = [];
    for (let index = 0; index < spec.candidates.length; index += 1) {
      const candidate = spec.candidates[index];
      const candidateEvidence = evidence[index];
      const request = requests[index];
      const artifacts = candidate === undefined ? undefined : resultByCandidate.get(candidate.id);
      if (
        candidate === undefined ||
        artifacts === undefined ||
        candidateEvidence === undefined ||
        request === undefined ||
        candidateEvidence.candidateId !== candidate.id
      ) {
        throw new Error("capture probe evidence ordering is incomplete");
      }
      reports.push({
        artifacts,
        bundle: relative(staging, resolve(staging, "bundles", candidate.id)),
        candidateId: candidate.id,
        evidence: candidateEvidence,
        label: candidate.label,
        request,
      });
    }
    const nodeMemory = process.memoryUsage();
    memorySampler.setStage("write-report");
    const processTree = memorySampler.stop();
    const largestDimension = Math.max(
      ...requests.flatMap((request) => [totalWidth(request), totalHeight(request)]),
    );
    const measuredMinimumBytes = (spec.workerEnvelopeMiB ?? 0) * 1_024 * 1_024;
    const report: ProbeReport = {
      candidates: reports,
      network: browserResult.network,
      runtime: {
        ...execution.browserMemory,
        hostTotalMemoryBytes: totalmem(),
        ingestWorkerMaxRssBytes: ingested.workerMaxRssBytes,
        nodeArrayBuffersBytes: nodeMemory.arrayBuffers,
        nodeExternalBytes: nodeMemory.external,
        nodeHeapTotalBytes: nodeMemory.heapTotal,
        nodeHeapUsedBytes: nodeMemory.heapUsed,
        nodeMaxRssBytes: process.resourceUsage().maxRSS * 1_024,
        processTree,
        recommendedParallelWorkers: deriveCaptureWorkerCount(
          totalmem(),
          largestDimension,
          measuredMinimumBytes,
        ),
        workerEnvelopeBytes: captureWorkerEnvelopeBytes(
          largestDimension,
          measuredMinimumBytes,
        ),
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
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(
      redactSecrets(message, [apiKey, ...uploadSecrets]),
    );
  } finally {
    memorySampler.stop();
    await ingest?.abort().catch(() => undefined);
    await coordinator?.close().catch(() => undefined);
    await rendererServer?.close().catch(() => undefined);
    await rm(staging, { force: true, recursive: true });
  }
}

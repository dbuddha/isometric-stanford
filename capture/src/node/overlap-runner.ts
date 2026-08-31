import { createHash, randomBytes } from "node:crypto";
import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import { totalmem } from "node:os";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type {
  BrowserMemoryMetrics,
  CaptureRequest,
  GoogleNetworkTelemetry,
  ProbeBrowserResult,
  ProbeCandidate,
  ProbeCandidateEvidence,
} from "../contracts.js";
import { redactSecrets, totalHeight, totalWidth } from "../contracts.js";
import {
  captureWorkerEnvelopeBytes,
  deriveCaptureWorkerCount,
} from "./capture-memory-policy.js";
import { runDirectChromiumProbe } from "./headless-probe.js";
import type { ProbeJoinEvidence } from "./probe-artifacts.js";
import { startProbeCoordinator } from "./probe-coordinator.js";
import type { ProbeCoordinator } from "./probe-coordinator.js";
import { startProbeIngest } from "./probe-ingest-client.js";
import type { ProbeIngestClient } from "./probe-ingest-client.js";
import { ProcessMemorySampler } from "./process-memory.js";
import type { ProcessMemoryReport } from "./process-memory.js";
import {
  deriveRegisteredOverlapRequests,
  type RegisteredGridReport,
} from "./registered-grid.js";
import { compareRustOverlap } from "./rust-reference.js";
import { startStaticRendererServer } from "./static-renderer-server.js";
import type { StaticRendererServer } from "./static-renderer-server.js";

const OVERLAP_SPEC_SCHEMA = "isometric-reference-overlap-probe/v1";
const CAPTURE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");

interface OverlapSpec {
  capture: CaptureRequest;
  requestLimit: number;
  schema: typeof OVERLAP_SPEC_SCHEMA;
}

interface RuntimeMetrics extends BrowserMemoryMetrics {
  hostTotalMemoryBytes: number;
  ingestWorkerMaxRssBytes: number;
  nodeArrayBuffersBytes: number;
  nodeExternalBytes: number;
  nodeHeapTotalBytes: number;
  nodeHeapUsedBytes: number;
  nodeMaxRssBytes: number;
  processTree: ProcessMemoryReport;
  recommendedParallelWorkers: number;
  workerEnvelopeBytes: number;
}

interface OverlapCandidateReport {
  artifacts: ProbeJoinEvidence;
  bundle: string;
  candidateId: "left" | "monolithic" | "right";
  evidence: ProbeCandidateEvidence;
  request: CaptureRequest;
}

interface CameraRegistrationReport {
  fixedWorldMatrix: true;
  horizontalPixelsPerMeter: number;
  maximumScaleErrorPixelsPerMeter: number;
  projectionCenterX: { left: number; monolithic: number; right: number };
  verticalPixelsPerMeter: number;
  worldMatrixSha256: string;
}

interface RustComparisonReport {
  failure_classifications: string[];
  passed: boolean;
  schema: "isometric-reference-overlap-report/v1";
  [key: string]: unknown;
}

export interface OverlapExperimentReport {
  cameraRegistration: CameraRegistrationReport;
  candidates: OverlapCandidateReport[];
  comparison: RustComparisonReport;
  grid: RegisteredGridReport;
  network: GoogleNetworkTelemetry;
  runtime: RuntimeMetrics;
  schema: "isometric-reference-overlap-experiment/v1";
}

function assertFixedCameraRegistration(
  candidates: ProbeCandidateEvidence[],
  requests: ReturnType<typeof deriveRegisteredOverlapRequests>,
): CameraRegistrationReport {
  const byId = new Map(candidates.map((candidate) => [candidate.candidateId, candidate]));
  const monolithic = byId.get("monolithic");
  const left = byId.get("left");
  const right = byId.get("right");
  if (monolithic === undefined || left === undefined || right === undefined) {
    throw new Error("registered overlap camera evidence is incomplete");
  }
  for (const candidate of [left, right]) {
    if (
      candidate.cameraWorldMatrix.length !== monolithic.cameraWorldMatrix.length ||
      candidate.cameraWorldMatrix.some(
        (value, index) => value !== monolithic.cameraWorldMatrix[index],
      )
    ) {
      throw new Error("registered overlap moved the fixed camera world matrix");
    }
  }
  const scales = [
    [monolithic, requests.monolithic],
    [left, requests.left],
    [right, requests.right],
  ] as const;
  const horizontal = scales.map(
    ([candidate, request]) =>
      Math.abs(candidate.projectionMatrix[0] ?? Number.NaN) * totalWidth(request) / 2,
  );
  const vertical = scales.map(
    ([candidate, request]) =>
      Math.abs(candidate.projectionMatrix[5] ?? Number.NaN) * totalHeight(request) / 2,
  );
  const expected = 1_000 / requests.monolithic.tile.millimetersPerPixel;
  const errors = [...horizontal, ...vertical].map((value) => Math.abs(value - expected));
  const maximumScaleErrorPixelsPerMeter = Math.max(...errors);
  if (!Number.isFinite(maximumScaleErrorPixelsPerMeter) || maximumScaleErrorPixelsPerMeter > 1e-9) {
    throw new Error("registered overlap projection changed its source pixel scale");
  }
  return {
    fixedWorldMatrix: true,
    horizontalPixelsPerMeter: horizontal[0] ?? Number.NaN,
    maximumScaleErrorPixelsPerMeter,
    projectionCenterX: {
      left: left.projectionMatrix[12] ?? Number.NaN,
      monolithic: monolithic.projectionMatrix[12] ?? Number.NaN,
      right: right.projectionMatrix[12] ?? Number.NaN,
    },
    verticalPixelsPerMeter: vertical[0] ?? Number.NaN,
    worldMatrixSha256: createHash("sha256")
      .update(JSON.stringify(monolithic.cameraWorldMatrix))
      .digest("hex"),
  };
}

export async function readOverlapSpec(path: string): Promise<OverlapSpec> {
  const parsed: unknown = JSON.parse(await readFile(path, "utf8"));
  if (typeof parsed !== "object" || parsed === null) {
    throw new Error("registered overlap spec must be an object");
  }
  const value = parsed as Partial<OverlapSpec>;
  if (
    value.schema !== OVERLAP_SPEC_SCHEMA ||
    value.capture === undefined ||
    !Number.isSafeInteger(value.requestLimit) ||
    value.requestLimit !== 450
  ) {
    throw new Error("registered overlap spec identity or request ceiling is invalid");
  }
  deriveRegisteredOverlapRequests(value.capture);
  return value as OverlapSpec;
}

async function assertAbsent(path: string): Promise<void> {
  try {
    await stat(path);
    throw new Error("overlap output already exists; evidence is immutable");
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
      throw error;
    }
  }
}

function comparisonRequest(staging: string, requests: ReturnType<typeof deriveRegisteredOverlapRequests>) {
  return {
    schema: "isometric-reference-overlap-comparison/v1",
    left: {
      directory: "raw/left",
      width_px: totalWidth(requests.left),
      height_px: totalHeight(requests.left),
    },
    right: {
      directory: "raw/right",
      width_px: totalWidth(requests.right),
      height_px: totalHeight(requests.right),
    },
    monolithic: {
      directory: "raw/monolithic",
      width_px: totalWidth(requests.monolithic),
      height_px: totalHeight(requests.monolithic),
    },
    independent_core_width_px: requests.left.tile.coreWidthPx,
    core_height_px: requests.left.tile.coreHeightPx,
    guard_px: requests.left.tile.guardPx,
    output_directory: "comparison",
    requestPath: resolve(staging, "comparison-request.json"),
  };
}

function parseComparison(value: unknown): RustComparisonReport {
  if (
    typeof value !== "object" ||
    value === null ||
    (value as Partial<RustComparisonReport>).schema !==
      "isometric-reference-overlap-report/v1" ||
    typeof (value as Partial<RustComparisonReport>).passed !== "boolean" ||
    !Array.isArray((value as Partial<RustComparisonReport>).failure_classifications)
  ) {
    throw new Error("Rust overlap comparator returned an invalid report");
  }
  return value as RustComparisonReport;
}

export async function runOverlapExperiment(
  spec: OverlapSpec,
  outputDirectory: string,
  apiKey: string,
): Promise<string> {
  if (apiKey.length < 6) {
    throw new Error("Google tile credential is missing");
  }
  const output = resolve(outputDirectory);
  await mkdir(dirname(output), { mode: 0o700, recursive: true });
  await assertAbsent(output);
  const staging = resolve(dirname(output), `.overlap-${randomBytes(8).toString("hex")}`);
  await mkdir(staging, { mode: 0o700, recursive: false });
  const requests = deriveRegisteredOverlapRequests(spec.capture);
  const ordered = [
    { candidateId: "monolithic" as const, request: requests.monolithic },
    { candidateId: "left" as const, request: requests.left },
    { candidateId: "right" as const, request: requests.right },
  ];
  const uploadSecrets: string[] = [];
  const memorySampler = new ProcessMemorySampler();
  let ingest: ProbeIngestClient | undefined;
  let coordinator: ProbeCoordinator | undefined;
  let rendererServer: StaticRendererServer | undefined;
  memorySampler.start();
  try {
    ingest = await startProbeIngest(
      staging,
      ordered.map(({ candidateId, request }) => ({ candidateId, request })),
      true,
    );
    uploadSecrets.push(...ingest.targets.map((target) => target.upload.token));
    const targets = new Map(ingest.targets.map((target) => [target.candidateId, target.upload]));
    const candidates: ProbeCandidate[] = ordered.map(({ candidateId, request }) => {
      const upload = targets.get(candidateId);
      if (upload === undefined) {
        throw new Error("overlap ingest worker returned incomplete upload targets");
      }
      return { candidateId, request, upload };
    });
    rendererServer = await startStaticRendererServer(resolve(CAPTURE_ROOT, "dist"));
    coordinator = await startProbeCoordinator({
      apiKey,
      candidates,
      requestLimit: spec.requestLimit,
    });
    uploadSecrets.push(coordinator.token);
    memorySampler.setStage("capture-and-encode");
    const execution = await runDirectChromiumProbe(
      rendererServer.url,
      coordinator,
      spec.capture.readiness.timeoutMs * candidates.length + 60_000,
    );
    const browserResult: ProbeBrowserResult = execution.probe;
    if (
      browserResult.network.blocked !== 0 ||
      browserResult.network.rootTilesetRequests !== 1 ||
      browserResult.network.attempted > spec.requestLimit
    ) {
      throw new Error("registered overlap violated its one-session Google request budget");
    }
    const cameraRegistration = assertFixedCameraRegistration(
      browserResult.candidates,
      requests,
    );
    memorySampler.setStage("validate-bundles");
    const ingested = await ingest.finalize(browserResult.candidates);
    const artifacts = new Map(
      ingested.results.map((result) => [result.candidateId, result.artifacts]),
    );
    const reports: OverlapCandidateReport[] = ordered.map(({ candidateId, request }, index) => {
      const evidence = browserResult.candidates[index];
      const candidateArtifacts = artifacts.get(candidateId);
      if (
        evidence === undefined ||
        evidence.candidateId !== candidateId ||
        candidateArtifacts === undefined
      ) {
        throw new Error("registered overlap candidate evidence is incomplete");
      }
      return {
        artifacts: candidateArtifacts,
        bundle: relative(staging, resolve(staging, "bundles", candidateId)),
        candidateId,
        evidence,
        request,
      };
    });

    const comparison = comparisonRequest(staging, requests);
    const { requestPath, ...portableComparison } = comparison;
    await writeFile(requestPath, `${JSON.stringify(portableComparison, null, 2)}\n`, {
      flag: "wx",
      mode: 0o600,
    });
    memorySampler.setStage("compare-overlap");
    compareRustOverlap(requestPath);
    const rustComparison = parseComparison(
      JSON.parse(await readFile(resolve(staging, "comparison/comparison.json"), "utf8")),
    );

    const nodeMemory = process.memoryUsage();
    memorySampler.setStage("write-report");
    const processTree = memorySampler.stop();
    const largestDimension = Math.max(
      totalWidth(requests.monolithic),
      totalHeight(requests.monolithic),
    );
    const report: OverlapExperimentReport = {
      cameraRegistration,
      candidates: reports,
      comparison: rustComparison,
      grid: requests.grid,
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
        recommendedParallelWorkers: deriveCaptureWorkerCount(totalmem(), largestDimension),
        workerEnvelopeBytes: captureWorkerEnvelopeBytes(largestDimension),
      },
      schema: "isometric-reference-overlap-experiment/v1",
    };
    await writeFile(resolve(staging, "overlap-report.json"), `${JSON.stringify(report, null, 2)}\n`, {
      flag: "wx",
      mode: 0o600,
    });
    await rename(staging, output);
    return output;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(redactSecrets(message, [apiKey, ...uploadSecrets]));
  } finally {
    memorySampler.stop();
    await ingest?.abort().catch(() => undefined);
    await coordinator?.close().catch(() => undefined);
    await rendererServer?.close().catch(() => undefined);
    await rm(staging, { force: true, recursive: true });
  }
}

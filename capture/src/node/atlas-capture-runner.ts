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
  deriveRegisteredAtlasRequests,
  type RegisteredAtlasGridReport,
} from "./registered-grid.js";
import { compileRustAtlas, inspectRustAtlas } from "./rust-reference.js";
import { startStaticRendererServer } from "./static-renderer-server.js";
import type { StaticRendererServer } from "./static-renderer-server.js";

const ATLAS_CAPTURE_SPEC_SCHEMA = "isometric-reference-atlas-capture/v1";
const ATLAS_CAPTURE_REPORT_SCHEMA = "isometric-reference-atlas-capture-report/v1";
const ATLAS_MANIFEST_FILENAME = "reference-atlas.manifest.json";
const CAPTURE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const THREE_HOURS_MILLISECONDS = 3 * 60 * 60 * 1_000;
const SHA256_PATTERN = /^[0-9a-f]{64}$/;

export interface AtlasCaptureSpec {
  atlasId: string;
  capture: CaptureRequest;
  requestLimit: number;
  schema: typeof ATLAS_CAPTURE_SPEC_SCHEMA;
  workerEnvelopeMiB: number;
}

interface AtlasCaptureCandidateReport {
  artifacts: ProbeJoinEvidence;
  bundle: string;
  candidateId: string;
  evidence: ProbeCandidateEvidence;
  request: CaptureRequest;
}

export interface AtlasCameraRegistrationReport {
  fixedWorldMatrix: true;
  horizontalPixelsPerMeter: number;
  maximumProjectionCenterErrorPixels: number;
  maximumScaleErrorPixelsPerMeter: number;
  projectionCentersPixels: Record<string, { x: number; y: number }>;
  verticalPixelsPerMeter: number;
  worldMatrixSha256: string;
}

interface AtlasSessionReport {
  expiresAt: string;
  rootTilesetSha256: string;
  sessionId: string;
  startedAt: string;
}

interface AtlasArtifactReport {
  compileEvidence: string;
  directory: string;
  inspectEvidence: string;
  manifestSha256: string;
  request: string;
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

export interface AtlasCaptureReport {
  atlas: AtlasArtifactReport;
  cameraRegistration: AtlasCameraRegistrationReport;
  candidates: AtlasCaptureCandidateReport[];
  grid: RegisteredAtlasGridReport;
  network: GoogleNetworkTelemetry;
  runtime: RuntimeMetrics;
  schema: typeof ATLAS_CAPTURE_REPORT_SCHEMA;
  session: AtlasSessionReport;
}

function isSafeIdentifier(value: unknown): value is string {
  return typeof value === "string" && /^[a-z0-9-]{1,64}$/.test(value);
}

async function assertAbsent(path: string): Promise<void> {
  try {
    await stat(path);
    throw new Error("atlas capture output already exists; evidence is immutable");
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
      throw error;
    }
  }
}

function sha256(bytes: string | Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function assertNetworkSession(network: GoogleNetworkTelemetry, requestLimit: number): string {
  const formatCount = Object.values(network.formats).reduce((total, count) => total + count, 0);
  const statusCount = Object.values(network.statuses).reduce((total, count) => total + count, 0);
  if (
    network.attempted < 1 ||
    network.attempted > requestLimit ||
    network.completed !== network.attempted ||
    network.blocked !== 0 ||
    network.failed !== 0 ||
    network.billableRootRequests !== 1 ||
    network.rootTilesetSha256 === null ||
    !SHA256_PATTERN.test(network.rootTilesetSha256) ||
    formatCount !== network.attempted ||
    statusCount !== network.attempted ||
    Object.entries(network.statuses).some(
      ([status, count]) => Number(status) < 200 || Number(status) >= 300 || count < 1,
    )
  ) {
    throw new Error("atlas capture violated its one-session Google request contract");
  }
  return network.rootTilesetSha256;
}

export function assertAtlasCameraRegistration(
  candidates: ProbeCandidateEvidence[],
  requests: ReturnType<typeof deriveRegisteredAtlasRequests>,
): AtlasCameraRegistrationReport {
  if (candidates.length !== requests.ordered.length || candidates.length !== 4) {
    throw new Error("atlas camera evidence is incomplete");
  }
  const anchor = candidates[0];
  if (anchor === undefined || anchor.candidateId !== "r0c0") {
    throw new Error("atlas camera anchor is missing");
  }
  const projectionCentersPixels: Record<string, { x: number; y: number }> = {};
  const horizontalScales: number[] = [];
  const verticalScales: number[] = [];
  const projectionCenterErrors: number[] = [];
  for (let index = 0; index < candidates.length; index += 1) {
    const candidate = candidates[index];
    const request = requests.ordered[index];
    const expected = requests.grid.candidates[index];
    if (
      candidate === undefined ||
      request === undefined ||
      expected === undefined ||
      candidate.candidateId !== expected.candidateId ||
      candidate.cameraWorldMatrix.length !== anchor.cameraWorldMatrix.length ||
      candidate.cameraWorldMatrix.some(
        (value, matrixIndex) => value !== anchor.cameraWorldMatrix[matrixIndex],
      )
    ) {
      throw new Error("atlas capture moved or reordered its fixed camera");
    }
    const horizontalPixelsPerMeter =
      Math.abs(candidate.projectionMatrix[0] ?? Number.NaN) * totalWidth(request) / 2;
    const verticalPixelsPerMeter =
      Math.abs(candidate.projectionMatrix[5] ?? Number.NaN) * totalHeight(request) / 2;
    horizontalScales.push(horizontalPixelsPerMeter);
    verticalScales.push(verticalPixelsPerMeter);
    const center = {
      x:
        -(candidate.projectionMatrix[12] ?? Number.NaN) *
        totalWidth(request) /
        2,
      y:
        -(candidate.projectionMatrix[13] ?? Number.NaN) *
        totalHeight(request) /
        2,
    };
    projectionCentersPixels[candidate.candidateId] = center;
    projectionCenterErrors.push(
      Math.hypot(
        center.x - expected.actualCenterOffsetPixels.x,
        center.y - expected.actualCenterOffsetPixels.y,
      ),
    );
  }
  const expectedScale = 1_000 / requests.ordered[0]!.tile.millimetersPerPixel;
  const maximumScaleErrorPixelsPerMeter = Math.max(
    ...[...horizontalScales, ...verticalScales].map((value) => Math.abs(value - expectedScale)),
  );
  const maximumProjectionCenterErrorPixels = Math.max(...projectionCenterErrors);
  if (
    !Number.isFinite(maximumScaleErrorPixelsPerMeter) ||
    maximumScaleErrorPixelsPerMeter > 1e-9 ||
    !Number.isFinite(maximumProjectionCenterErrorPixels) ||
    maximumProjectionCenterErrorPixels > 1e-6
  ) {
    throw new Error("atlas off-axis projection does not match its registered grid");
  }
  return {
    fixedWorldMatrix: true,
    horizontalPixelsPerMeter: horizontalScales[0] ?? Number.NaN,
    maximumProjectionCenterErrorPixels,
    maximumScaleErrorPixelsPerMeter,
    projectionCentersPixels,
    verticalPixelsPerMeter: verticalScales[0] ?? Number.NaN,
    worldMatrixSha256: sha256(JSON.stringify(anchor.cameraWorldMatrix)),
  };
}

export async function readAtlasCaptureSpec(path: string): Promise<AtlasCaptureSpec> {
  const parsed: unknown = JSON.parse(await readFile(path, "utf8"));
  if (typeof parsed !== "object" || parsed === null) {
    throw new Error("atlas capture spec must be an object");
  }
  const value = parsed as Partial<AtlasCaptureSpec>;
  if (
    value.schema !== ATLAS_CAPTURE_SPEC_SCHEMA ||
    !isSafeIdentifier(value.atlasId) ||
    value.capture === undefined ||
    value.requestLimit !== 1_000 ||
    value.workerEnvelopeMiB !== 2_048
  ) {
    throw new Error("atlas capture spec identity or resource ceiling is invalid");
  }
  deriveRegisteredAtlasRequests(value.capture);
  return value as AtlasCaptureSpec;
}

export async function runAtlasCapture(
  spec: AtlasCaptureSpec,
  outputDirectory: string,
  apiKey: string,
): Promise<string> {
  if (apiKey.length < 6) {
    throw new Error("Google tile credential is missing");
  }
  const output = resolve(outputDirectory);
  await mkdir(dirname(output), { mode: 0o700, recursive: true });
  await assertAbsent(output);
  const staging = resolve(dirname(output), `.atlas-capture-${randomBytes(8).toString("hex")}`);
  await mkdir(staging, { mode: 0o700, recursive: false });
  const requests = deriveRegisteredAtlasRequests(spec.capture);
  const ordered = requests.ordered.map((request, index) => ({
    candidateId: requests.grid.candidates[index]!.candidateId,
    request,
  }));
  const uploadSecrets: string[] = [];
  const memorySampler = new ProcessMemorySampler();
  let ingest: ProbeIngestClient | undefined;
  let coordinator: ProbeCoordinator | undefined;
  let rendererServer: StaticRendererServer | undefined;
  const startedAt = new Date();
  memorySampler.start();
  try {
    ingest = await startProbeIngest(
      staging,
      ordered.map(({ candidateId, request }) => ({ candidateId, request })),
    );
    uploadSecrets.push(...ingest.targets.map((target) => target.upload.token));
    const targets = new Map(ingest.targets.map((target) => [target.candidateId, target.upload]));
    const candidates: ProbeCandidate[] = ordered.map(({ candidateId, request }) => {
      const upload = targets.get(candidateId);
      if (upload === undefined) {
        throw new Error("atlas ingest worker returned incomplete upload targets");
      }
      return { candidateId, request, upload };
    });
    rendererServer = await startStaticRendererServer(resolve(CAPTURE_ROOT, "dist"));
    coordinator = await startProbeCoordinator({ apiKey, candidates, requestLimit: spec.requestLimit });
    uploadSecrets.push(coordinator.token);
    memorySampler.setStage("capture-and-encode");
    const execution = await runDirectChromiumProbe(
      rendererServer.url,
      coordinator,
      spec.capture.readiness.timeoutMs * candidates.length + 60_000,
    );
    const browserResult: ProbeBrowserResult = execution.probe;
    const rootTilesetSha256 = assertNetworkSession(browserResult.network, spec.requestLimit);
    let previousAttempted = 0;
    for (const candidate of browserResult.candidates) {
      if (
        assertNetworkSession(candidate.networkAfterCandidate, spec.requestLimit) !==
          rootTilesetSha256 ||
        candidate.networkAfterCandidate.attempted < previousAttempted ||
        candidate.networkAfterCandidate.attempted > browserResult.network.attempted
      ) {
        throw new Error("atlas candidate telemetry does not belong to one ordered root session");
      }
      previousAttempted = candidate.networkAfterCandidate.attempted;
    }
    const cameraRegistration = assertAtlasCameraRegistration(
      browserResult.candidates,
      requests,
    );
    if (
      browserResult.candidates.some(
        (candidate) => !candidate.complete || candidate.coreCoverageBasisPoints < 9_950,
      )
    ) {
      throw new Error("atlas capture did not meet the complete 99.5 percent coverage gate");
    }

    memorySampler.setStage("validate-bundles");
    const ingested = await ingest.finalize(browserResult.candidates);
    const artifacts = new Map(
      ingested.results.map((result) => [result.candidateId, result.artifacts]),
    );
    const reports: AtlasCaptureCandidateReport[] = ordered.map(
      ({ candidateId, request }, index) => {
        const evidence = browserResult.candidates[index];
        const candidateArtifacts = artifacts.get(candidateId);
        if (
          evidence === undefined ||
          evidence.candidateId !== candidateId ||
          candidateArtifacts === undefined
        ) {
          throw new Error("atlas capture candidate evidence is incomplete");
        }
        return {
          artifacts: candidateArtifacts,
          bundle: relative(staging, resolve(staging, "bundles", candidateId)),
          candidateId,
          evidence,
          request,
        };
      },
    );

    const session: AtlasSessionReport = {
      expiresAt: new Date(startedAt.getTime() + THREE_HOURS_MILLISECONDS).toISOString(),
      rootTilesetSha256,
      sessionId: `google-root-${rootTilesetSha256.slice(0, 16)}`,
      startedAt: startedAt.toISOString(),
    };
    const atlasRequestPath = resolve(staging, "atlas-request.json");
    await writeFile(
      atlasRequestPath,
      `${JSON.stringify(
        {
          atlas_id: spec.atlasId,
          bundle_directories: ordered.map(({ candidateId }) => `bundles/${candidateId}`),
          schema: "isometric-reference-atlas-request/v1",
          source_session: {
            expires_at: session.expiresAt,
            root_tileset_sha256: session.rootTilesetSha256,
            session_id: session.sessionId,
            started_at: session.startedAt,
          },
        },
        null,
        2,
      )}\n`,
      { flag: "wx", mode: 0o600 },
    );
    memorySampler.setStage("compile-atlas");
    const atlasDirectory = resolve(staging, "atlas");
    const compileEvidence = compileRustAtlas(atlasRequestPath, atlasDirectory);
    const inspectEvidence = inspectRustAtlas(atlasDirectory);
    const manifestBytes = await readFile(resolve(atlasDirectory, ATLAS_MANIFEST_FILENAME));
    const manifestSha256 = sha256(manifestBytes);
    if (
      !compileEvidence.includes(manifestSha256) ||
      !inspectEvidence.includes(manifestSha256)
    ) {
      throw new Error("Rust atlas command evidence does not match the canonical manifest hash");
    }

    const nodeMemory = process.memoryUsage();
    memorySampler.setStage("write-report");
    const processTree = memorySampler.stop();
    const workerEnvelopeBytes = spec.workerEnvelopeMiB * 1_024 * 1_024;
    if (
      processTree.peak.treeBytes > workerEnvelopeBytes ||
      ingested.workerMaxRssBytes > workerEnvelopeBytes
    ) {
      throw new Error("atlas capture exceeded its two GiB process-tree memory envelope");
    }
    const report: AtlasCaptureReport = {
      atlas: {
        compileEvidence,
        directory: "atlas",
        inspectEvidence,
        manifestSha256,
        request: "atlas-request.json",
      },
      cameraRegistration,
      candidates: reports,
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
        recommendedParallelWorkers: deriveCaptureWorkerCount(
          totalmem(),
          totalWidth(requests.ordered[0]!),
          workerEnvelopeBytes,
        ),
        workerEnvelopeBytes: Math.max(
          captureWorkerEnvelopeBytes(totalWidth(requests.ordered[0]!)),
          workerEnvelopeBytes,
        ),
      },
      schema: ATLAS_CAPTURE_REPORT_SCHEMA,
      session,
    };
    const reportJson = `${JSON.stringify(report, null, 2)}\n`;
    if (reportJson.includes(apiKey) || uploadSecrets.some((secret) => reportJson.includes(secret))) {
      throw new Error("atlas capture report retained a credential");
    }
    await writeFile(resolve(staging, "atlas-capture-report.json"), reportJson, {
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

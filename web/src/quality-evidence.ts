import type { LoadedReferenceLayer } from "./reference-bundle";

const QUALITY_SCHEMA = "isometric-reference-quality-review/v1";
const MAX_REPORT_BYTES = 1_048_576;
const MAX_IMAGE_BYTES = 20 * 1_024 * 1_024;

export const QUALITY_CANDIDATE_IDS = [
  "baseline-sse20-250mm",
  "lod-sse8-250mm",
  "lod-sse4-250mm",
  "sample-sse8-125mm",
  "maximum-sse4-125mm",
] as const;

export type QualityCandidateId = (typeof QUALITY_CANDIDATE_IDS)[number];

export interface QualityCandidate {
  evidence: {
    coreCoverageBasisPoints: number;
    diagnostics: {
      cachedBytes: number;
      errorTarget: number;
      triangles: number;
      visibleTileDepthMaximum: number;
    };
    elapsedMs: number;
    networkAfterCandidate: { attempted: number };
    visibleTiles: number;
  };
  image: {
    byteLength: number;
    candidateId: QualityCandidateId;
    heightPx: number;
    path: string;
    requestDelta: number;
    sha256: string;
    widthPx: number;
  };
  label: string;
  request: {
    camera: {
      azimuthMillidegrees: number;
      elevationMillidegrees: number;
      orthographicHeightMm: number;
      orthographicWidthMm: number;
    };
    quality: {
      maxScreenSpaceErrorPx: number;
      maximumTileCacheMiB: number;
      minimumTileCacheMiB: number;
      textureMipmaps: boolean;
    };
    tile: {
      coreHeightPx: number;
      coreWidthPx: number;
      guardPx: number;
      millimetersPerPixel: number;
    };
  };
}

export interface QualityReviewReport {
  candidates: QualityCandidate[];
  conclusions: {
    deepestSourceLod: string;
    historicalImagerySelectorAvailable: false;
    sourceLodPlateau: true;
    supersamplingAddsSourceGeometry: false;
  };
  network: {
    attempted: number;
    billableRootRequests: number;
    blocked: number;
    completed: number;
    failed: number;
    requestLimit: number;
  };
  runtime: {
    processTree: { peak: { treeBytes: number } };
  };
  schema: typeof QUALITY_SCHEMA;
}

function record(value: unknown, name: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${name} must be an object`);
  }
  return value as Record<string, unknown>;
}

function integer(value: unknown, minimum: number, maximum: number, name: string): number {
  if (!Number.isSafeInteger(value) || Number(value) < minimum || Number(value) > maximum) {
    throw new Error(`${name} is outside its accepted range`);
  }
  return Number(value);
}

function text(value: unknown, pattern: RegExp, name: string): string {
  if (typeof value !== "string" || !pattern.test(value)) {
    throw new Error(`${name} is invalid`);
  }
  return value;
}

function parseCandidate(value: unknown, expectedId: QualityCandidateId): QualityCandidate {
  const candidate = record(value, "quality candidate");
  const evidence = record(candidate.evidence, "quality candidate evidence");
  const diagnostics = record(evidence.diagnostics, "quality diagnostics");
  const cumulative = record(evidence.networkAfterCandidate, "quality candidate network");
  const image = record(candidate.image, "quality candidate image");
  const request = record(candidate.request, "quality candidate request");
  const camera = record(request.camera, "quality camera");
  const quality = record(request.quality, "quality source profile");
  const tile = record(request.tile, "quality tile");
  const candidateId = text(image.candidateId, /^[a-z0-9-]{1,64}$/, "quality candidate ID");
  if (candidateId !== expectedId) {
    throw new Error("quality candidates are not in their canonical order");
  }
  const highSampling = expectedId.endsWith("125mm");
  const width = integer(image.widthPx, 1, 4_096, "quality image width");
  const height = integer(image.heightPx, 1, 4_096, "quality image height");
  const coreWidth = integer(tile.coreWidthPx, 1, 3_584, "quality core width");
  const coreHeight = integer(tile.coreHeightPx, 1, 3_584, "quality core height");
  const guard = integer(tile.guardPx, 1, 1_024, "quality guard");
  const millimetersPerPixel = integer(
    tile.millimetersPerPixel,
    1,
    100_000,
    "quality source scale",
  );
  const expectedCore = highSampling ? 2_048 : 1_024;
  const expectedGuard = highSampling ? 256 : 128;
  const expectedScale = highSampling ? 125 : 250;
  if (
    width !== expectedCore ||
    height !== expectedCore ||
    coreWidth !== expectedCore ||
    coreHeight !== expectedCore ||
    guard !== expectedGuard ||
    millimetersPerPixel !== expectedScale ||
    (coreWidth + 2 * guard) * millimetersPerPixel !== 320_000 ||
    (coreHeight + 2 * guard) * millimetersPerPixel !== 320_000
  ) {
    throw new Error("quality candidate does not preserve the registered physical footprint");
  }
  const expectedError = expectedId.includes("sse20") ? 20 : expectedId.includes("sse8") ? 8 : 4;
  const errorTarget = integer(
    quality.maxScreenSpaceErrorPx,
    1,
    64,
    "quality screen-space error",
  );
  if (
    errorTarget !== expectedError ||
    diagnostics.errorTarget !== expectedError ||
    quality.textureMipmaps !== false ||
    quality.minimumTileCacheMiB !== 512 ||
    quality.maximumTileCacheMiB !== 2_048 ||
    camera.azimuthMillidegrees !== 330_000 ||
    camera.elevationMillidegrees !== 42_000 ||
    camera.orthographicWidthMm !== 320_000 ||
    camera.orthographicHeightMm !== 320_000
  ) {
    throw new Error("quality candidate changed a fixed camera or acquisition control");
  }
  const path = text(image.path, /^[a-z0-9./-]{1,160}$/, "quality image path");
  if (path !== `candidates/${expectedId}/core.png`) {
    throw new Error("quality image path is not allowlisted");
  }
  return {
    evidence: {
      coreCoverageBasisPoints: integer(
        evidence.coreCoverageBasisPoints,
        9_950,
        10_000,
        "quality source coverage",
      ),
      diagnostics: {
        cachedBytes: integer(diagnostics.cachedBytes, 1, 2_147_483_648, "quality cache"),
        errorTarget,
        triangles: integer(diagnostics.triangles, 1, 100_000_000, "quality triangles"),
        visibleTileDepthMaximum: integer(
          diagnostics.visibleTileDepthMaximum,
          1,
          128,
          "quality tile depth",
        ),
      },
      elapsedMs: integer(evidence.elapsedMs, 1, 300_000, "quality elapsed time"),
      networkAfterCandidate: {
        attempted: integer(cumulative.attempted, 1, 1_000, "quality cumulative requests"),
      },
      visibleTiles: integer(evidence.visibleTiles, 1, 100_000, "quality visible tiles"),
    },
    image: {
      byteLength: integer(image.byteLength, 24, MAX_IMAGE_BYTES, "quality image length"),
      candidateId: expectedId,
      heightPx: height,
      path,
      requestDelta: integer(image.requestDelta, 0, 1_000, "quality request delta"),
      sha256: text(image.sha256, /^[0-9a-f]{64}$/, "quality image SHA-256"),
      widthPx: width,
    },
    label: text(candidate.label, /^.{1,128}$/, "quality candidate label"),
    request: {
      camera: {
        azimuthMillidegrees: 330_000,
        elevationMillidegrees: 42_000,
        orthographicHeightMm: 320_000,
        orthographicWidthMm: 320_000,
      },
      quality: {
        maxScreenSpaceErrorPx: expectedError,
        maximumTileCacheMiB: 2_048,
        minimumTileCacheMiB: 512,
        textureMipmaps: false,
      },
      tile: {
        coreHeightPx: coreHeight,
        coreWidthPx: coreWidth,
        guardPx: guard,
        millimetersPerPixel,
      },
    },
  };
}

export function parseQualityReviewReport(value: unknown): QualityReviewReport {
  const report = record(value, "quality review report");
  if (report.schema !== QUALITY_SCHEMA || !Array.isArray(report.candidates)) {
    throw new Error("quality review report schema is unsupported");
  }
  const rawCandidates = report.candidates;
  const candidates = QUALITY_CANDIDATE_IDS.map((id, index) =>
    parseCandidate(rawCandidates[index], id),
  );
  if (rawCandidates.length !== candidates.length) {
    throw new Error("quality review contains an unexpected candidate");
  }
  const network = record(report.network, "quality network evidence");
  const attempted = integer(network.attempted, 1, 1_000, "quality attempted requests");
  const parsedNetwork = {
    attempted,
    billableRootRequests: integer(network.billableRootRequests, 1, 1, "quality root requests"),
    blocked: integer(network.blocked, 0, 0, "quality blocked requests"),
    completed: integer(network.completed, attempted, attempted, "quality completed requests"),
    failed: integer(network.failed, 0, 0, "quality failed requests"),
    requestLimit: integer(network.requestLimit, attempted, 1_000, "quality request limit"),
  };
  let cumulative = 0;
  for (const candidate of candidates) {
    cumulative += candidate.image.requestDelta;
    if (candidate.evidence.networkAfterCandidate.attempted !== cumulative) {
      throw new Error("quality candidate request deltas contradict cumulative telemetry");
    }
  }
  if (cumulative !== attempted) {
    throw new Error("quality candidate requests contradict the session total");
  }
  const conclusions = record(report.conclusions, "quality conclusions");
  const sse8 = candidates[1];
  const sse4 = candidates[2];
  const sample8 = candidates[3];
  const sample4 = candidates[4];
  if (
    conclusions.deepestSourceLod !== "sse8-250mm" ||
    conclusions.historicalImagerySelectorAvailable !== false ||
    conclusions.sourceLodPlateau !== true ||
    conclusions.supersamplingAddsSourceGeometry !== false ||
    sse8.image.sha256 !== sse4.image.sha256 ||
    sample8.image.sha256 !== sample4.image.sha256 ||
    sse4.image.requestDelta !== 0 ||
    sample4.image.requestDelta !== 0
  ) {
    throw new Error("quality conclusions are not supported by the registered candidates");
  }
  const runtime = record(report.runtime, "quality runtime evidence");
  const processTree = record(runtime.processTree, "quality process tree");
  const peak = record(processTree.peak, "quality process peak");
  const treeBytes = integer(peak.treeBytes, 1, 3 * 1_024 ** 3, "quality process RSS");
  return {
    candidates,
    conclusions: {
      deepestSourceLod: "sse8-250mm",
      historicalImagerySelectorAvailable: false,
      sourceLodPlateau: true,
      supersamplingAddsSourceGeometry: false,
    },
    network: parsedNetwork,
    runtime: { processTree: { peak: { treeBytes } } },
    schema: QUALITY_SCHEMA,
  };
}

function pngDimensions(bytes: Uint8Array): { height: number; width: number } {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (
    bytes.length < 24 ||
    bytes[0] !== 137 ||
    String.fromCharCode(...bytes.subarray(12, 16)) !== "IHDR"
  ) {
    throw new Error("quality evidence is not a PNG");
  }
  return { height: view.getUint32(20), width: view.getUint32(16) };
}

async function sha256(bytes: Uint8Array): Promise<string> {
  const copy = new Uint8Array(bytes.length);
  copy.set(bytes);
  const digest = await crypto.subtle.digest("SHA-256", copy.buffer);
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

async function boundedFetch(url: string, maximum: number, signal: AbortSignal): Promise<Uint8Array> {
  const response = await fetch(url, { cache: "no-store", signal });
  if (!response.ok) {
    throw new Error(`quality evidence request failed with status ${response.status}`);
  }
  const declared = Number(response.headers.get("content-length"));
  if (Number.isFinite(declared) && declared > maximum) {
    throw new Error("quality evidence exceeds its declared byte budget");
  }
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length > maximum) {
    throw new Error("quality evidence exceeds its byte budget");
  }
  return bytes;
}

export async function loadQualityReviewReport(
  url: string,
  signal: AbortSignal,
): Promise<QualityReviewReport> {
  const bytes = await boundedFetch(url, MAX_REPORT_BYTES, signal);
  return parseQualityReviewReport(JSON.parse(new TextDecoder().decode(bytes)));
}

export async function loadQualityImage(
  reportUrl: string,
  candidate: QualityCandidate,
  signal: AbortSignal,
): Promise<LoadedReferenceLayer> {
  const absoluteReportUrl = new URL(reportUrl, window.location.href);
  const url = new URL(candidate.image.path, absoluteReportUrl).toString();
  const bytes = await boundedFetch(url, candidate.image.byteLength, signal);
  if (bytes.length !== candidate.image.byteLength || (await sha256(bytes)) !== candidate.image.sha256) {
    throw new Error(`${candidate.image.candidateId} SHA-256 does not match the quality report`);
  }
  const dimensions = pngDimensions(bytes);
  if (dimensions.width !== candidate.image.widthPx || dimensions.height !== candidate.image.heightPx) {
    throw new Error(`${candidate.image.candidateId} dimensions do not match the quality report`);
  }
  return {
    bytes,
    record: {
      byte_length: bytes.length,
      encoding: "png-rgba8",
      height_px: dimensions.height,
      kind: "color",
      path: candidate.image.path,
      sha256: candidate.image.sha256,
      width_px: dimensions.width,
    },
  };
}

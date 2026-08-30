import { createHash } from "node:crypto";
import { readFile, stat, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const QUALITY_SCHEMA = "isometric-reference-quality-review/v1";
const PROBE_SCHEMA = "isometric-reference-probe-report/v1";
const CANDIDATE_IDS = [
  "baseline-sse20-250mm",
  "lod-sse8-250mm",
  "lod-sse4-250mm",
  "sample-sse8-125mm",
  "maximum-sse4-125mm",
] as const;
const SOURCE_GEOMETRY_METRICS = [
  "cachedBytes",
  "triangles",
  "visibleTileDepthMaximum",
  "visibleTileDepthMedian",
  "visibleTileDepthMinimum",
] as const;

interface ProbeCandidate {
  artifacts: { coreFile: string };
  candidateId: string;
  evidence: {
    coreCoverageBasisPoints: number;
    diagnostics: Record<string, number>;
    elapsedMs: number;
    networkAfterCandidate: { attempted: number };
    visibleTiles: number;
  };
  label: string;
  request: {
    camera: Record<string, number | string>;
    quality: Record<string, number | boolean>;
    tile: Record<string, number | string>;
  };
}

interface ProbeReport {
  candidates: ProbeCandidate[];
  network: Record<string, unknown>;
  runtime: Record<string, unknown>;
  schema: string;
}

export interface QualityImageRecord {
  byteLength: number;
  candidateId: string;
  heightPx: number;
  path: string;
  requestDelta: number;
  sha256: string;
  widthPx: number;
}

export interface QualityReviewReport {
  candidates: Array<{
    evidence: ProbeCandidate["evidence"];
    image: QualityImageRecord;
    label: string;
    request: ProbeCandidate["request"];
  }>;
  conclusions: {
    deepestSourceLod: string;
    historicalImagerySelectorAvailable: false;
    sourceLodPlateau: boolean;
    supersamplingAddsSourceGeometry: boolean;
  };
  network: Record<string, unknown>;
  runtime: Record<string, unknown>;
  schema: typeof QUALITY_SCHEMA;
}

function pngDimensions(bytes: Buffer): { height: number; width: number } {
  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  if (
    bytes.length < 24 ||
    !bytes.subarray(0, signature.length).equals(signature) ||
    bytes.subarray(12, 16).toString("ascii") !== "IHDR"
  ) {
    throw new Error("quality candidate is not a bounded PNG");
  }
  return { height: bytes.readUInt32BE(20), width: bytes.readUInt32BE(16) };
}

function record(value: unknown, name: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${name} must be an object`);
  }
  return value as Record<string, unknown>;
}

function parseProbe(value: unknown): ProbeReport {
  const report = record(value, "quality probe report");
  if (
    report.schema !== PROBE_SCHEMA ||
    !Array.isArray(report.candidates) ||
    report.candidates.length !== CANDIDATE_IDS.length
  ) {
    throw new Error("quality probe report identity is invalid");
  }
  for (const [index, expectedId] of CANDIDATE_IDS.entries()) {
    const candidate = record(report.candidates[index], "quality candidate");
    if (candidate.candidateId !== expectedId) {
      throw new Error("quality probe candidate order is invalid");
    }
  }
  return report as unknown as ProbeReport;
}

function sourceGeometrySignature(candidate: QualityReviewReport["candidates"][number]): string {
  const values = [candidate.evidence.visibleTiles];
  for (const key of SOURCE_GEOMETRY_METRICS) {
    values.push(candidate.evidence.diagnostics[key] as number);
  }
  if (values.some((value) => !Number.isSafeInteger(value) || value < 0)) {
    throw new Error("quality candidate source-geometry evidence is incomplete");
  }
  return values.join(":");
}

export async function writeQualityReviewReport(directory: string): Promise<string> {
  const root = resolve(directory);
  const sourcePath = resolve(root, "report.json");
  const source = parseProbe(JSON.parse(await readFile(sourcePath, "utf8")));
  const candidates: QualityReviewReport["candidates"] = [];
  let previousRequests = 0;
  for (const candidate of source.candidates) {
    const relativePath = `candidates/${candidate.candidateId}/core.png`;
    if (candidate.artifacts.coreFile !== "core.png") {
      throw new Error("quality probe candidate core path is invalid");
    }
    const path = resolve(root, relativePath);
    const metadata = await stat(path);
    if (!metadata.isFile() || metadata.size < 24 || metadata.size > 32 * 1_024 * 1_024) {
      throw new Error("quality candidate exceeds its bounded image contract");
    }
    const bytes = await readFile(path);
    const dimensions = pngDimensions(bytes);
    const attempted = candidate.evidence.networkAfterCandidate.attempted;
    if (!Number.isSafeInteger(attempted) || attempted < previousRequests) {
      throw new Error("quality candidate network sequence is invalid");
    }
    candidates.push({
      evidence: candidate.evidence,
      image: {
        byteLength: bytes.length,
        candidateId: candidate.candidateId,
        heightPx: dimensions.height,
        path: relativePath,
        requestDelta: attempted - previousRequests,
        sha256: createHash("sha256").update(bytes).digest("hex"),
        widthPx: dimensions.width,
      },
      label: candidate.label,
      request: candidate.request,
    });
    previousRequests = attempted;
  }
  const byId = new Map(candidates.map((candidate) => [candidate.image.candidateId, candidate]));
  const sse8 = byId.get("lod-sse8-250mm");
  const sse4 = byId.get("lod-sse4-250mm");
  const sample8 = byId.get("sample-sse8-125mm");
  const sample4 = byId.get("maximum-sse4-125mm");
  if (!sse8 || !sse4 || !sample8 || !sample4) {
    throw new Error("quality candidate evidence is incomplete");
  }
  const sse8Geometry = sourceGeometrySignature(sse8);
  const sample8Geometry = sourceGeometrySignature(sample8);
  const sourceLodPlateau =
    sse8.image.sha256 === sse4.image.sha256 &&
    sse4.image.requestDelta === 0 &&
    sse8Geometry === sourceGeometrySignature(sse4) &&
    sample8.image.sha256 === sample4.image.sha256 &&
    sample4.image.requestDelta === 0 &&
    sample8Geometry === sourceGeometrySignature(sample4);
  const supersamplingAddsSourceGeometry =
    sample8.image.requestDelta > 0 || sample8Geometry !== sse8Geometry;
  const report: QualityReviewReport = {
    candidates,
    conclusions: {
      deepestSourceLod: sourceLodPlateau ? "sse8-250mm" : "not-proven",
      historicalImagerySelectorAvailable: false,
      sourceLodPlateau,
      supersamplingAddsSourceGeometry,
    },
    network: source.network,
    runtime: source.runtime,
    schema: QUALITY_SCHEMA,
  };
  const output = resolve(root, "quality-review.json");
  await writeFile(output, `${JSON.stringify(report, null, 2)}\n`, { flag: "wx", mode: 0o600 });
  return output;
}

import type { LoadedReferenceLayer } from "./reference-bundle";

const REPAIR_SCHEMA = "isometric-reference-repair-review/v1";
const ALGORITHM = "reference-repair-rust/v1";
const MAX_REPORT_BYTES = 1_048_576;
const MAX_IMAGE_BYTES = 8 * 1_024 * 1_024;

export const REPAIR_IMAGE_IDS = [
  "source-logical",
  "candidate-a-rgb",
  "candidate-b-geometry",
  "candidate-c-canopy-repair",
  "canopy-mask",
  "structural-edges",
] as const;

export type RepairImageId = (typeof REPAIR_IMAGE_IDS)[number];

export interface RepairImageRecord {
  byte_length: number;
  height_px: number;
  id: RepairImageId;
  label: string;
  path: string;
  sha256: string;
  width_px: number;
}

export interface RepairCandidateMetrics {
  candidate_id: "candidate-a-rgb" | "candidate-b-geometry" | "candidate-c-canopy-repair";
  canopy_interior_edge_ppm: number;
  changed_from_source_ppm: number;
  colors_used: number;
  mean_luminance_microunits: number;
  non_structural_edge_ppm: number;
  structural_edge_recall_basis_points: number;
}

export interface RepairReviewReport {
  algorithm: typeof ALGORITHM;
  blocking_findings: string[];
  camera_azimuth_millidegrees: number;
  camera_elevation_millidegrees: number;
  candidates: RepairCandidateMetrics[];
  canopy_pixels: number;
  estimated_peak_working_bytes: number;
  gates: {
    canopy_fragmentation_improved: boolean;
    deterministic_post_capture: boolean;
    palette_bound: boolean;
    passenger_cars_preserved_by_policy: boolean;
    qualified_for_expansion: false;
    structural_edge_recall: boolean;
  };
  images: RepairImageRecord[];
  logical_millimeters_per_pixel: number;
  schema: typeof REPAIR_SCHEMA;
  source_bundle_id: string;
  source_manifest_sha256: string;
  source_millimeters_per_pixel: number;
  structural_edge_pixels: number;
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

export function parseRepairReviewReport(value: unknown): RepairReviewReport {
  const report = record(value, "repair review report");
  if (report.schema !== REPAIR_SCHEMA || report.algorithm !== ALGORITHM) {
    throw new Error("repair review report schema is unsupported");
  }
  const rawImages = report.images;
  if (!Array.isArray(rawImages) || rawImages.length !== REPAIR_IMAGE_IDS.length) {
    throw new Error("repair review image set is incomplete");
  }
  const images = REPAIR_IMAGE_IDS.map((expectedId, index): RepairImageRecord => {
    const image = record(rawImages[index], "repair review image");
    const id = text(image.id, /^[a-z0-9-]{1,64}$/, "repair image ID");
    const path = text(image.path, /^[a-z0-9-]+\.png$/, "repair image path");
    if (id !== expectedId || path !== `${expectedId}.png`) {
      throw new Error("repair review images are not in canonical order");
    }
    return {
      byte_length: integer(image.byte_length, 24, MAX_IMAGE_BYTES, "repair image length"),
      height_px: integer(image.height_px, 1, 2_048, "repair image height"),
      id: expectedId,
      label: text(image.label, /^.{1,160}$/, "repair image label"),
      path,
      sha256: text(image.sha256, /^[0-9a-f]{64}$/, "repair image SHA-256"),
      width_px: integer(image.width_px, 1, 2_048, "repair image width"),
    };
  });
  const rawCandidates = report.candidates;
  if (!Array.isArray(rawCandidates) || rawCandidates.length !== 3) {
    throw new Error("repair candidate metrics are incomplete");
  }
  const candidateIds = REPAIR_IMAGE_IDS.slice(1, 4) as RepairCandidateMetrics["candidate_id"][];
  const candidates = candidateIds.map((candidateId, index): RepairCandidateMetrics => {
    const metrics = record(rawCandidates[index], "repair candidate metrics");
    if (metrics.candidate_id !== candidateId) {
      throw new Error("repair candidate metrics are not in canonical order");
    }
    return {
      candidate_id: candidateId,
      canopy_interior_edge_ppm: integer(metrics.canopy_interior_edge_ppm, 0, 1_000_000, "canopy edge density"),
      changed_from_source_ppm: integer(metrics.changed_from_source_ppm, 0, 1_000_000, "changed pixels"),
      colors_used: integer(metrics.colors_used, 1, 128, "candidate colors"),
      mean_luminance_microunits: integer(metrics.mean_luminance_microunits, 0, 255_000_000, "candidate luminance"),
      non_structural_edge_ppm: integer(metrics.non_structural_edge_ppm, 0, 1_000_000, "non-structural edges"),
      structural_edge_recall_basis_points: integer(metrics.structural_edge_recall_basis_points, 0, 10_000, "structural edge recall"),
    };
  });
  const gates = record(report.gates, "repair gates");
  const gate = (name: string) => {
    if (typeof gates[name] !== "boolean") {
      throw new Error(`repair gate ${name} is invalid`);
    }
    return Boolean(gates[name]);
  };
  if (gate("qualified_for_expansion")) {
    throw new Error("the bounded repair study cannot self-qualify expansion");
  }
  if (!Array.isArray(report.blocking_findings) || report.blocking_findings.length === 0) {
    throw new Error("repair review blockers are missing");
  }
  const blockers = report.blocking_findings.map((finding) =>
    text(finding, /^[a-z0-9-]{1,96}$/, "repair blocker"),
  );
  return {
    algorithm: ALGORITHM,
    blocking_findings: blockers,
    camera_azimuth_millidegrees: integer(report.camera_azimuth_millidegrees, 0, 360_000, "camera azimuth"),
    camera_elevation_millidegrees: integer(report.camera_elevation_millidegrees, 1, 90_000, "camera elevation"),
    candidates,
    canopy_pixels: integer(report.canopy_pixels, 0, 4_194_304, "canopy pixels"),
    estimated_peak_working_bytes: integer(report.estimated_peak_working_bytes, 1, 96 * 1_024 * 1_024, "working memory"),
    gates: {
      canopy_fragmentation_improved: gate("canopy_fragmentation_improved"),
      deterministic_post_capture: gate("deterministic_post_capture"),
      palette_bound: gate("palette_bound"),
      passenger_cars_preserved_by_policy: gate("passenger_cars_preserved_by_policy"),
      qualified_for_expansion: false,
      structural_edge_recall: gate("structural_edge_recall"),
    },
    images,
    logical_millimeters_per_pixel: integer(report.logical_millimeters_per_pixel, 1, 10_000, "logical sampling"),
    schema: REPAIR_SCHEMA,
    source_bundle_id: text(report.source_bundle_id, /^[a-z0-9-]{1,96}$/, "source bundle ID"),
    source_manifest_sha256: text(report.source_manifest_sha256, /^[0-9a-f]{64}$/, "source manifest SHA-256"),
    source_millimeters_per_pixel: integer(report.source_millimeters_per_pixel, 1, 10_000, "source sampling"),
    structural_edge_pixels: integer(report.structural_edge_pixels, 0, 4_194_304, "structural pixels"),
  };
}

function pngDimensions(bytes: Uint8Array): { height: number; width: number } {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (bytes.length < 24 || bytes[0] !== 137 || String.fromCharCode(...bytes.subarray(12, 16)) !== "IHDR") {
    throw new Error("repair evidence is not a PNG");
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
    throw new Error(`repair evidence request failed with status ${response.status}`);
  }
  const declared = Number(response.headers.get("content-length"));
  if (Number.isFinite(declared) && declared > maximum) {
    throw new Error("repair evidence exceeds its declared byte budget");
  }
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length > maximum) {
    throw new Error("repair evidence exceeds its byte budget");
  }
  return bytes;
}

export async function loadRepairReviewReport(url: string, signal: AbortSignal): Promise<RepairReviewReport> {
  const bytes = await boundedFetch(url, MAX_REPORT_BYTES, signal);
  return parseRepairReviewReport(JSON.parse(new TextDecoder().decode(bytes)));
}

export async function loadRepairImage(
  reportUrl: string,
  image: RepairImageRecord,
  signal: AbortSignal,
): Promise<LoadedReferenceLayer> {
  const absoluteReportUrl = new URL(reportUrl, window.location.href);
  const url = new URL(image.path, absoluteReportUrl).toString();
  const bytes = await boundedFetch(url, image.byte_length, signal);
  if (bytes.length !== image.byte_length || (await sha256(bytes)) !== image.sha256) {
    throw new Error(`${image.id} SHA-256 does not match the repair report`);
  }
  const dimensions = pngDimensions(bytes);
  if (dimensions.width !== image.width_px || dimensions.height !== image.height_px) {
    throw new Error(`${image.id} dimensions do not match the repair report`);
  }
  return {
    bytes,
    record: {
      byte_length: bytes.length,
      encoding: "png-rgba8",
      height_px: dimensions.height,
      kind: "color",
      path: image.path,
      sha256: image.sha256,
      width_px: dimensions.width,
    },
  };
}

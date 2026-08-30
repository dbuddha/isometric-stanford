const REPORT_SCHEMA = "isometric-reference-overlap-experiment/v1";
const COMPARISON_SCHEMA = "isometric-reference-overlap-report/v1";
const MAX_REPORT_BYTES = 2 * 1024 * 1024;
const MAX_IMAGE_BYTES = 16 * 1024 * 1024;
const MAX_TOTAL_IMAGE_BYTES = 64 * 1024 * 1024;
const PNG_SIGNATURE = [137, 80, 78, 71, 13, 10, 26, 10] as const;

export const CORE_IMAGE_IDS = [
  "joined-core",
  "monolithic-core",
  "core-oracle-heatmap",
] as const;
export const OVERLAP_IMAGE_IDS = [
  "overlap-left",
  "overlap-right",
  "overlap-monolithic",
  "overlap-heatmap",
] as const;
export const OVERLAP_IMAGE_IDS_ALL = [...CORE_IMAGE_IDS, ...OVERLAP_IMAGE_IDS] as const;

export type OverlapImageId = (typeof OVERLAP_IMAGE_IDS_ALL)[number];

export const OVERLAP_IMAGE_LABELS: Record<OverlapImageId, string> = {
  "joined-core": "Joined independent cores",
  "monolithic-core": "Monolithic oracle core",
  "core-oracle-heatmap": "Core mismatch heatmap",
  "overlap-left": "Left guard overlap",
  "overlap-right": "Right guard overlap",
  "overlap-monolithic": "Monolithic overlap",
  "overlap-heatmap": "Guard mismatch heatmap",
};

export interface OverlapEvidenceImage {
  byte_length: number;
  height_px: number;
  path: string;
  sha256: string;
  width_px: number;
}

export interface DifferenceMetrics {
  exact_mismatch_pixels: number;
  maximum_absolute_difference: number;
  mean_absolute_difference_microunits: number;
  passed: boolean;
  pixels_above_tolerance: number;
  pixels_above_tolerance_ppm: number;
  pixels_compared: number;
}

export interface LayerComparison {
  gate: {
    maximum_absolute_difference: number;
    maximum_above_tolerance_ppm: number;
  };
  joined_vs_monolithic_core: DifferenceMetrics;
  joined_boundary_vs_monolithic: DifferenceMetrics;
  left_vs_monolithic_overlap: DifferenceMetrics;
  left_vs_right_overlap: DifferenceMetrics;
  left_vs_right_seam_corridor: DifferenceMetrics;
  right_vs_monolithic_overlap: DifferenceMetrics;
}

export interface OverlapExperimentReport {
  cameraRegistration: {
    fixedWorldMatrix: true;
    horizontalPixelsPerMeter: number;
    maximumScaleErrorPixelsPerMeter: number;
    projectionCenterX: { left: number; monolithic: number; right: number };
    verticalPixelsPerMeter: number;
    worldMatrixSha256: string;
  };
  candidates: Array<{
    candidateId: "left" | "monolithic" | "right";
    evidence: {
      coreCoverageBasisPoints: number;
      elapsedMs: number;
      visibleTiles: number;
    };
  }>;
  comparison: {
    boundary_structural_edge_pixels: number;
    failure_classifications: string[];
    gates: {
      all_relations: boolean;
      lighting_seam: boolean;
      source: {
        independent_seam: boolean;
        monolithic_seam: boolean;
      };
    };
    images: Record<OverlapImageId, OverlapEvidenceImage>;
    layers: Record<string, LayerComparison>;
    passed: boolean;
    registration_search: {
      baseline_above_tolerance_ppm: number;
      best_above_tolerance_ppm: number;
      best_dx_px: number;
      best_dy_px: number;
      observations_compared: number;
      radius_px: number;
    };
    schema: typeof COMPARISON_SCHEMA;
  };
  grid: {
    cameraScreenRightBearingMillidegrees: number;
    checkedSavedPixelCenters: number;
    maximumPixelCenterErrorPixels: number;
  };
  network: {
    attempted: number;
    billableRootRequests: number;
    blocked: number;
    completed: number;
    failed: number;
    formats: { glb: number; json: number };
    requestLimit: number;
    responseBodyBytes: number;
    statuses: Record<string, number>;
  };
  runtime: {
    ingestWorkerMaxRssBytes: number;
    nodeMaxRssBytes: number;
    processTree: { peakProcessTreeRssBytes: number };
    workerEnvelopeBytes: number;
  };
  schema: typeof REPORT_SCHEMA;
}

export interface LoadedOverlapImage {
  bytes: Uint8Array;
  id: OverlapImageId;
  record: OverlapEvidenceImage;
}

export interface LoadedOverlapEvidence {
  images: ReadonlyMap<OverlapImageId, LoadedOverlapImage>;
  report: OverlapExperimentReport;
  reportUrl: string;
}

type Fetcher = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

function record(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function integer(value: unknown, minimum: number, maximum: number, name: string): number {
  if (!Number.isSafeInteger(value) || Number(value) < minimum || Number(value) > maximum) {
    throw new Error(`${name} is outside its accepted integer range`);
  }
  return Number(value);
}

function countMap(
  value: unknown,
  maximumEntries: number,
  maximumCount: number,
  name: string,
): Record<string, number> {
  const values = record(value);
  if (!values || Object.keys(values).length > maximumEntries) {
    throw new Error(`${name} violates its bounded count map`);
  }
  return Object.fromEntries(
    Object.entries(values).map(([key, count]) => {
      if (!/^[a-z0-9-]{1,32}$/.test(key)) {
        throw new Error(`${name} contains an invalid key`);
      }
      return [key, integer(count, 0, maximumCount, `${name} ${key}`)];
    }),
  );
}

function boundedNumber(value: unknown, minimum: number, maximum: number, name: string): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) {
    throw new Error(`${name} is outside its accepted numeric range`);
  }
  return value;
}

function parseDifference(value: unknown, name: string): DifferenceMetrics {
  const metrics = record(value);
  if (!metrics || typeof metrics.passed !== "boolean") {
    throw new Error(`${name} comparison metrics are invalid`);
  }
  const pixelsCompared = integer(metrics.pixels_compared, 1, 100_000_000, `${name} pixels`);
  const exactMismatch = integer(
    metrics.exact_mismatch_pixels,
    0,
    pixelsCompared,
    `${name} exact mismatch pixels`,
  );
  const above = integer(
    metrics.pixels_above_tolerance,
    0,
    pixelsCompared,
    `${name} above-tolerance pixels`,
  );
  return {
    exact_mismatch_pixels: exactMismatch,
    maximum_absolute_difference: integer(
      metrics.maximum_absolute_difference,
      0,
      0xffff_ffff,
      `${name} maximum difference`,
    ),
    mean_absolute_difference_microunits: integer(
      metrics.mean_absolute_difference_microunits,
      0,
      0xffff_ffff * 1_000_000,
      `${name} mean difference`,
    ),
    passed: metrics.passed,
    pixels_above_tolerance: above,
    pixels_above_tolerance_ppm: integer(
      metrics.pixels_above_tolerance_ppm,
      0,
      1_000_000,
      `${name} above-tolerance ppm`,
    ),
    pixels_compared: pixelsCompared,
  };
}

function parseLayer(value: unknown, name: string): LayerComparison {
  const layer = record(value);
  const gate = layer ? record(layer.gate) : null;
  if (!layer || !gate) {
    throw new Error(`${name} layer comparison is invalid`);
  }
  return {
    gate: {
      maximum_absolute_difference: integer(
        gate.maximum_absolute_difference,
        0,
        0xffff_ffff,
        `${name} gate maximum difference`,
      ),
      maximum_above_tolerance_ppm: integer(
        gate.maximum_above_tolerance_ppm,
        0,
        1_000_000,
        `${name} gate ppm`,
      ),
    },
    joined_vs_monolithic_core: parseDifference(
      layer.joined_vs_monolithic_core,
      `${name} joined core`,
    ),
    joined_boundary_vs_monolithic: parseDifference(
      layer.joined_boundary_vs_monolithic,
      `${name} joined boundary`,
    ),
    left_vs_monolithic_overlap: parseDifference(
      layer.left_vs_monolithic_overlap,
      `${name} left oracle`,
    ),
    left_vs_right_overlap: parseDifference(
      layer.left_vs_right_overlap,
      `${name} independent overlap`,
    ),
    left_vs_right_seam_corridor: parseDifference(
      layer.left_vs_right_seam_corridor,
      `${name} independent seam corridor`,
    ),
    right_vs_monolithic_overlap: parseDifference(
      layer.right_vs_monolithic_overlap,
      `${name} right oracle`,
    ),
  };
}

function parseImage(value: unknown, id: OverlapImageId): OverlapEvidenceImage {
  const image = record(value);
  const expectedPath = `${id}.png`;
  if (
    !image ||
    image.path !== expectedPath ||
    typeof image.sha256 !== "string" ||
    !/^[0-9a-f]{64}$/.test(image.sha256)
  ) {
    throw new Error(`${id} violates its overlap image identity`);
  }
  return {
    byte_length: integer(image.byte_length, 1, MAX_IMAGE_BYTES, `${id} byte length`),
    height_px: integer(image.height_px, 1, 4_096, `${id} height`),
    path: expectedPath,
    sha256: image.sha256,
    width_px: integer(image.width_px, 1, 4_096, `${id} width`),
  };
}

function parseReport(value: unknown): OverlapExperimentReport {
  const report = record(value);
  const comparison = report ? record(report.comparison) : null;
  const cameraRegistration = report ? record(report.cameraRegistration) : null;
  const projectionCenter = cameraRegistration ? record(cameraRegistration.projectionCenterX) : null;
  const images = comparison ? record(comparison.images) : null;
  const grid = report ? record(report.grid) : null;
  const network = report ? record(report.network) : null;
  const runtime = report ? record(report.runtime) : null;
  const processTree = runtime ? record(runtime.processTree) : null;
  const processTreePeak = processTree ? record(processTree.peak) : null;
  const registration = comparison ? record(comparison.registration_search) : null;
  const gates = comparison ? record(comparison.gates) : null;
  const sourceGates = gates ? record(gates.source) : null;
  const rawLayers = comparison ? record(comparison.layers) : null;
  if (
    !report ||
    report.schema !== REPORT_SCHEMA ||
    !comparison ||
    !cameraRegistration ||
    cameraRegistration.fixedWorldMatrix !== true ||
    typeof cameraRegistration.worldMatrixSha256 !== "string" ||
    !/^[0-9a-f]{64}$/.test(cameraRegistration.worldMatrixSha256) ||
    !projectionCenter ||
    comparison.schema !== COMPARISON_SCHEMA ||
    typeof comparison.passed !== "boolean" ||
    !Array.isArray(comparison.failure_classifications) ||
    comparison.failure_classifications.some((value) => typeof value !== "string") ||
    !images ||
    !grid ||
    !network ||
    !runtime ||
    !processTree ||
    !processTreePeak ||
    !Array.isArray(report.candidates) ||
    report.candidates.length !== 3 ||
    !rawLayers ||
    !registration ||
    !gates ||
    !sourceGates ||
    [
      gates.all_relations,
      gates.lighting_seam,
      sourceGates.independent_seam,
      sourceGates.monolithic_seam,
    ].some((item) => typeof item !== "boolean")
  ) {
    throw new Error("registered overlap report schema is invalid");
  }
  const parsedImages = Object.fromEntries(
    OVERLAP_IMAGE_IDS_ALL.map((id) => [id, parseImage(images[id], id)]),
  ) as Record<OverlapImageId, OverlapEvidenceImage>;
  const total = Object.values(parsedImages).reduce((sum, image) => sum + image.byte_length, 0);
  if (total > MAX_TOTAL_IMAGE_BYTES) {
    throw new Error("registered overlap images exceed their aggregate byte budget");
  }
  if (
    parsedImages["joined-core"].width_px !== parsedImages["monolithic-core"].width_px ||
    parsedImages["joined-core"].height_px !== parsedImages["monolithic-core"].height_px ||
    parsedImages["core-oracle-heatmap"].width_px !== parsedImages["joined-core"].width_px ||
    parsedImages["core-oracle-heatmap"].height_px !== parsedImages["joined-core"].height_px ||
    OVERLAP_IMAGE_IDS.some(
      (id) =>
        parsedImages[id].width_px !== parsedImages["overlap-left"].width_px ||
        parsedImages[id].height_px !== parsedImages["overlap-left"].height_px,
    )
  ) {
    throw new Error("registered overlap images contradict their shared comparison grids");
  }
  const layerNames = [
    "color",
    "coverage",
    "fixed-shadow",
    "linear-depth",
    "view-normal",
    "whitebox",
  ];
  if (
    Object.keys(rawLayers).length !== layerNames.length ||
    layerNames.some((name) => rawLayers[name] === undefined)
  ) {
    throw new Error("registered overlap report requires exactly six compared layers");
  }
  const layers = Object.fromEntries(
    layerNames.map((name) => [name, parseLayer(rawLayers[name], name)]),
  );
  const candidateIds = new Set<string>();
  const candidates = report.candidates.map((value, index) => {
    const candidate = record(value);
    const evidence = candidate ? record(candidate.evidence) : null;
    if (
      !candidate ||
      !evidence ||
      !["left", "monolithic", "right"].includes(String(candidate.candidateId)) ||
      candidateIds.has(String(candidate.candidateId))
    ) {
      throw new Error(`candidate ${index} violates the one-session experiment contract`);
    }
    candidateIds.add(String(candidate.candidateId));
    return {
      candidateId: candidate.candidateId as "left" | "monolithic" | "right",
      evidence: {
        coreCoverageBasisPoints: integer(
          evidence.coreCoverageBasisPoints,
          9_950,
          10_000,
          `candidate ${index} coverage`,
        ),
        elapsedMs: boundedNumber(evidence.elapsedMs, 0, 300_000, `candidate ${index} elapsed time`),
        visibleTiles: integer(evidence.visibleTiles, 1, 10_000, `candidate ${index} visible tiles`),
      },
    };
  });
  const attempted = integer(network.attempted, 1, 450, "attempted Google requests");
  const completed = integer(network.completed, 1, attempted, "completed Google requests");
  const failed = integer(network.failed, 0, attempted, "failed Google requests");
  const blocked = integer(network.blocked, 0, 0, "blocked Google request count");
  const requestLimit = integer(network.requestLimit, 450, 450, "Google request ceiling");
  const formats = countMap(network.formats, 3, attempted, "Google response formats");
  const statuses = countMap(network.statuses, 8, completed, "Google response statuses");
  if (
    Object.keys(formats).some((key) => key !== "glb" && key !== "json" && key !== "other") ||
    Object.values(formats).reduce((sum, count) => sum + count, 0) !== attempted ||
    Object.values(statuses).reduce((sum, count) => sum + count, 0) !== completed ||
    completed + failed > attempted ||
    formats.glb === undefined ||
    formats.json === undefined
  ) {
    throw new Error("Google network evidence contradicts the one-session request totals");
  }
  const baselinePpm = integer(
    registration.baseline_above_tolerance_ppm,
    0,
    1_000_000,
    "registration baseline ppm",
  );
  const bestPpm = integer(
    registration.best_above_tolerance_ppm,
    0,
    baselinePpm,
    "registration best ppm",
  );
  return {
    cameraRegistration: {
      fixedWorldMatrix: true,
      horizontalPixelsPerMeter: boundedNumber(
        cameraRegistration.horizontalPixelsPerMeter,
        0.001,
        1_000,
        "horizontal source scale",
      ),
      maximumScaleErrorPixelsPerMeter: boundedNumber(
        cameraRegistration.maximumScaleErrorPixelsPerMeter,
        0,
        1e-9,
        "source scale error",
      ),
      projectionCenterX: {
        left: boundedNumber(projectionCenter.left, -1, 1, "left projection center"),
        monolithic: boundedNumber(
          projectionCenter.monolithic,
          -1,
          1,
          "monolithic projection center",
        ),
        right: boundedNumber(projectionCenter.right, -1, 1, "right projection center"),
      },
      verticalPixelsPerMeter: boundedNumber(
        cameraRegistration.verticalPixelsPerMeter,
        0.001,
        1_000,
        "vertical source scale",
      ),
      worldMatrixSha256: cameraRegistration.worldMatrixSha256,
    },
    candidates,
    comparison: {
      boundary_structural_edge_pixels: integer(
        comparison.boundary_structural_edge_pixels,
        0,
        100_000_000,
        "boundary structural edges",
      ),
      failure_classifications: comparison.failure_classifications as string[],
      gates: {
        all_relations: gates.all_relations as boolean,
        lighting_seam: gates.lighting_seam as boolean,
        source: sourceGates as OverlapExperimentReport["comparison"]["gates"]["source"],
      },
      images: parsedImages,
      layers,
      passed: comparison.passed,
      registration_search: {
        baseline_above_tolerance_ppm: baselinePpm,
        best_above_tolerance_ppm: bestPpm,
        best_dx_px: integer(registration.best_dx_px, -2, 2, "registration horizontal offset"),
        best_dy_px: integer(registration.best_dy_px, -2, 2, "registration vertical offset"),
        observations_compared: integer(
          registration.observations_compared,
          1,
          500_000_000,
          "registration observations",
        ),
        radius_px: integer(registration.radius_px, 2, 2, "registration radius"),
      },
      schema: COMPARISON_SCHEMA,
    },
    grid: {
      cameraScreenRightBearingMillidegrees: integer(
        grid.cameraScreenRightBearingMillidegrees,
        0,
        359_999,
        "camera screen-right bearing",
      ),
      checkedSavedPixelCenters: integer(
        grid.checkedSavedPixelCenters,
        1,
        100_000_000,
        "checked grid pixel centers",
      ),
      maximumPixelCenterErrorPixels: boundedNumber(
        grid.maximumPixelCenterErrorPixels,
        0,
        0.5,
        "grid round-trip error",
      ),
    },
    network: {
      attempted,
      billableRootRequests: integer(network.billableRootRequests, 1, 1, "billable root request count"),
      blocked,
      completed,
      failed,
      formats: { glb: formats.glb, json: formats.json },
      requestLimit,
      responseBodyBytes: integer(
        network.responseBodyBytes,
        1,
        2_000_000_000,
        "Google response bytes",
      ),
      statuses,
    },
    runtime: {
      ingestWorkerMaxRssBytes: integer(
        runtime.ingestWorkerMaxRssBytes,
        1,
        16_000_000_000,
        "ingest worker RSS",
      ),
      nodeMaxRssBytes: integer(runtime.nodeMaxRssBytes, 1, 16_000_000_000, "Node RSS"),
      processTree: {
        peakProcessTreeRssBytes: integer(
          processTreePeak.treeBytes,
          1,
          64_000_000_000,
          "process tree RSS",
        ),
      },
      workerEnvelopeBytes: integer(
        runtime.workerEnvelopeBytes,
        1,
        16_000_000_000,
        "worker envelope",
      ),
    },
    schema: REPORT_SCHEMA,
  };
}

async function readBounded(response: Response, limit: number, expected?: number): Promise<Uint8Array> {
  if (!response.ok || response.redirected) {
    throw new Error(`overlap evidence request failed with status ${response.status}`);
  }
  const declared = response.headers.get("content-length");
  if (declared !== null) {
    const length = Number(declared);
    if (!Number.isSafeInteger(length) || length > limit || (expected !== undefined && length !== expected)) {
      throw new Error("overlap evidence declared an invalid byte length");
    }
  }
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length > limit || (expected !== undefined && bytes.length !== expected)) {
    throw new Error("overlap evidence has an invalid byte length");
  }
  return bytes;
}

async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const copy = new Uint8Array(bytes.length);
  copy.set(bytes);
  const digest = await crypto.subtle.digest("SHA-256", copy.buffer);
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function inspectPng(bytes: Uint8Array, image: OverlapEvidenceImage, id: string): void {
  if (
    bytes.length < 33 ||
    PNG_SIGNATURE.some((byte, index) => bytes[index] !== byte) ||
    new TextDecoder().decode(bytes.subarray(12, 16)) !== "IHDR"
  ) {
    throw new Error(`${id} is not a portable PNG`);
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (view.getUint32(16, false) !== image.width_px || view.getUint32(20, false) !== image.height_px) {
    throw new Error(`${id} PNG dimensions contradict the overlap report`);
  }
}

export async function loadOverlapEvidence(
  reportUrl: string,
  signal?: AbortSignal,
  fetcher: Fetcher = fetch,
): Promise<LoadedOverlapEvidence> {
  const reportBytes = await readBounded(
    await fetcher(reportUrl, { cache: "no-store", redirect: "error", signal }),
    MAX_REPORT_BYTES,
  );
  const report = parseReport(JSON.parse(new TextDecoder().decode(reportBytes)));
  const images = new Map<OverlapImageId, LoadedOverlapImage>();
  for (const id of OVERLAP_IMAGE_IDS_ALL) {
    const record = report.comparison.images[id];
    const url = new URL(`comparison/${record.path}`, new URL(reportUrl, window.location.href));
    const bytes = await readBounded(
      await fetcher(url, { cache: "no-store", redirect: "error", signal }),
      MAX_IMAGE_BYTES,
      record.byte_length,
    );
    if ((await sha256Hex(bytes)) !== record.sha256) {
      throw new Error(`${id} SHA-256 does not match the overlap report`);
    }
    inspectPng(bytes, record, id);
    images.set(id, { bytes, id, record });
  }
  return { images, report, reportUrl };
}

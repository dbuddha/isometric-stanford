export const CAPTURE_SCHEMA = "isometric-reference-capture/v1";
export const REQUIRED_LAYER_NAMES = [
  "color",
  "whitebox",
  "linear-depth",
  "view-normal",
  "fixed-shadow",
  "coverage",
] as const;

export type LayerName = (typeof REQUIRED_LAYER_NAMES)[number];

export interface TileSpec {
  regionId: string;
  column: number;
  row: number;
  coreWidthPx: number;
  coreHeightPx: number;
  guardPx: number;
  millimetersPerPixel: number;
  centerLongitudeE7: number;
  centerLatitudeE7: number;
}

export interface CameraSpec {
  projection: "orthographic";
  azimuthMillidegrees: number;
  elevationMillidegrees: number;
  targetAltitudeMm: number;
  nearMm: number;
  farMm: number;
  orthographicWidthMm: number;
  orthographicHeightMm: number;
  cameraDistanceMm: number;
}

export interface LightingSpec {
  sunAzimuthMillidegrees: number;
  sunElevationMillidegrees: number;
}

export interface ReadinessSpec {
  timeoutMs: number;
  stableFrames: number;
  stableDurationMs: number;
  minimumVisibleTiles: number;
}

export interface CaptureRequest {
  schema: typeof CAPTURE_SCHEMA;
  bundleId: string;
  provider: "synthetic" | "google-photorealistic-3d-tiles";
  sourceEpoch: string;
  tile: TileSpec;
  camera: CameraSpec;
  lighting: LightingSpec;
  readiness: ReadinessSpec;
}

export interface CaptureEvidence {
  attributions: string[];
  cameraFingerprint: string;
  complete: boolean;
  coreCoverageBasisPoints: number;
  elapsedMs: number;
  layerOrder: LayerName[];
  stableFrames: number;
  visibleTiles: number;
}

export interface SceneDiagnostics {
  cachedBytes: number;
  cachedTiles: number;
  errorTarget: number;
  geometries: number;
  maxCachedBytes: number;
  textures: number;
  triangles: number;
}

export interface ProbeCandidate {
  candidateId: string;
  request: CaptureRequest;
  upload: UploadTarget;
}

export interface ProbeCandidateEvidence extends CaptureEvidence {
  cameraWorldMatrix: number[];
  candidateId: string;
  diagnostics: SceneDiagnostics;
  projectionMatrix: number[];
}

export interface UploadTarget {
  token: string;
  url: string;
}

function isIntegerIn(value: unknown, minimum: number, maximum: number): value is number {
  return Number.isInteger(value) && Number(value) >= minimum && Number(value) <= maximum;
}

function isSafeIdentifier(value: unknown): value is string {
  return typeof value === "string" && /^[a-z0-9-]{1,64}$/.test(value);
}

export function totalWidth(request: CaptureRequest): number {
  return request.tile.coreWidthPx + 2 * request.tile.guardPx;
}

export function totalHeight(request: CaptureRequest): number {
  return request.tile.coreHeightPx + 2 * request.tile.guardPx;
}

export function cameraFingerprint(request: CaptureRequest): string {
  const values = [
    request.tile.centerLongitudeE7,
    request.tile.centerLatitudeE7,
    totalWidth(request),
    totalHeight(request),
    request.tile.millimetersPerPixel,
    request.camera.azimuthMillidegrees,
    request.camera.elevationMillidegrees,
    request.camera.targetAltitudeMm,
    request.camera.nearMm,
    request.camera.farMm,
    request.camera.orthographicWidthMm,
    request.camera.orthographicHeightMm,
    request.camera.cameraDistanceMm,
  ];
  return values.join(":");
}

export function validateCaptureRequest(value: unknown): asserts value is CaptureRequest {
  if (typeof value !== "object" || value === null) {
    throw new Error("capture request must be an object");
  }
  const request = value as Partial<CaptureRequest>;
  if (
    request.schema !== CAPTURE_SCHEMA ||
    !isSafeIdentifier(request.bundleId) ||
    (request.provider !== "synthetic" &&
      request.provider !== "google-photorealistic-3d-tiles") ||
    typeof request.sourceEpoch !== "string" ||
    request.sourceEpoch.length === 0 ||
    request.tile === undefined ||
    request.camera === undefined ||
    request.lighting === undefined ||
    request.readiness === undefined
  ) {
    throw new Error("capture request identity is invalid");
  }
  const tile = request.tile;
  if (
    !isSafeIdentifier(tile.regionId) ||
    !isIntegerIn(tile.column, -1_000_000, 1_000_000) ||
    !isIntegerIn(tile.row, -1_000_000, 1_000_000) ||
    !isIntegerIn(tile.coreWidthPx, 1, 3_584) ||
    !isIntegerIn(tile.coreHeightPx, 1, 3_584) ||
    !isIntegerIn(tile.guardPx, 1, 1_024) ||
    !isIntegerIn(tile.millimetersPerPixel, 1, 100_000) ||
    !isIntegerIn(tile.centerLongitudeE7, -1_800_000_000, 1_800_000_000) ||
    !isIntegerIn(tile.centerLatitudeE7, -900_000_000, 900_000_000) ||
    totalWidth(request as CaptureRequest) > 4_096 ||
    totalHeight(request as CaptureRequest) > 4_096
  ) {
    throw new Error("capture tile contract is invalid");
  }
  const camera = request.camera;
  if (
    camera.projection !== "orthographic" ||
    !isIntegerIn(camera.azimuthMillidegrees, 0, 359_999) ||
    !isIntegerIn(camera.elevationMillidegrees, 1_000, 89_999) ||
    !Number.isSafeInteger(camera.targetAltitudeMm) ||
    !isIntegerIn(camera.nearMm, 1, Number.MAX_SAFE_INTEGER) ||
    !isIntegerIn(camera.farMm, camera.nearMm + 1, Number.MAX_SAFE_INTEGER) ||
    !isIntegerIn(camera.cameraDistanceMm, camera.nearMm + 1, camera.farMm - 1)
  ) {
    throw new Error("capture camera contract is invalid");
  }
  const expectedWidthMm = totalWidth(request as CaptureRequest) * tile.millimetersPerPixel;
  const expectedHeightMm = totalHeight(request as CaptureRequest) * tile.millimetersPerPixel;
  if (
    camera.orthographicWidthMm !== expectedWidthMm ||
    camera.orthographicHeightMm !== expectedHeightMm
  ) {
    throw new Error("orthographic span does not match the registered grid");
  }
  const lighting = request.lighting;
  if (
    !isIntegerIn(lighting.sunAzimuthMillidegrees, 0, 359_999) ||
    !isIntegerIn(lighting.sunElevationMillidegrees, 1_000, 89_999)
  ) {
    throw new Error("capture lighting contract is invalid");
  }
  const readiness = request.readiness;
  if (
    !isIntegerIn(readiness.timeoutMs, 1_000, 300_000) ||
    !isIntegerIn(readiness.stableFrames, 2, 600) ||
    !isIntegerIn(readiness.stableDurationMs, 100, 30_000) ||
    !isIntegerIn(readiness.minimumVisibleTiles, 1, 100_000)
  ) {
    throw new Error("capture readiness contract is invalid");
  }
}

export function redactSecrets(message: string, secrets: readonly string[]): string {
  let redacted = message.replaceAll(
    /([?&](?:key|token|api_key|session)=)[^&\s]+/gi,
    "$1[REDACTED]",
  );
  for (const secret of secrets) {
    if (secret.length >= 6) {
      redacted = redacted.replaceAll(secret, "[REDACTED]");
    }
  }
  return redacted;
}

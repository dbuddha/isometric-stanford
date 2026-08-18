export const REFERENCE_SCHEMA = "isometric-reference-manifest/v2";

export const REFERENCE_LAYERS = [
  "color",
  "whitebox",
  "linear-depth",
  "view-normal",
  "fixed-shadow",
  "coverage",
] as const;

export type ReferenceLayerKind = (typeof REFERENCE_LAYERS)[number];

export const REFERENCE_LAYER_LABELS: Record<ReferenceLayerKind, string> = {
  color: "Google color",
  whitebox: "Whitebox geometry",
  "linear-depth": "Linear depth",
  "view-normal": "Surface normals",
  "fixed-shadow": "Fixed shadow",
  coverage: "Source coverage",
};

const LAYER_CONTRACT: Record<
  ReferenceLayerKind,
  { colorType?: 0 | 6; encoding: string; filename: string }
> = {
  color: { colorType: 6, encoding: "png-rgba8", filename: "color.png" },
  whitebox: { colorType: 6, encoding: "png-rgba8", filename: "whitebox.png" },
  "linear-depth": {
    encoding: "raw-u32le-millimeters",
    filename: "depth.bin",
  },
  "view-normal": { colorType: 6, encoding: "png-rgba8", filename: "normal.png" },
  "fixed-shadow": { colorType: 0, encoding: "png-gray8", filename: "fixed-shadow.png" },
  coverage: { colorType: 0, encoding: "png-gray8", filename: "coverage.png" },
};

const MAX_MANIFEST_BYTES = 256 * 1024;
const MAX_LAYER_BYTES = 80 * 1024 * 1024;
const MAX_TOTAL_LAYER_BYTES = REFERENCE_LAYERS.length * MAX_LAYER_BYTES;
const PNG_SIGNATURE = [137, 80, 78, 71, 13, 10, 26, 10] as const;
const DEPTH_MAGIC = "ISOD32V1";

export interface ReferenceLayerRecord {
  kind: ReferenceLayerKind;
  path: string;
  encoding: string;
  width_px: number;
  height_px: number;
  byte_length: number;
  sha256: string;
}

export interface ReferenceManifest {
  schema: typeof REFERENCE_SCHEMA;
  bundle_id: string;
  tile: {
    region_id: string;
    column: number;
    row: number;
    core_width_px: number;
    core_height_px: number;
    guard_px: number;
    millimeters_per_pixel: number;
    center_longitude_e7: number;
    center_latitude_e7: number;
  };
  camera: {
    projection: "orthographic";
    azimuth_millidegrees: number;
    elevation_millidegrees: number;
    target_altitude_mm: number;
    near_mm: number;
    far_mm: number;
    orthographic_width_mm: number;
    orthographic_height_mm: number;
    camera_distance_mm: number;
  };
  lighting: {
    sun_azimuth_millidegrees: number;
    sun_elevation_millidegrees: number;
  };
  capture: {
    renderer: string;
    renderer_version: string;
    provider: string;
    source_epoch: string;
    complete: true;
    attributions: string[];
  };
  core_coverage_basis_points: number;
  layers: ReferenceLayerRecord[];
}

export interface LoadedReferenceLayer {
  bytes: Uint8Array;
  record: ReferenceLayerRecord;
}

export interface LoadedReferenceBundle {
  layers: ReadonlyMap<ReferenceLayerKind, LoadedReferenceLayer>;
  manifest: ReferenceManifest;
  manifestSha256: string;
  manifestUrl: string;
  totalLayerBytes: number;
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

function text(value: unknown, pattern: RegExp, name: string): string {
  if (typeof value !== "string" || !pattern.test(value)) {
    throw new Error(`${name} is invalid`);
  }
  return value;
}

function lowercaseSha256(value: unknown, name: string): string {
  return text(value, /^[0-9a-f]{64}$/, name);
}

function requiredRecord(parent: Record<string, unknown>, key: string): Record<string, unknown> {
  const value = record(parent[key]);
  if (!value) {
    throw new Error(`${key} must be an object`);
  }
  return value;
}

function parseLayer(
  value: unknown,
  expectedWidth: number,
  expectedHeight: number,
): ReferenceLayerRecord {
  const layer = record(value);
  if (!layer || !REFERENCE_LAYERS.includes(layer.kind as ReferenceLayerKind)) {
    throw new Error("reference layer kind is invalid");
  }
  const kind = layer.kind as ReferenceLayerKind;
  const contract = LAYER_CONTRACT[kind];
  const path = text(layer.path, /^[a-z0-9][a-z0-9.-]{0,63}$/, `${kind} path`);
  if (path !== contract.filename || layer.encoding !== contract.encoding) {
    throw new Error(`${kind} does not match its portable layer contract`);
  }
  const width = integer(layer.width_px, 1, 4_096, `${kind} width`);
  const height = integer(layer.height_px, 1, 4_096, `${kind} height`);
  if (width !== expectedWidth || height !== expectedHeight) {
    throw new Error(`${kind} is not registered to the shared pixel grid`);
  }
  return {
    kind,
    path,
    encoding: contract.encoding,
    width_px: width,
    height_px: height,
    byte_length: integer(layer.byte_length, 1, MAX_LAYER_BYTES, `${kind} byte length`),
    sha256: lowercaseSha256(layer.sha256, `${kind} SHA-256`),
  };
}

export function parseReferenceManifest(value: unknown): ReferenceManifest {
  const manifest = record(value);
  if (!manifest || manifest.schema !== REFERENCE_SCHEMA) {
    throw new Error("reference manifest schema is unsupported");
  }
  const tile = requiredRecord(manifest, "tile");
  const coreWidth = integer(tile.core_width_px, 1, 3_584, "core width");
  const coreHeight = integer(tile.core_height_px, 1, 3_584, "core height");
  const guard = integer(tile.guard_px, 1, 1_024, "guard");
  const expectedWidth = coreWidth + 2 * guard;
  const expectedHeight = coreHeight + 2 * guard;
  if (expectedWidth > 4_096 || expectedHeight > 4_096) {
    throw new Error("registered reference dimensions exceed the bounded surface");
  }

  const rawLayers = manifest.layers;
  if (!Array.isArray(rawLayers) || rawLayers.length !== REFERENCE_LAYERS.length) {
    throw new Error("reference manifest must contain exactly six registered layers");
  }
  const layers = rawLayers.map((layer) => parseLayer(layer, expectedWidth, expectedHeight));
  for (const [index, kind] of REFERENCE_LAYERS.entries()) {
    if (layers[index]?.kind !== kind) {
      throw new Error("reference layers are missing, duplicated, or out of canonical order");
    }
  }
  const totalLayerBytes = layers.reduce((sum, layer) => sum + layer.byte_length, 0);
  if (totalLayerBytes > MAX_TOTAL_LAYER_BYTES) {
    throw new Error("reference bundle exceeds its bounded byte budget");
  }

  const camera = requiredRecord(manifest, "camera");
  const lighting = requiredRecord(manifest, "lighting");
  const capture = requiredRecord(manifest, "capture");
  const attributions = capture.attributions;
  if (
    camera.projection !== "orthographic" ||
    capture.complete !== true ||
    !Array.isArray(attributions) ||
    attributions.length === 0 ||
    attributions.some((item) => typeof item !== "string" || item.length === 0 || item.length > 512)
  ) {
    throw new Error("reference camera, completion, or attribution evidence is invalid");
  }
  const millimetersPerPixel = integer(
    tile.millimeters_per_pixel,
    1,
    100_000,
    "millimeters per pixel",
  );
  const orthographicWidth = integer(
    camera.orthographic_width_mm,
    1,
    Number.MAX_SAFE_INTEGER,
    "orthographic width",
  );
  const orthographicHeight = integer(
    camera.orthographic_height_mm,
    1,
    Number.MAX_SAFE_INTEGER,
    "orthographic height",
  );
  const near = integer(camera.near_mm, 1, Number.MAX_SAFE_INTEGER, "camera near plane");
  const far = integer(camera.far_mm, 2, Number.MAX_SAFE_INTEGER, "camera far plane");
  const distance = integer(
    camera.camera_distance_mm,
    1,
    Number.MAX_SAFE_INTEGER,
    "camera distance",
  );
  if (
    orthographicWidth !== expectedWidth * millimetersPerPixel ||
    orthographicHeight !== expectedHeight * millimetersPerPixel ||
    near >= far ||
    distance <= near ||
    distance >= far
  ) {
    throw new Error("reference camera clipping or span does not match the registered grid");
  }

  return {
    schema: REFERENCE_SCHEMA,
    bundle_id: text(manifest.bundle_id, /^[a-z0-9-]{1,64}$/, "bundle ID"),
    tile: {
      region_id: text(tile.region_id, /^[a-z0-9-]{1,64}$/, "region ID"),
      column: integer(tile.column, -1_000_000, 1_000_000, "tile column"),
      row: integer(tile.row, -1_000_000, 1_000_000, "tile row"),
      core_width_px: coreWidth,
      core_height_px: coreHeight,
      guard_px: guard,
      millimeters_per_pixel: millimetersPerPixel,
      center_longitude_e7: integer(
        tile.center_longitude_e7,
        -1_800_000_000,
        1_800_000_000,
        "center longitude",
      ),
      center_latitude_e7: integer(
        tile.center_latitude_e7,
        -900_000_000,
        900_000_000,
        "center latitude",
      ),
    },
    camera: {
      projection: "orthographic",
      azimuth_millidegrees: integer(
        camera.azimuth_millidegrees,
        0,
        359_999,
        "camera azimuth",
      ),
      elevation_millidegrees: integer(
        camera.elevation_millidegrees,
        1_000,
        89_999,
        "camera elevation",
      ),
      target_altitude_mm: integer(
        camera.target_altitude_mm,
        Number.MIN_SAFE_INTEGER,
        Number.MAX_SAFE_INTEGER,
        "camera target altitude",
      ),
      near_mm: near,
      far_mm: far,
      orthographic_width_mm: orthographicWidth,
      orthographic_height_mm: orthographicHeight,
      camera_distance_mm: distance,
    },
    lighting: {
      sun_azimuth_millidegrees: integer(
        lighting.sun_azimuth_millidegrees,
        0,
        359_999,
        "sun azimuth",
      ),
      sun_elevation_millidegrees: integer(
        lighting.sun_elevation_millidegrees,
        1_000,
        89_999,
        "sun elevation",
      ),
    },
    capture: {
      renderer: text(capture.renderer, /^[a-z0-9][a-z0-9+._-]{0,127}$/, "renderer"),
      renderer_version: text(
        capture.renderer_version,
        /^[a-zA-Z0-9][a-zA-Z0-9+._-]{0,255}$/,
        "renderer version",
      ),
      provider: text(capture.provider, /^[a-z0-9][a-z0-9-]{0,127}$/, "provider"),
      source_epoch: text(capture.source_epoch, /^.{1,128}$/, "source epoch"),
      complete: true,
      attributions: [...(attributions as string[])],
    },
    core_coverage_basis_points: integer(
      manifest.core_coverage_basis_points,
      9_950,
      10_000,
      "core coverage",
    ),
    layers,
  };
}

async function sha256Hex(bytes: Uint8Array): Promise<string> {
  if (!globalThis.crypto?.subtle) {
    throw new Error("browser SHA-256 is unavailable");
  }
  const buffer = new ArrayBuffer(bytes.length);
  new Uint8Array(buffer).set(bytes);
  const digest = await globalThis.crypto.subtle.digest("SHA-256", buffer);
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

async function readBounded(response: Response, limit: number, expected?: number): Promise<Uint8Array> {
  if (!response.ok || response.redirected) {
    throw new Error(`reference artifact request failed with status ${response.status}`);
  }
  const declared = response.headers.get("content-length");
  if (declared !== null) {
    const declaredLength = Number(declared);
    if (!Number.isSafeInteger(declaredLength) || declaredLength > limit || (expected !== undefined && declaredLength !== expected)) {
      throw new Error("reference artifact declared an invalid byte length");
    }
  }
  if (!response.body) {
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (bytes.length > limit || (expected !== undefined && bytes.length !== expected)) {
      throw new Error("reference artifact has an invalid byte length");
    }
    return bytes;
  }
  const chunks: Uint8Array[] = [];
  const reader = response.body.getReader();
  let total = 0;
  try {
    while (true) {
      const result = await reader.read();
      if (result.done) {
        break;
      }
      total += result.value.length;
      if (total > limit || (expected !== undefined && total > expected)) {
        await reader.cancel();
        throw new Error("reference artifact exceeded its bounded byte length");
      }
      chunks.push(result.value);
    }
  } finally {
    reader.releaseLock();
  }
  if (expected !== undefined && total !== expected) {
    throw new Error("reference artifact has an invalid byte length");
  }
  const bytes = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.length;
  }
  return bytes;
}

function inspectPng(bytes: Uint8Array, record: ReferenceLayerRecord, expectedColorType: 0 | 6): void {
  if (
    bytes.length < 33 ||
    PNG_SIGNATURE.some((byte, index) => bytes[index] !== byte) ||
    new TextDecoder().decode(bytes.subarray(12, 16)) !== "IHDR"
  ) {
    throw new Error(`${record.kind} is not a portable PNG`);
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (
    view.getUint32(16, false) !== record.width_px ||
    view.getUint32(20, false) !== record.height_px ||
    bytes[24] !== 8 ||
    bytes[25] !== expectedColorType
  ) {
    throw new Error(`${record.kind} PNG header does not match its manifest`);
  }
}

function inspectDepth(bytes: Uint8Array, record: ReferenceLayerRecord): void {
  if (bytes.length !== 16 + record.width_px * record.height_px * 4) {
    throw new Error("linear-depth byte length does not match its registered grid");
  }
  const magic = new TextDecoder().decode(bytes.subarray(0, 8));
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (
    magic !== DEPTH_MAGIC ||
    view.getUint32(8, true) !== record.width_px ||
    view.getUint32(12, true) !== record.height_px
  ) {
    throw new Error("linear-depth header does not match its manifest");
  }
}

function inspectLayer(bytes: Uint8Array, layer: ReferenceLayerRecord): void {
  const contract = LAYER_CONTRACT[layer.kind];
  if (contract.colorType === undefined) {
    inspectDepth(bytes, layer);
  } else {
    inspectPng(bytes, layer, contract.colorType);
  }
}

export async function loadReferenceBundle(
  manifestLocation: string,
  signal?: AbortSignal,
  fetcher: Fetcher = fetch,
): Promise<LoadedReferenceBundle> {
  const manifestUrl = new URL(manifestLocation, window.location.href);
  const manifestBytes = await readBounded(
    await fetcher(manifestUrl, { cache: "no-store", signal }),
    MAX_MANIFEST_BYTES,
  );
  let decoded: unknown;
  try {
    decoded = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(manifestBytes));
  } catch {
    throw new Error("reference manifest is not valid UTF-8 JSON");
  }
  const manifest = parseReferenceManifest(decoded);
  const layers = new Map<ReferenceLayerKind, LoadedReferenceLayer>();
  let totalLayerBytes = 0;
  for (const layer of manifest.layers) {
    const layerUrl = new URL(`./${layer.path}`, manifestUrl);
    if (layerUrl.origin !== manifestUrl.origin) {
      throw new Error(`${layer.kind} escapes the reference bundle origin`);
    }
    const bytes = await readBounded(
      await fetcher(layerUrl, { cache: "no-store", signal }),
      MAX_LAYER_BYTES,
      layer.byte_length,
    );
    if ((await sha256Hex(bytes)) !== layer.sha256) {
      throw new Error(`${layer.kind} SHA-256 does not match the manifest`);
    }
    inspectLayer(bytes, layer);
    totalLayerBytes += bytes.length;
    layers.set(layer.kind, { bytes, record: layer });
  }
  return {
    layers,
    manifest,
    manifestSha256: await sha256Hex(manifestBytes),
    manifestUrl: manifestUrl.href,
    totalLayerBytes,
  };
}

export function depthPreviewPixels(layer: LoadedReferenceLayer): Uint8ClampedArray {
  if (layer.record.kind !== "linear-depth") {
    throw new Error("depth preview requires the linear-depth layer");
  }
  inspectDepth(layer.bytes, layer.record);
  const view = new DataView(layer.bytes.buffer, layer.bytes.byteOffset, layer.bytes.byteLength);
  let minimum = Number.MAX_SAFE_INTEGER;
  let maximum = 0;
  for (let offset = 16; offset < layer.bytes.length; offset += 4) {
    const value = view.getUint32(offset, true);
    if (value > 0) {
      minimum = Math.min(minimum, value);
      maximum = Math.max(maximum, value);
    }
  }
  const output = new Uint8ClampedArray(layer.record.width_px * layer.record.height_px * 4);
  const range = Math.max(1, maximum - minimum);
  for (let offset = 16, target = 0; offset < layer.bytes.length; offset += 4, target += 4) {
    const value = view.getUint32(offset, true);
    const tone = value === 0 ? 0 : 255 - Math.round(((value - minimum) * 220) / range);
    output[target] = tone;
    output[target + 1] = tone;
    output[target + 2] = tone;
    output[target + 3] = 255;
  }
  return output;
}

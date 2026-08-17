export interface ReleaseMetadata {
  status: "artifact-candidate";
  qualified: false;
  styleId: string;
  styleSha256: string;
  worldSha256: string;
  width: number;
  height: number;
  tileCount: number;
  encodedBytes: number;
  tileSetSha256: string;
}

interface UnknownRecord {
  [key: string]: unknown;
}

function record(value: unknown): UnknownRecord | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as UnknownRecord)
    : null;
}

function positiveInteger(value: unknown): value is number {
  return Number.isInteger(value) && Number(value) > 0;
}

function lowercaseSha256(value: unknown): value is string {
  return typeof value === "string" && /^[0-9a-f]{64}$/.test(value);
}

export function parseReleaseMetadata(value: unknown): ReleaseMetadata {
  const manifest = record(value);
  const dzi = record(manifest?.dzi);
  if (
    manifest?.schema !== "isometric-release/v1" ||
    manifest.status !== "artifact-candidate" ||
    manifest.qualified !== false ||
    typeof manifest.style_id !== "string" ||
    !manifest.style_id.startsWith("stanford_v1.") ||
    !lowercaseSha256(manifest.style_sha256) ||
    !lowercaseSha256(manifest.world_sha256) ||
    dzi?.descriptor !== "hero.dzi" ||
    dzi.format !== "webp" ||
    dzi.tile_size !== 512 ||
    dzi.overlap !== 0 ||
    !positiveInteger(dzi.width) ||
    !positiveInteger(dzi.height) ||
    !positiveInteger(dzi.tile_count) ||
    !positiveInteger(dzi.encoded_bytes) ||
    !lowercaseSha256(dzi.tile_set_sha256)
  ) {
    throw new Error("release metadata is invalid or claims qualification");
  }
  return {
    status: "artifact-candidate",
    qualified: false,
    styleId: manifest.style_id,
    styleSha256: manifest.style_sha256,
    worldSha256: manifest.world_sha256,
    width: dzi.width,
    height: dzi.height,
    tileCount: dzi.tile_count,
    encodedBytes: dzi.encoded_bytes,
    tileSetSha256: dzi.tile_set_sha256,
  };
}

export async function loadReleaseMetadata(url: string): Promise<ReleaseMetadata> {
  const response = await fetch(url, { cache: "no-cache" });
  if (!response.ok) {
    throw new Error(`release metadata request failed with ${response.status}`);
  }
  return parseReleaseMetadata(await response.json());
}

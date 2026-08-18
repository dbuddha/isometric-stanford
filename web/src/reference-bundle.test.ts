import {
  REFERENCE_LAYERS,
  depthPreviewPixels,
  loadReferenceBundle,
  parseReferenceManifest,
  type ReferenceLayerKind,
} from "./reference-bundle";
import { describe, expect, it, vi } from "vitest";

const FILENAMES: Record<ReferenceLayerKind, string> = {
  color: "color.png",
  whitebox: "whitebox.png",
  "linear-depth": "depth.bin",
  "view-normal": "normal.png",
  "fixed-shadow": "fixed-shadow.png",
  coverage: "coverage.png",
};

const ENCODINGS: Record<ReferenceLayerKind, string> = {
  color: "png-rgba8",
  whitebox: "png-rgba8",
  "linear-depth": "raw-u32le-millimeters",
  "view-normal": "png-rgba8",
  "fixed-shadow": "png-gray8",
  coverage: "png-gray8",
};

function pngHeader(width: number, height: number, colorType: 0 | 6): Uint8Array {
  const bytes = new Uint8Array(33);
  bytes.set([137, 80, 78, 71, 13, 10, 26, 10]);
  const view = new DataView(bytes.buffer);
  view.setUint32(8, 13, false);
  bytes.set(new TextEncoder().encode("IHDR"), 12);
  view.setUint32(16, width, false);
  view.setUint32(20, height, false);
  bytes[24] = 8;
  bytes[25] = colorType;
  return bytes;
}

function depth(width: number, height: number): Uint8Array {
  const bytes = new Uint8Array(16 + width * height * 4);
  bytes.set(new TextEncoder().encode("ISOD32V1"));
  const view = new DataView(bytes.buffer);
  view.setUint32(8, width, true);
  view.setUint32(12, height, true);
  for (let index = 0; index < width * height; index += 1) {
    view.setUint32(16 + index * 4, index === 0 ? 0 : 1_000 + index * 250, true);
  }
  return bytes;
}

async function digest(bytes: Uint8Array): Promise<string> {
  const buffer = new ArrayBuffer(bytes.length);
  new Uint8Array(buffer).set(bytes);
  const value = await crypto.subtle.digest("SHA-256", buffer);
  return Array.from(new Uint8Array(value), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function body(bytes: Uint8Array): ArrayBuffer {
  const buffer = new ArrayBuffer(bytes.length);
  new Uint8Array(buffer).set(bytes);
  return buffer;
}

async function fixture() {
  const width = 3;
  const height = 3;
  const artifacts = new Map<ReferenceLayerKind, Uint8Array>();
  for (const kind of REFERENCE_LAYERS) {
    artifacts.set(
      kind,
      kind === "linear-depth"
        ? depth(width, height)
        : pngHeader(
            width,
            height,
            kind === "fixed-shadow" || kind === "coverage" ? 0 : 6,
          ),
    );
  }
  const layers = await Promise.all(
    REFERENCE_LAYERS.map(async (kind) => {
      const bytes = artifacts.get(kind);
      if (!bytes) {
        throw new Error("fixture layer missing");
      }
      return {
        kind,
        path: FILENAMES[kind],
        encoding: ENCODINGS[kind],
        width_px: width,
        height_px: height,
        byte_length: bytes.length,
        sha256: await digest(bytes),
      };
    }),
  );
  const manifest = {
    schema: "isometric-reference-manifest/v2",
    bundle_id: "test-bundle",
    tile: {
      region_id: "test-region",
      column: 0,
      row: 0,
      core_width_px: 1,
      core_height_px: 1,
      guard_px: 1,
      millimeters_per_pixel: 250,
      center_longitude_e7: -1_221_697_000,
      center_latitude_e7: 374_275_000,
    },
    camera: {
      projection: "orthographic",
      azimuth_millidegrees: 315_000,
      elevation_millidegrees: 42_000,
      target_altitude_mm: 20_000,
      near_mm: 100,
      far_mm: 4_000_000,
      orthographic_width_mm: 750,
      orthographic_height_mm: 750,
      camera_distance_mm: 2_000_000,
    },
    lighting: {
      sun_azimuth_millidegrees: 315_000,
      sun_elevation_millidegrees: 42_000,
    },
    capture: {
      renderer: "threejs-synthetic-fixture",
      renderer_version: "capture-v1+test",
      provider: "synthetic",
      source_epoch: "fixture",
      complete: true,
      attributions: ["fixture:synthetic"],
    },
    core_coverage_basis_points: 10_000,
    layers,
  };
  return { artifacts, manifest };
}

describe("registered reference bundle", () => {
  it("accepts the exact six-layer camera contract", async () => {
    const { manifest } = await fixture();
    expect(parseReferenceManifest(manifest)).toMatchObject({
      bundle_id: "test-bundle",
      core_coverage_basis_points: 10_000,
    });
  });

  it("rejects missing, escaped, and misregistered layers", async () => {
    const { manifest } = await fixture();
    expect(() => parseReferenceManifest({ ...manifest, layers: manifest.layers.slice(1) })).toThrow(
      /exactly six/,
    );
    const escaped = structuredClone(manifest);
    escaped.layers[0]!.path = "../color.png";
    expect(() => parseReferenceManifest(escaped)).toThrow(/path/);
    const shifted = structuredClone(manifest);
    shifted.layers[0]!.width_px = 2;
    expect(() => parseReferenceManifest(shifted)).toThrow(/shared pixel grid/);
  });

  it("streams and verifies every registered artifact before exposing a bundle", async () => {
    const { artifacts, manifest } = await fixture();
    const manifestBytes = new TextEncoder().encode(JSON.stringify(manifest));
    const fetcher = vi.fn(async (input: RequestInfo | URL) => {
      const url = new URL(String(input));
      if (url.pathname.endsWith("reference.manifest.json")) {
        return new Response(body(manifestBytes), {
          headers: { "content-length": String(manifestBytes.length) },
        });
      }
      const layer = manifest.layers.find((candidate) => url.pathname.endsWith(candidate.path));
      const bytes = layer ? artifacts.get(layer.kind) : undefined;
      return bytes
        ? new Response(body(bytes), { headers: { "content-length": String(bytes.length) } })
        : new Response("missing", { status: 404 });
    });
    const loaded = await loadReferenceBundle("/reference/reference.manifest.json", undefined, fetcher);
    expect(loaded.layers.size).toBe(6);
    expect(loaded.totalLayerBytes).toBe(manifest.layers.reduce((sum, layer) => sum + layer.byte_length, 0));
    expect(loaded.manifestSha256).toMatch(/^[0-9a-f]{64}$/);
    expect(fetcher).toHaveBeenCalledTimes(7);
  });

  it("fails closed on missing and hash-mismatched artifacts", async () => {
    const { artifacts, manifest } = await fixture();
    const manifestBytes = new TextEncoder().encode(JSON.stringify(manifest));
    const fetcher = vi.fn(async (input: RequestInfo | URL) => {
      const url = new URL(String(input));
      if (url.pathname.endsWith("reference.manifest.json")) {
        return new Response(body(manifestBytes));
      }
      if (url.pathname.endsWith("whitebox.png")) {
        return new Response("missing", { status: 404 });
      }
      const layer = manifest.layers.find((candidate) => url.pathname.endsWith(candidate.path));
      const bytes = layer ? artifacts.get(layer.kind) : undefined;
      return new Response(bytes ? body(Uint8Array.from(bytes, (byte) => byte ^ 1)) : "missing");
    });
    await expect(
      loadReferenceBundle("/reference/reference.manifest.json", undefined, fetcher),
    ).rejects.toThrow(/SHA-256|status 404/);
  });

  it("creates a deterministic near-is-bright depth preview", async () => {
    const { artifacts, manifest } = await fixture();
    const parsed = parseReferenceManifest(manifest);
    const record = parsed.layers.find((layer) => layer.kind === "linear-depth");
    const bytes = artifacts.get("linear-depth");
    if (!record || !bytes) {
      throw new Error("depth fixture missing");
    }
    const preview = depthPreviewPixels({ bytes, record });
    expect(preview).toHaveLength(3 * 3 * 4);
    expect(preview.slice(0, 4)).toEqual(Uint8ClampedArray.from([0, 0, 0, 255]));
    expect(preview[4]).toBeGreaterThan(preview.at(-4) ?? 255);
  });
});

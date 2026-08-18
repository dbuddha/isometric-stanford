import { createHash } from "node:crypto";
import type { Page } from "@playwright/test";

import { encodePng } from "../../capture/src/node/png.js";

const WIDTH = 64;
const HEIGHT = 64;
const MANIFEST_ROUTE = "**/fixture/reference/reference.manifest.json";
const LAYER_ROUTE = (path: string) => `**/fixture/reference/${path}`;

type LayerKind =
  | "color"
  | "whitebox"
  | "linear-depth"
  | "view-normal"
  | "fixed-shadow"
  | "coverage";

interface FixtureOptions {
  corruptManifest?: boolean;
  missingLayer?: string;
}

function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function rgba(kind: "color" | "whitebox" | "view-normal"): Uint8Array {
  const pixels = new Uint8Array(WIDTH * HEIGHT * 4);
  for (let y = 0; y < HEIGHT; y += 1) {
    for (let x = 0; x < WIDTH; x += 1) {
      const offset = (y * WIDTH + x) * 4;
      const tower = x >= 27 && x <= 37 && y >= 7 && y <= 46;
      const roof = y >= 43 && y <= 50 && x >= 18 && x <= 48;
      const road = y >= 51;
      let color: [number, number, number];
      if (kind === "view-normal") {
        color = tower ? [128, 220, 128] : roof ? [128, 128, 245] : [128, 128, 255];
      } else if (kind === "whitebox") {
        const tone = tower ? 218 : roof ? 195 : road ? 128 : 166;
        color = [tone, tone, tone];
      } else if (tower) {
        color = (x + y) % 6 < 3 ? [184, 122, 83] : [222, 184, 134];
      } else if (roof) {
        color = [187, 94, 62];
      } else if (road) {
        color = [75, 78, 75];
      } else {
        color = (x + y) % 7 === 0 ? [54, 91, 57] : [75, 112, 64];
      }
      pixels.set([...color, 255], offset);
    }
  }
  return pixels;
}

function gray(kind: "fixed-shadow" | "coverage"): Uint8Array {
  const pixels = new Uint8Array(WIDTH * HEIGHT);
  for (let y = 0; y < HEIGHT; y += 1) {
    for (let x = 0; x < WIDTH; x += 1) {
      pixels[y * WIDTH + x] =
        kind === "coverage" ? 255 : x > 36 && x - 36 > Math.max(0, y - 45) ? 82 : 220;
    }
  }
  return pixels;
}

function depth(): Uint8Array {
  const bytes = new Uint8Array(16 + WIDTH * HEIGHT * 4);
  bytes.set(new TextEncoder().encode("ISOD32V1"));
  const view = new DataView(bytes.buffer);
  view.setUint32(8, WIDTH, true);
  view.setUint32(12, HEIGHT, true);
  for (let y = 0; y < HEIGHT; y += 1) {
    for (let x = 0; x < WIDTH; x += 1) {
      const tower = x >= 27 && x <= 37 && y >= 7 && y <= 46;
      view.setUint32(16 + (y * WIDTH + x) * 4, tower ? 1_850_000 + y * 40 : 2_010_000 + y * 20, true);
    }
  }
  return bytes;
}

function fixture() {
  const artifacts = new Map<string, Buffer>();
  artifacts.set("color.png", encodePng(rgba("color"), WIDTH, HEIGHT, 6));
  artifacts.set("whitebox.png", encodePng(rgba("whitebox"), WIDTH, HEIGHT, 6));
  artifacts.set("depth.bin", Buffer.from(depth()));
  artifacts.set("normal.png", encodePng(rgba("view-normal"), WIDTH, HEIGHT, 6));
  artifacts.set("fixed-shadow.png", encodePng(gray("fixed-shadow"), WIDTH, HEIGHT, 0));
  artifacts.set("coverage.png", encodePng(gray("coverage"), WIDTH, HEIGHT, 0));
  const contracts: Array<{ kind: LayerKind; path: string; encoding: string }> = [
    { kind: "color", path: "color.png", encoding: "png-rgba8" },
    { kind: "whitebox", path: "whitebox.png", encoding: "png-rgba8" },
    { kind: "linear-depth", path: "depth.bin", encoding: "raw-u32le-millimeters" },
    { kind: "view-normal", path: "normal.png", encoding: "png-rgba8" },
    { kind: "fixed-shadow", path: "fixed-shadow.png", encoding: "png-gray8" },
    { kind: "coverage", path: "coverage.png", encoding: "png-gray8" },
  ];
  const manifest = {
    schema: "isometric-reference-manifest/v2",
    bundle_id: "hoover-review-fixture",
    tile: {
      region_id: "hoover-pilot",
      column: 0,
      row: 0,
      core_width_px: 48,
      core_height_px: 48,
      guard_px: 8,
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
      orthographic_width_mm: WIDTH * 250,
      orthographic_height_mm: HEIGHT * 250,
      camera_distance_mm: 2_000_000,
    },
    lighting: {
      sun_azimuth_millidegrees: 315_000,
      sun_elevation_millidegrees: 42_000,
    },
    capture: {
      renderer: "threejs-synthetic-fixture",
      renderer_version: "capture-v1+three-0.185.1+fixture",
      provider: "synthetic",
      source_epoch: "fixture-2026-08-18",
      complete: true,
      attributions: ["fixture:synthetic"],
    },
    core_coverage_basis_points: 10_000,
    layers: contracts.map((contract) => {
      const bytes = artifacts.get(contract.path);
      if (!bytes) {
        throw new Error(`fixture ${contract.path} is missing`);
      }
      return {
        ...contract,
        width_px: WIDTH,
        height_px: HEIGHT,
        byte_length: bytes.length,
        sha256: sha256(bytes),
      };
    }),
  };
  return { artifacts, manifest };
}

export async function installReferenceFixture(page: Page, options: FixtureOptions = {}) {
  const { artifacts, manifest } = fixture();
  if (options.corruptManifest) {
    manifest.layers[0]!.width_px -= 1;
  }
  const manifestBytes = Buffer.from(`${JSON.stringify(manifest)}\n`);
  await page.route(MANIFEST_ROUTE, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      headers: { "content-length": String(manifestBytes.length) },
      body: manifestBytes,
    }),
  );
  for (const [path, artifact] of artifacts) {
    await page.route(LAYER_ROUTE(path), (route) =>
      path === options.missingLayer
        ? route.fulfill({ status: 404, body: "missing" })
        : route.fulfill({
            status: 200,
            contentType: path.endsWith(".png") ? "image/png" : "application/octet-stream",
            headers: { "content-length": String(artifact.length) },
            body: artifact,
          }),
    );
  }
}

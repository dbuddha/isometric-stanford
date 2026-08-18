import { createHash, randomBytes } from "node:crypto";
import { mkdir, open, rename, rm, stat, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import {
  REQUIRED_LAYER_NAMES,
  cameraFingerprint,
  totalHeight,
  totalWidth,
} from "../contracts.js";
import type { CaptureEvidence, CaptureRequest, LayerName } from "../contracts.js";
import { encodePng } from "./png.js";

export type PixelFormat = "gray8" | "rgba8" | "u32le-millimeters";

interface AcceptedLayer {
  byteLength: number;
  encoding: string;
  filename: string;
  kind: string;
  sha256: string;
}

const LAYER_CONTRACT: Record<
  LayerName,
  { colorType?: 0 | 6; encoding: string; filename: string; kind: string; pixelFormat: PixelFormat }
> = {
  color: { colorType: 6, encoding: "png-rgba8", filename: "color.png", kind: "color", pixelFormat: "rgba8" },
  whitebox: {
    colorType: 6,
    encoding: "png-rgba8",
    filename: "whitebox.png",
    kind: "whitebox",
    pixelFormat: "rgba8",
  },
  "linear-depth": {
    encoding: "raw-u32le-millimeters",
    filename: "depth.bin",
    kind: "linear-depth",
    pixelFormat: "u32le-millimeters",
  },
  "view-normal": {
    colorType: 6,
    encoding: "png-rgba8",
    filename: "normal.png",
    kind: "view-normal",
    pixelFormat: "rgba8",
  },
  "fixed-shadow": {
    colorType: 0,
    encoding: "png-gray8",
    filename: "fixed-shadow.png",
    kind: "fixed-shadow",
    pixelFormat: "gray8",
  },
  coverage: {
    colorType: 0,
    encoding: "png-gray8",
    filename: "coverage.png",
    kind: "coverage",
    pixelFormat: "gray8",
  },
};

function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

export class BundleWriter {
  readonly #accepted = new Map<LayerName, AcceptedLayer>();
  readonly #outputDirectory: string;
  readonly #request: CaptureRequest;
  readonly #stagingDirectory: string;

  public constructor(outputDirectory: string, request: CaptureRequest) {
    this.#outputDirectory = resolve(outputDirectory);
    this.#request = request;
    const suffix = randomBytes(8).toString("hex");
    this.#stagingDirectory = resolve(
      dirname(this.#outputDirectory),
      `.capture-${request.bundleId}-${suffix}`,
    );
  }

  public get stagingDirectory(): string {
    return this.#stagingDirectory;
  }

  public async start(): Promise<void> {
    await mkdir(dirname(this.#outputDirectory), { recursive: true });
    try {
      await stat(this.#outputDirectory);
      throw new Error("capture output already exists; immutable bundles are never overwritten");
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
        throw error;
      }
    }
    await mkdir(this.#stagingDirectory, { recursive: false, mode: 0o700 });
  }

  public async accept(
    name: LayerName,
    pixels: Uint8Array,
    width: number,
    height: number,
    pixelFormat: PixelFormat,
  ): Promise<void> {
    const expectedName = REQUIRED_LAYER_NAMES[this.#accepted.size];
    const contract = LAYER_CONTRACT[name];
    if (
      name !== expectedName ||
      this.#accepted.has(name) ||
      contract.pixelFormat !== pixelFormat ||
      width !== totalWidth(this.#request) ||
      height !== totalHeight(this.#request)
    ) {
      throw new Error(`capture layer ${name} violates registration or ordering`);
    }
    let artifact: Uint8Array;
    if (contract.colorType === undefined) {
      const expectedLength = 16 + width * height * 4;
      if (pixels.length !== expectedLength || Buffer.from(pixels.subarray(0, 8)).toString() !== "ISOD32V1") {
        throw new Error("capture depth payload violates its portable contract");
      }
      artifact = pixels;
    } else {
      artifact = encodePng(pixels, width, height, contract.colorType);
    }
    await writeFile(resolve(this.#stagingDirectory, contract.filename), artifact, {
      flag: "wx",
      mode: 0o600,
    });
    this.#accepted.set(name, {
      byteLength: artifact.length,
      encoding: contract.encoding,
      filename: contract.filename,
      kind: contract.kind,
      sha256: sha256(artifact),
    });
  }

  public async finalize(
    evidence: CaptureEvidence,
    validate: (stagingDirectory: string) => Promise<void>,
  ): Promise<string> {
    if (
      this.#accepted.size !== REQUIRED_LAYER_NAMES.length ||
      !evidence.complete ||
      evidence.attributions.length === 0 ||
      evidence.cameraFingerprint !== cameraFingerprint(this.#request) ||
      JSON.stringify(evidence.layerOrder) !== JSON.stringify(REQUIRED_LAYER_NAMES)
    ) {
      throw new Error("capture evidence is incomplete or not registered to the requested camera");
    }
    const width = totalWidth(this.#request);
    const height = totalHeight(this.#request);
    const manifest = {
      schema: "isometric-reference-manifest/v2",
      bundle_id: this.#request.bundleId,
      tile: {
        region_id: this.#request.tile.regionId,
        column: this.#request.tile.column,
        row: this.#request.tile.row,
        core_width_px: this.#request.tile.coreWidthPx,
        core_height_px: this.#request.tile.coreHeightPx,
        guard_px: this.#request.tile.guardPx,
        millimeters_per_pixel: this.#request.tile.millimetersPerPixel,
        center_longitude_e7: this.#request.tile.centerLongitudeE7,
        center_latitude_e7: this.#request.tile.centerLatitudeE7,
      },
      camera: {
        projection: this.#request.camera.projection,
        azimuth_millidegrees: this.#request.camera.azimuthMillidegrees,
        elevation_millidegrees: this.#request.camera.elevationMillidegrees,
        target_altitude_mm: this.#request.camera.targetAltitudeMm,
        near_mm: this.#request.camera.nearMm,
        far_mm: this.#request.camera.farMm,
        orthographic_width_mm: this.#request.camera.orthographicWidthMm,
        orthographic_height_mm: this.#request.camera.orthographicHeightMm,
        camera_distance_mm: this.#request.camera.cameraDistanceMm,
      },
      lighting: {
        sun_azimuth_millidegrees: this.#request.lighting.sunAzimuthMillidegrees,
        sun_elevation_millidegrees: this.#request.lighting.sunElevationMillidegrees,
      },
      capture: {
        renderer:
          this.#request.provider === "synthetic"
            ? "threejs-synthetic-fixture"
            : "threejs-google-3d-tiles",
        renderer_version: "capture-v1+three-0.185.1+3d-tiles-renderer-0.5.0",
        provider: this.#request.provider,
        source_epoch: this.#request.sourceEpoch,
        complete: true,
        attributions: evidence.attributions,
      },
      core_coverage_basis_points: evidence.coreCoverageBasisPoints,
      layers: REQUIRED_LAYER_NAMES.map((name) => {
        const layer = this.#accepted.get(name);
        if (layer === undefined) {
          throw new Error(`capture layer ${name} disappeared before finalization`);
        }
        return {
          kind: layer.kind,
          path: layer.filename,
          encoding: layer.encoding,
          width_px: width,
          height_px: height,
          byte_length: layer.byteLength,
          sha256: layer.sha256,
        };
      }),
    };
    const manifestPath = resolve(this.#stagingDirectory, "reference.manifest.json");
    const handle = await open(manifestPath, "wx", 0o600);
    try {
      await handle.writeFile(`${JSON.stringify(manifest, null, 2)}\n`);
      await handle.sync();
    } finally {
      await handle.close();
    }
    await validate(this.#stagingDirectory);
    await rename(this.#stagingDirectory, this.#outputDirectory);
    return this.#outputDirectory;
  }

  public async abort(): Promise<void> {
    await rm(this.#stagingDirectory, { force: true, recursive: true });
  }
}

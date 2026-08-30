import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import type { CaptureRequest, LayerName } from "../contracts.js";
import type { PixelFormat } from "./bundle-writer.js";
import { encodePng } from "./png.js";

export interface ProbeJoinEvidence {
  assembledRawSha256: string;
  cellFiles: [string, string];
  coreFile: string;
  mismatchPixels: number;
  sourceRawSha256: string;
}

function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function cropRgba(
  pixels: Uint8Array,
  sourceWidth: number,
  x: number,
  y: number,
  width: number,
  height: number,
): Uint8Array {
  const output = new Uint8Array(width * height * 4);
  for (let row = 0; row < height; row += 1) {
    const sourceStart = ((y + row) * sourceWidth + x) * 4;
    output.set(pixels.subarray(sourceStart, sourceStart + width * 4), row * width * 4);
  }
  return output;
}

function joinHorizontal(left: Uint8Array, right: Uint8Array, size: number): Uint8Array {
  const width = size * 2;
  const output = new Uint8Array(width * size * 4);
  for (let row = 0; row < size; row += 1) {
    const target = row * width * 4;
    output.set(left.subarray(row * size * 4, (row + 1) * size * 4), target);
    output.set(right.subarray(row * size * 4, (row + 1) * size * 4), target + size * 4);
  }
  return output;
}

export class ProbeArtifactWriter {
  readonly #directory: string;
  readonly #request: CaptureRequest;
  #evidence: ProbeJoinEvidence | undefined;

  public constructor(directory: string, request: CaptureRequest) {
    this.#directory = resolve(directory);
    this.#request = request;
  }

  public async accept(
    name: LayerName,
    pixels: Uint8Array,
    width: number,
    height: number,
    pixelFormat: PixelFormat,
  ): Promise<void> {
    if (name !== "color") {
      return;
    }
    const { coreHeightPx, coreWidthPx, guardPx } = this.#request.tile;
    if (
      this.#evidence !== undefined ||
      pixelFormat !== "rgba8" ||
      coreWidthPx !== 1_024 ||
      coreHeightPx !== 1_024 ||
      width !== coreWidthPx + guardPx * 2 ||
      height !== coreHeightPx + guardPx * 2 ||
      pixels.length !== width * height * 4
    ) {
      throw new Error("probe color layer does not match the 2x2 registered core");
    }
    const core = cropRgba(pixels, width, guardPx, guardPx, coreWidthPx, coreHeightPx);
    const left = cropRgba(core, coreWidthPx, 0, 0, 512, 512);
    const right = cropRgba(core, coreWidthPx, 512, 0, 512, 512);
    const sourceTop = cropRgba(core, coreWidthPx, 0, 0, 1_024, 512);
    const assembled = joinHorizontal(left, right, 512);
    let mismatchPixels = 0;
    for (let offset = 0; offset < assembled.length; offset += 4) {
      if (
        assembled[offset] !== sourceTop[offset] ||
        assembled[offset + 1] !== sourceTop[offset + 1] ||
        assembled[offset + 2] !== sourceTop[offset + 2] ||
        assembled[offset + 3] !== sourceTop[offset + 3]
      ) {
        mismatchPixels += 1;
      }
    }
    if (mismatchPixels !== 0) {
      throw new Error("probe cell assembly differs from its monolithic source crop");
    }
    await mkdir(this.#directory, { mode: 0o700, recursive: true });
    const coreFile = "core.png";
    const leftFile = "cell-0-0.png";
    const rightFile = "cell-1-0.png";
    await Promise.all([
      writeFile(resolve(this.#directory, coreFile), encodePng(core, 1_024, 1_024, 6), {
        flag: "wx",
        mode: 0o600,
      }),
      writeFile(resolve(this.#directory, leftFile), encodePng(left, 512, 512, 6), {
        flag: "wx",
        mode: 0o600,
      }),
      writeFile(resolve(this.#directory, rightFile), encodePng(right, 512, 512, 6), {
        flag: "wx",
        mode: 0o600,
      }),
      writeFile(
        resolve(this.#directory, "joined-top.png"),
        encodePng(assembled, 1_024, 512, 6),
        { flag: "wx", mode: 0o600 },
      ),
    ]);
    this.#evidence = {
      assembledRawSha256: sha256(assembled),
      cellFiles: [leftFile, rightFile],
      coreFile,
      mismatchPixels,
      sourceRawSha256: sha256(sourceTop),
    };
  }

  public finalize(): ProbeJoinEvidence {
    if (this.#evidence === undefined) {
      throw new Error("probe produced no color join evidence");
    }
    return this.#evidence;
  }
}

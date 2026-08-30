import { createHash } from "node:crypto";
import { chmod, mkdir, open } from "node:fs/promises";
import { resolve } from "node:path";
import type { CaptureRequest, LayerName } from "../contracts.js";
import type { PixelFormat } from "./bundle-writer.js";
import { writePngFile } from "./png.js";
import { cropRustPng } from "./rust-reference.js";

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

async function hashRawCrop(
  path: string,
  sourceWidth: number,
  x: number,
  y: number,
  width: number,
  height: number,
): Promise<string> {
  const digest = createHash("sha256");
  const handle = await open(path, "r");
  const row = Buffer.allocUnsafe(width * 4);
  try {
    for (let offset = 0; offset < height; offset += 1) {
      const position = ((y + offset) * sourceWidth + x) * 4;
      const { bytesRead } = await handle.read(row, 0, row.length, position);
      if (bytesRead !== row.length) {
        throw new Error("probe raw crop ended before its declared bounds");
      }
      digest.update(row);
    }
  } finally {
    await handle.close();
  }
  return digest.digest("hex");
}

async function hashJoinedCells(
  path: string,
  sourceWidth: number,
  x: number,
  y: number,
  size: number,
): Promise<string> {
  const digest = createHash("sha256");
  const handle = await open(path, "r");
  const cellRow = Buffer.allocUnsafe(size * 4);
  try {
    for (let row = 0; row < size; row += 1) {
      for (let column = 0; column < 2; column += 1) {
        const position = ((y + row) * sourceWidth + x + column * size) * 4;
        const { bytesRead } = await handle.read(cellRow, 0, cellRow.length, position);
        if (bytesRead !== cellRow.length) {
          throw new Error("probe cell row ended before its declared bounds");
        }
        digest.update(cellRow);
      }
    }
  } finally {
    await handle.close();
  }
  return digest.digest("hex");
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
      coreWidthPx < 1_024 ||
      coreHeightPx < 512 ||
      width !== coreWidthPx + guardPx * 2 ||
      height !== coreHeightPx + guardPx * 2 ||
      pixels.length !== width * height * 4
    ) {
      throw new Error("probe color layer does not contain the registered join fixture");
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
    await writePngFile(
      resolve(this.#directory, coreFile),
      core,
      coreWidthPx,
      coreHeightPx,
      6,
    );
    await writePngFile(resolve(this.#directory, leftFile), left, 512, 512, 6);
    await writePngFile(resolve(this.#directory, rightFile), right, 512, 512, 6);
    await writePngFile(resolve(this.#directory, "joined-top.png"), assembled, 1_024, 512, 6);
    this.#evidence = {
      assembledRawSha256: sha256(assembled),
      cellFiles: [leftFile, rightFile],
      coreFile,
      mismatchPixels,
      sourceRawSha256: sha256(sourceTop),
    };
  }

  public async acceptFile(
    name: LayerName,
    path: string,
    byteLength: number,
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
      coreWidthPx < 1_024 ||
      coreHeightPx < 512 ||
      width !== coreWidthPx + guardPx * 2 ||
      height !== coreHeightPx + guardPx * 2 ||
      byteLength !== width * height * 4
    ) {
      throw new Error("probe raw color layer does not contain the registered join fixture");
    }
    await mkdir(this.#directory, { mode: 0o700, recursive: true });
    const coreFile = "core.png";
    const leftFile = "cell-0-0.png";
    const rightFile = "cell-1-0.png";
    const outputs = [
      [coreFile, guardPx, guardPx, coreWidthPx, coreHeightPx],
      [leftFile, guardPx, guardPx, 512, 512],
      [rightFile, guardPx + 512, guardPx, 512, 512],
      ["joined-top.png", guardPx, guardPx, 1_024, 512],
    ] as const;
    for (const [filename, x, y, cropWidth, cropHeight] of outputs) {
      const output = resolve(this.#directory, filename);
      cropRustPng(path, output, width, height, x, y, cropWidth, cropHeight);
      await chmod(output, 0o600);
    }
    const sourceRawSha256 = await hashRawCrop(path, width, guardPx, guardPx, 1_024, 512);
    const assembledRawSha256 = await hashJoinedCells(path, width, guardPx, guardPx, 512);
    if (sourceRawSha256 !== assembledRawSha256) {
      throw new Error("probe cell assembly differs from its monolithic source crop");
    }
    this.#evidence = {
      assembledRawSha256,
      cellFiles: [leftFile, rightFile],
      coreFile,
      mismatchPixels: 0,
      sourceRawSha256,
    };
  }

  public finalize(): ProbeJoinEvidence {
    if (this.#evidence === undefined) {
      throw new Error("probe produced no color join evidence");
    }
    return this.#evidence;
  }
}

import { constants } from "node:fs";
import { chmod, copyFile, mkdir, stat } from "node:fs/promises";
import { resolve } from "node:path";
import type { CaptureRequest, LayerName } from "../contracts.js";
import type { PixelFormat } from "./bundle-writer.js";

const RAW_LAYER_FILES: Record<LayerName, string> = {
  color: "color.rgba8",
  coverage: "coverage.gray8",
  "fixed-shadow": "fixed-shadow.gray8",
  "linear-depth": "depth.u32le",
  "view-normal": "normal.rgba8",
  whitebox: "whitebox.rgba8",
};

export class RawLayerArchive {
  readonly #directory: string;
  readonly #request: CaptureRequest;

  public constructor(directory: string, request: CaptureRequest) {
    this.#directory = resolve(directory);
    this.#request = request;
  }

  public async acceptFile(
    name: LayerName,
    path: string,
    byteLength: number,
    width: number,
    height: number,
    pixelFormat: PixelFormat,
  ): Promise<void> {
    const expectedWidth =
      this.#request.tile.coreWidthPx + this.#request.tile.guardPx * 2;
    const expectedHeight =
      this.#request.tile.coreHeightPx + this.#request.tile.guardPx * 2;
    const expectedLength =
      pixelFormat === "gray8"
        ? width * height
        : pixelFormat === "rgba8"
          ? width * height * 4
          : 16 + width * height * 4;
    if (
      width !== expectedWidth ||
      height !== expectedHeight ||
      byteLength !== expectedLength
    ) {
      throw new Error("raw overlap layer contradicts its registered grid");
    }
    await mkdir(this.#directory, { mode: 0o700, recursive: true });
    const output = resolve(this.#directory, RAW_LAYER_FILES[name]);
    await copyFile(path, output, constants.COPYFILE_EXCL);
    await chmod(output, 0o600);
    if ((await stat(output)).size !== byteLength) {
      throw new Error("raw overlap layer archive changed byte length");
    }
  }
}

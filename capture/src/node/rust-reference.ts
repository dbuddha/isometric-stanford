import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const REPOSITORY_ROOT = existsSync(resolve(process.cwd(), "Cargo.toml"))
  ? resolve(process.cwd())
  : resolve(process.cwd(), "..");
function runReferenceCommand(arguments_: string[]): string {
  const environment = { ...process.env };
  delete environment.GOOGLE_MAP_TILES_API_KEY;
  const result = spawnSync(
    "cargo",
    ["run", "--quiet", "--locked", "--", "reference", ...arguments_],
    {
    cwd: REPOSITORY_ROOT,
    encoding: "utf8",
    env: environment,
    maxBuffer: 1_048_576,
    },
  );
  if (result.status !== 0) {
    throw new Error(`Rust reference image command failed: ${result.stderr.trim()}`);
  }
  return result.stdout.trim();
}

export function validateRustBundle(stagingDirectory: string): void {
  runReferenceCommand(["inspect", stagingDirectory]);
}

export function encodeRustPng(
  rawPath: string,
  outputPath: string,
  width: number,
  height: number,
  pixelFormat: "gray8" | "rgba8",
): void {
  runReferenceCommand([
    "encode-png",
    rawPath,
    outputPath,
    String(width),
    String(height),
    pixelFormat,
  ]);
}

export function cropRustPng(
  rawPath: string,
  outputPath: string,
  sourceWidth: number,
  sourceHeight: number,
  x: number,
  y: number,
  width: number,
  height: number,
): void {
  runReferenceCommand([
    "crop-png",
    rawPath,
    outputPath,
    String(sourceWidth),
    String(sourceHeight),
    String(x),
    String(y),
    String(width),
    String(height),
    "rgba8",
  ]);
}

export function compareRustOverlap(requestPath: string): void {
  runReferenceCommand(["compare-overlap", requestPath]);
}

export function compileRustAtlas(requestPath: string, outputDirectory: string): string {
  return runReferenceCommand(["atlas-compile", requestPath, outputDirectory]);
}

export function inspectRustAtlas(outputDirectory: string): string {
  return runReferenceCommand(["atlas-inspect", outputDirectory]);
}

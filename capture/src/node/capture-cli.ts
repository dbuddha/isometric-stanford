import { resolve } from "node:path";
import { captureBundle, readCaptureRequest } from "./capture-runner.js";

function argument(name: string): string {
  const index = process.argv.indexOf(name);
  const value = index >= 0 ? process.argv[index + 1] : undefined;
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`required argument ${name} is missing`);
  }
  return value;
}

async function main(): Promise<void> {
  const specPath = resolve(argument("--spec"));
  const outputDirectory = resolve(argument("--output"));
  const apiKey = process.env.GOOGLE_MAP_TILES_API_KEY ?? "";
  if (apiKey.length < 6) {
    throw new Error("GOOGLE_MAP_TILES_API_KEY is required for live reference capture");
  }
  const request = await readCaptureRequest(specPath);
  const output = await captureBundle(request, outputDirectory, apiKey);
  process.stdout.write(`captured registered reference bundle at ${output}\n`);
}

main().catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`capture failed: ${message}\n`);
  process.exitCode = 1;
});

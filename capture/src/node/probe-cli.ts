import { resolve } from "node:path";
import { readProbeSpec, runProbe } from "./probe-runner.js";

function argument(name: string): string {
  const index = process.argv.indexOf(name);
  const value = index >= 0 ? process.argv[index + 1] : undefined;
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`required argument ${name} is missing`);
  }
  return value;
}

async function main(): Promise<void> {
  const spec = await readProbeSpec(resolve(argument("--spec")));
  const outputDirectory = resolve(argument("--output"));
  const apiKey = process.env.GOOGLE_MAP_TILES_API_KEY ?? "";
  if (apiKey.length < 6) {
    throw new Error("GOOGLE_MAP_TILES_API_KEY is required for the live capture probe");
  }
  const output = await runProbe(spec, outputDirectory, apiKey);
  process.stdout.write(`captured bounded Hoover camera probe at ${output}\n`);
}

main().catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`capture probe failed: ${message}\n`);
  process.exitCode = 1;
});

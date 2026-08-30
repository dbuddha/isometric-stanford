import { resolve } from "node:path";
import { readOverlapSpec, runOverlapExperiment } from "./overlap-runner.js";

function argument(name: string): string {
  const index = process.argv.indexOf(name);
  const value = index >= 0 ? process.argv[index + 1] : undefined;
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`required argument ${name} is missing`);
  }
  return value;
}

async function main(): Promise<void> {
  const spec = await readOverlapSpec(resolve(argument("--spec")));
  const outputDirectory = resolve(argument("--output"));
  const apiKey = process.env.GOOGLE_MAP_TILES_API_KEY ?? "";
  if (apiKey.length < 6) {
    throw new Error("GOOGLE_MAP_TILES_API_KEY is required for the live overlap experiment");
  }
  const output = await runOverlapExperiment(spec, outputDirectory, apiKey);
  process.stdout.write(`captured registered Hoover overlap evidence at ${output}\n`);
}

main().catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`registered overlap experiment failed: ${message}\n`);
  process.exitCode = 1;
});

import { randomBytes } from "node:crypto";
import { mkdir, readFile, rename, rm, stat } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { CaptureRequest, ProbeCandidate } from "../contracts.js";
import { redactSecrets, validateCaptureRequest } from "../contracts.js";
import { runDirectChromiumProbe } from "./headless-probe.js";
import { startProbeCoordinator } from "./probe-coordinator.js";
import type { ProbeCoordinator } from "./probe-coordinator.js";
import { startProbeIngest } from "./probe-ingest-client.js";
import type { ProbeIngestClient } from "./probe-ingest-client.js";
import { validateRustBundle as validateRustBundleCommand } from "./rust-reference.js";
import { startStaticRendererServer } from "./static-renderer-server.js";
import type { StaticRendererServer } from "./static-renderer-server.js";

const CAPTURE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const LIVE_CAPTURE_REQUEST_LIMIT = 1_000;
const SINGLE_CAPTURE_ID = "capture";

export async function readCaptureRequest(path: string): Promise<CaptureRequest> {
  const parsed: unknown = JSON.parse(await readFile(path, "utf8"));
  validateCaptureRequest(parsed);
  return parsed;
}

export async function validateRustBundle(stagingDirectory: string): Promise<void> {
  validateRustBundleCommand(stagingDirectory);
}

async function assertAbsent(path: string): Promise<void> {
  try {
    await stat(path);
    throw new Error("capture output already exists; registered bundles are immutable");
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
      throw error;
    }
  }
}

export async function captureBundle(
  request: CaptureRequest,
  outputDirectory: string,
  apiKey: string,
): Promise<string> {
  if (request.provider !== "google-photorealistic-3d-tiles") {
    throw new Error("only Google reference requests may be promoted as durable bundles");
  }
  if (apiKey.length < 6) {
    throw new Error("Google tile credential is missing");
  }
  const output = resolve(outputDirectory);
  await mkdir(dirname(output), { mode: 0o700, recursive: true });
  await assertAbsent(output);
  const staging = resolve(dirname(output), `.capture-${randomBytes(8).toString("hex")}`);
  await mkdir(staging, { mode: 0o700, recursive: false });
  const secrets: string[] = [];
  let coordinator: ProbeCoordinator | undefined;
  let ingest: ProbeIngestClient | undefined;
  let rendererServer: StaticRendererServer | undefined;
  try {
    ingest = await startProbeIngest(staging, [{ candidateId: SINGLE_CAPTURE_ID, request }]);
    const target = ingest.targets[0];
    if (target === undefined || target.candidateId !== SINGLE_CAPTURE_ID) {
      throw new Error("capture ingest worker returned no upload target");
    }
    secrets.push(target.upload.token);
    const candidate: ProbeCandidate = {
      candidateId: SINGLE_CAPTURE_ID,
      request,
      upload: target.upload,
    };
    rendererServer = await startStaticRendererServer(resolve(CAPTURE_ROOT, "dist"));
    coordinator = await startProbeCoordinator({
      apiKey,
      candidates: [candidate],
      requestLimit: LIVE_CAPTURE_REQUEST_LIMIT,
    });
    secrets.push(coordinator.token);
    const execution = await runDirectChromiumProbe(
      rendererServer.url,
      coordinator,
      request.readiness.timeoutMs + 60_000,
    );
    if (execution.probe.network.blocked !== 0) {
      throw new Error("capture exhausted its Google request budget");
    }
    const evidence = execution.probe.candidates[0];
    if (evidence === undefined || evidence.candidateId !== SINGLE_CAPTURE_ID) {
      throw new Error("capture browser returned incomplete registered evidence");
    }
    await ingest.finalize([evidence]);
    const bundle = resolve(staging, "bundles", SINGLE_CAPTURE_ID);
    await rename(bundle, output);
    return output;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(redactSecrets(message, [apiKey, ...secrets]));
  } finally {
    await ingest?.abort().catch(() => undefined);
    await coordinator?.close().catch(() => undefined);
    await rendererServer?.close().catch(() => undefined);
    await rm(staging, { force: true, recursive: true });
  }
}

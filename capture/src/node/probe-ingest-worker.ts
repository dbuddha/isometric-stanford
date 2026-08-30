import { resolve } from "node:path";
import type { CaptureRequest, ProbeCandidateEvidence } from "../contracts.js";
import { validateCaptureRequest } from "../contracts.js";
import { BundleWriter } from "./bundle-writer.js";
import type {
  ProbeIngestCandidate,
  ProbeIngestParentMessage,
  ProbeIngestWorkerMessage,
} from "./probe-ingest-contracts.js";
import { ProbeArtifactWriter } from "./probe-artifacts.js";
import { RawLayerArchive } from "./raw-layer-archive.js";
import { validateRustBundle } from "./rust-reference.js";
import { startUploadServer } from "./upload-server.js";
import type { UploadServer } from "./upload-server.js";

interface CandidateState {
  artifacts: ProbeArtifactWriter;
  candidateId: string;
  rawArchive: RawLayerArchive | undefined;
  request: CaptureRequest;
  upload: UploadServer;
  writer: BundleWriter;
}

const states: CandidateState[] = [];
let busy = false;
let finalized = false;

function send(message: ProbeIngestWorkerMessage): void {
  if (process.send === undefined) {
    throw new Error("probe ingest worker has no private IPC channel");
  }
  process.send(message);
}

function validCandidate(candidate: ProbeIngestCandidate): boolean {
  try {
    validateCaptureRequest(candidate.request);
  } catch {
    return false;
  }
  return (
    /^[a-z0-9-]{1,64}$/.test(candidate.candidateId) &&
    candidate.request.provider === "google-photorealistic-3d-tiles"
  );
}

async function closeUploads(): Promise<void> {
  await Promise.all(states.map(async (state) => state.upload.close().catch(() => undefined)));
}

async function abort(): Promise<void> {
  await closeUploads();
  await Promise.all(states.map(async (state) => state.writer.abort().catch(() => undefined)));
}

async function initialize(
  stagingDirectory: string,
  candidates: ProbeIngestCandidate[],
  archiveRawLayers: boolean,
): Promise<void> {
  if (states.length !== 0 || candidates.length < 1 || candidates.length > 8) {
    throw new Error("probe ingest worker initialization is invalid");
  }
  const ids = new Set<string>();
  for (const candidate of candidates) {
    if (!validCandidate(candidate) || ids.has(candidate.candidateId)) {
      throw new Error("probe ingest worker candidate contract is invalid");
    }
    ids.add(candidate.candidateId);
    const writer = new BundleWriter(
      resolve(stagingDirectory, "bundles", candidate.candidateId),
      candidate.request,
    );
    const artifacts = new ProbeArtifactWriter(
      resolve(stagingDirectory, "candidates", candidate.candidateId),
      candidate.request,
    );
    const rawArchive = archiveRawLayers
      ? new RawLayerArchive(
          resolve(stagingDirectory, "raw", candidate.candidateId),
          candidate.request,
        )
      : undefined;
    await writer.start();
    const upload = await startUploadServer({
      async acceptFile(name, path, byteLength, width, height, pixelFormat): Promise<void> {
        await artifacts.acceptFile(name, path, byteLength, width, height, pixelFormat);
        await rawArchive?.acceptFile(name, path, byteLength, width, height, pixelFormat);
        await writer.acceptFile(name, path, byteLength, width, height, pixelFormat);
      },
    });
    states.push({
      artifacts,
      candidateId: candidate.candidateId,
      rawArchive,
      request: candidate.request,
      upload,
      writer,
    });
  }
  send({
    targets: states.map((state) => ({
      candidateId: state.candidateId,
      upload: { token: state.upload.token, url: state.upload.url },
    })),
    type: "ready",
  });
}

async function finalize(evidence: ProbeCandidateEvidence[]): Promise<void> {
  if (finalized || evidence.length !== states.length) {
    throw new Error("probe ingest evidence count is invalid");
  }
  await closeUploads();
  const results = [];
  for (let index = 0; index < states.length; index += 1) {
    const state = states[index];
    const candidateEvidence = evidence[index];
    if (
      state === undefined ||
      candidateEvidence === undefined ||
      candidateEvidence.candidateId !== state.candidateId
    ) {
      throw new Error("probe ingest evidence ordering is invalid");
    }
    await state.writer.finalize(candidateEvidence, async (path) => validateRustBundle(path));
    results.push({ artifacts: state.artifacts.finalize(), candidateId: state.candidateId });
  }
  finalized = true;
  send({
    results,
    type: "finalized",
    workerMaxRssBytes: process.resourceUsage().maxRSS * 1_024,
  });
}

process.on("message", (message: ProbeIngestParentMessage) => {
  if (busy) {
    send({ message: "probe ingest worker received concurrent commands", type: "error" });
    return;
  }
  busy = true;
  void (async () => {
    try {
      if (message.type === "initialize") {
        await initialize(
          message.stagingDirectory,
          message.candidates,
          message.archiveRawLayers,
        );
      } else if (message.type === "finalize") {
        await finalize(message.evidence);
      } else {
        await abort();
      }
    } catch (error) {
      await abort();
      send({
        message: error instanceof Error ? error.message : "probe ingest worker failed",
        type: "error",
      });
    } finally {
      busy = false;
    }
  })();
});

process.on("disconnect", () => {
  if (!finalized) {
    void abort().finally(() => process.exit(0));
  }
});

import { fork } from "node:child_process";
import type { ChildProcess } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { ProbeCandidateEvidence, UploadTarget } from "../contracts.js";
import type {
  ProbeIngestCandidate,
  ProbeIngestParentMessage,
  ProbeIngestWorkerMessage,
} from "./probe-ingest-contracts.js";
import type { ProbeJoinEvidence } from "./probe-artifacts.js";

interface FinalizedIngest {
  results: Array<{ artifacts: ProbeJoinEvidence; candidateId: string }>;
  workerMaxRssBytes: number;
}

export interface ProbeIngestClient {
  abort(): Promise<void>;
  finalize(evidence: ProbeCandidateEvidence[]): Promise<FinalizedIngest>;
  targets: Array<{ candidateId: string; upload: UploadTarget }>;
}

function send(child: ChildProcess, message: ProbeIngestParentMessage): void {
  if (!child.connected) {
    throw new Error("probe ingest worker IPC channel is closed");
  }
  child.send(message);
}

function waitFor(
  child: ChildProcess,
  expected: "ready" | "finalized",
): Promise<ProbeIngestWorkerMessage> {
  return new Promise((resolve, reject) => {
    const cleanup = (): void => {
      child.off("error", onError);
      child.off("exit", onExit);
      child.off("message", onMessage);
    };
    const onError = (error: Error): void => {
      cleanup();
      reject(error);
    };
    const onExit = (code: number | null): void => {
      cleanup();
      const diagnostics = child.stderr?.read()?.toString().trim() ?? "";
      reject(
        new Error(
          `probe ingest worker exited before ${expected}: ${code ?? "signal"}${diagnostics.length > 0 ? ` (${diagnostics})` : ""}`,
        ),
      );
    };
    const onMessage = (message: ProbeIngestWorkerMessage): void => {
      if (message.type === "error") {
        cleanup();
        reject(new Error(message.message));
      } else if (message.type === expected) {
        cleanup();
        resolve(message);
      }
    };
    child.on("error", onError);
    child.on("exit", onExit);
    child.on("message", onMessage);
  });
}

export async function startProbeIngest(
  stagingDirectory: string,
  candidates: ProbeIngestCandidate[],
): Promise<ProbeIngestClient> {
  const environment = { ...process.env };
  delete environment.GOOGLE_MAP_TILES_API_KEY;
  const adjacentWorker = fileURLToPath(new URL("./probe-ingest-worker.js", import.meta.url));
  const workerPath = existsSync(adjacentWorker)
    ? adjacentWorker
    : resolve(dirname(fileURLToPath(import.meta.url)), "../../dist-node/src/node/probe-ingest-worker.js");
  const child = fork(workerPath, [], {
    env: environment,
    execArgv: process.execArgv.filter((argument) => argument === "--expose-gc"),
    serialization: "advanced",
    stdio: ["ignore", "ignore", "pipe", "ipc"],
  });
  const readyPromise = waitFor(child, "ready");
  send(child, { candidates, stagingDirectory, type: "initialize" });
  const ready = await readyPromise;
  if (ready.type !== "ready") {
    throw new Error("probe ingest worker returned an invalid ready message");
  }
  return {
    async abort(): Promise<void> {
      if (child.connected) {
        send(child, { type: "abort" });
        child.disconnect();
      }
      if (child.exitCode === null) {
        child.kill();
      }
    },
    async finalize(evidence: ProbeCandidateEvidence[]): Promise<FinalizedIngest> {
      const finalizedPromise = waitFor(child, "finalized");
      send(child, { evidence, type: "finalize" });
      const result = await finalizedPromise;
      if (result.type !== "finalized") {
        throw new Error("probe ingest worker returned invalid final evidence");
      }
      child.disconnect();
      return { results: result.results, workerMaxRssBytes: result.workerMaxRssBytes };
    },
    targets: ready.targets,
  };
}

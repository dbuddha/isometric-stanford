import type { CaptureRequest, ProbeCandidateEvidence, UploadTarget } from "../contracts.js";
import type { ProbeJoinEvidence } from "./probe-artifacts.js";

export interface ProbeIngestCandidate {
  candidateId: string;
  request: CaptureRequest;
}

export type ProbeIngestParentMessage =
  | {
      archiveRawLayers: boolean;
      candidates: ProbeIngestCandidate[];
      stagingDirectory: string;
      type: "initialize";
    }
  | { evidence: ProbeCandidateEvidence[]; type: "finalize" }
  | { type: "abort" };

export type ProbeIngestWorkerMessage =
  | {
      targets: Array<{ candidateId: string; upload: UploadTarget }>;
      type: "ready";
    }
  | {
      results: Array<{ artifacts: ProbeJoinEvidence; candidateId: string }>;
      type: "finalized";
      workerMaxRssBytes: number;
    }
  | { message: string; type: "error" };

import {
  MAX_GOOGLE_REQUESTS_PER_CAPTURE,
  REQUIRED_LAYER_NAMES,
  cameraFingerprint,
  redactSecrets,
  validateCaptureRequest,
} from "../contracts.js";
import type { CaptureEvidence, CaptureRequest, UploadTarget } from "../contracts.js";
import type { ProbeBrowserResult, ProbeCandidate, ProbeCandidateEvidence } from "../contracts.js";
import { BrowserGoogleRequestBudget } from "./google-network-budget.js";
import { createGoogleScene } from "./google-scene.js";
import type { LayerUpload, RegisteredScene } from "./pass-renderer.js";
import { renderRegisteredLayers } from "./pass-renderer.js";
import { createSyntheticScene } from "./synthetic-scene.js";

export interface BrowserCaptureApi {
  capture(request: unknown, upload: UploadTarget): Promise<CaptureEvidence>;
  probe(candidates: unknown, requestLimit: unknown): Promise<ProbeBrowserResult>;
  ready: true;
}

async function uploadLayer(target: UploadTarget, layer: LayerUpload): Promise<void> {
  const body =
    layer.bytes.byteOffset === 0 && layer.bytes.byteLength === layer.bytes.buffer.byteLength
      ? (layer.bytes.buffer as ArrayBuffer)
      : (layer.bytes.slice().buffer as ArrayBuffer);
  const response = await fetch(`${target.url}/layer/${layer.name}`, {
    body,
    headers: {
      "content-type": "application/octet-stream",
      "x-capture-height": String(layer.height),
      "x-capture-pixel-format": layer.pixelFormat,
      "x-capture-token": target.token,
      "x-capture-width": String(layer.width),
    },
    method: "POST",
  });
  if (!response.ok) {
    throw new Error(`capture upload failed for ${layer.name}: HTTP ${response.status}`);
  }
}

export function installCaptureRuntime(canvas: HTMLCanvasElement): BrowserCaptureApi {
  return {
    async capture(value: unknown, upload: UploadTarget): Promise<CaptureEvidence> {
      validateCaptureRequest(value);
      const request: CaptureRequest = value;
      const apiKey = window.__CAPTURE_SECRETS__?.googleApiKey ?? "";
      let registered: RegisteredScene | undefined;
      try {
        registered =
          request.provider === "synthetic"
            ? createSyntheticScene(canvas, request)
            : createGoogleScene(canvas, request, apiKey);
        const ready = await registered.waitUntilReady();
        const attributions = registered.attributions();
        if (attributions.length === 0) {
          throw new Error("capture provider returned no attribution records");
        }
        const attributionElement = document.querySelector<HTMLElement>("#attribution");
        if (attributionElement !== null) {
          attributionElement.textContent = attributions.join(" | ");
        }
        const coreCoverageBasisPoints = await renderRegisteredLayers(
          registered,
          request,
          async (layer) => uploadLayer(upload, layer),
        );
        return {
          attributions,
          cameraFingerprint: cameraFingerprint(request),
          complete: true,
          coreCoverageBasisPoints,
          elapsedMs: ready.elapsedMs,
          layerOrder: [...REQUIRED_LAYER_NAMES],
          stableFrames: ready.stableFrames,
          visibleTiles: ready.visibleTiles,
        };
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        throw new Error(redactSecrets(message, [apiKey, upload.token]));
      } finally {
        registered?.dispose();
      }
    },
    async probe(value: unknown, requestLimit: unknown): Promise<ProbeBrowserResult> {
      if (!Array.isArray(value) || value.length < 1 || value.length > 8) {
        throw new Error("capture probe requires between one and eight camera candidates");
      }
      const candidates = value as ProbeCandidate[];
      for (const candidate of candidates) {
        validateCaptureRequest(candidate.request);
        if (
          candidate.request.provider !== "google-photorealistic-3d-tiles" ||
          typeof candidate.candidateId !== "string" ||
          !/^[a-z0-9-]{1,64}$/.test(candidate.candidateId) ||
          typeof candidate.upload?.token !== "string" ||
          typeof candidate.upload.url !== "string"
        ) {
          throw new Error("capture probe candidate is invalid");
        }
      }
      const apiKey = window.__CAPTURE_SECRETS__?.googleApiKey ?? "";
      if (
        !Number.isSafeInteger(requestLimit) ||
        Number(requestLimit) < 1 ||
        Number(requestLimit) > MAX_GOOGLE_REQUESTS_PER_CAPTURE
      ) {
        throw new Error("capture probe browser request limit is invalid");
      }
      const network = new BrowserGoogleRequestBudget(Number(requestLimit));
      const restoreFetch = network.install();
      let registered: RegisteredScene | undefined;
      try {
        const first = candidates[0];
        if (first === undefined) {
          throw new Error("capture probe has no first candidate");
        }
        registered = createGoogleScene(canvas, first.request, apiKey);
        const results: ProbeCandidateEvidence[] = [];
        for (let index = 0; index < candidates.length; index += 1) {
          const candidate = candidates[index];
          if (candidate === undefined) {
            throw new Error("capture probe candidate disappeared");
          }
          if (index > 0) {
            if (registered.reframe === undefined) {
              throw new Error("capture provider cannot reframe one tileset session");
            }
            registered.reframe(candidate.request);
          }
          const ready = await registered.waitUntilReady();
          const attributions = registered.attributions();
          if (!attributions.includes("Google Maps") || attributions.length < 2) {
            throw new Error("capture provider returned incomplete attribution records");
          }
          const attributionElement = document.querySelector<HTMLElement>("#attribution");
          if (attributionElement !== null) {
            attributionElement.textContent = attributions.join(" | ");
          }
          const coreCoverageBasisPoints = await renderRegisteredLayers(
            registered,
            candidate.request,
            async (layer) => uploadLayer(candidate.upload, layer),
          );
          const diagnostics = registered.diagnostics?.();
          if (diagnostics === undefined) {
            throw new Error("capture provider returned no scene diagnostics");
          }
          results.push({
            attributions,
            cameraFingerprint: cameraFingerprint(candidate.request),
            cameraWorldMatrix: [...registered.camera.matrixWorld.elements],
            candidateId: candidate.candidateId,
            complete: true,
            coreCoverageBasisPoints,
            diagnostics,
            elapsedMs: ready.elapsedMs,
            layerOrder: [...REQUIRED_LAYER_NAMES],
            networkAfterCandidate: network.snapshot(),
            projectionMatrix: [...registered.camera.projectionMatrix.elements],
            stableFrames: ready.stableFrames,
            visibleTiles: ready.visibleTiles,
          });
        }
        return { candidates: results, network: network.snapshot() };
      } catch (error) {
        const tokens = candidates.map((candidate) => candidate.upload.token);
        const message = error instanceof Error ? error.message : String(error);
        throw new Error(
          redactSecrets(
            `${message}; Google request telemetry: ${JSON.stringify(network.snapshot())}`,
            [apiKey, ...tokens],
          ),
        );
      } finally {
        registered?.dispose();
        restoreFetch();
      }
    },
    ready: true,
  };
}

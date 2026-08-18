import {
  REQUIRED_LAYER_NAMES,
  cameraFingerprint,
  redactSecrets,
  validateCaptureRequest,
} from "../contracts.js";
import type { CaptureEvidence, CaptureRequest, UploadTarget } from "../contracts.js";
import { createGoogleScene } from "./google-scene.js";
import type { LayerUpload, RegisteredScene } from "./pass-renderer.js";
import { renderRegisteredLayers } from "./pass-renderer.js";
import { createSyntheticScene } from "./synthetic-scene.js";

export interface BrowserCaptureApi {
  capture(request: unknown, upload: UploadTarget): Promise<CaptureEvidence>;
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
    ready: true,
  };
}

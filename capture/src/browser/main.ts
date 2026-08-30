import { installCaptureRuntime } from "./runtime.js";
import type { BrowserMemoryMetrics, ProbeCandidate, ProbeExecutionResult } from "../contracts.js";

interface ProbeBootstrap {
  apiKey: string;
  candidates: ProbeCandidate[];
  requestLimit: number;
}

const canvas = document.querySelector<HTMLCanvasElement>("#capture-canvas");
const status = document.querySelector<HTMLElement>("#status");
if (canvas === null || status === null) {
  throw new Error("capture page is missing its required elements");
}
const captureStatus = status;
window.ISOMETRIC_CAPTURE = installCaptureRuntime(canvas);
captureStatus.textContent = "Capture runtime ready";

function browserMemory(): BrowserMemoryMetrics {
  const memory = (
    performance as Performance & {
      memory?: { jsHeapSizeLimit: number; totalJSHeapSize: number; usedJSHeapSize: number };
    }
  ).memory;
  return {
    jsHeapSizeLimitBytes: memory?.jsHeapSizeLimit ?? null,
    jsHeapTotalBytes: memory?.totalJSHeapSize ?? null,
    jsHeapUsedBytes: memory?.usedJSHeapSize ?? null,
  };
}

async function postResult(
  coordinator: string,
  token: string,
  payload: ProbeExecutionResult | { error: string },
): Promise<void> {
  const response = await fetch(`${coordinator}/result`, {
    body: JSON.stringify(payload),
    headers: { "content-type": "application/json", "x-probe-token": token },
    method: "POST",
  });
  if (!response.ok) {
    throw new Error(`probe coordinator rejected browser result: HTTP ${response.status}`);
  }
}

async function runHeadlessProbe(): Promise<void> {
  const fragment = new URLSearchParams(window.location.hash.slice(1));
  const coordinator = fragment.get("probe");
  const token = fragment.get("token");
  if (coordinator === null || token === null) {
    return;
  }
  window.history.replaceState(null, "", window.location.pathname);
  try {
    const response = await fetch(`${coordinator}/bootstrap`, {
      headers: { "x-probe-token": token },
    });
    if (!response.ok) {
      throw new Error(`probe coordinator rejected bootstrap: HTTP ${response.status}`);
    }
    const bootstrap = (await response.json()) as ProbeBootstrap;
    if (
      typeof bootstrap.apiKey !== "string" ||
      bootstrap.apiKey.length < 6 ||
      !Array.isArray(bootstrap.candidates)
    ) {
      throw new Error("probe coordinator returned an invalid bootstrap contract");
    }
    window.__CAPTURE_SECRETS__ = { googleApiKey: bootstrap.apiKey };
    bootstrap.apiKey = "";
    if (window.ISOMETRIC_CAPTURE === undefined) {
      throw new Error("capture runtime missing during headless probe");
    }
    const probe = await window.ISOMETRIC_CAPTURE.probe(
      bootstrap.candidates,
      bootstrap.requestLimit,
    );
    window.__CAPTURE_SECRETS__ = undefined;
    captureStatus.textContent = "Capture probe complete";
    await postResult(coordinator, token, { browserMemory: browserMemory(), probe });
  } catch (error) {
    window.__CAPTURE_SECRETS__ = undefined;
    captureStatus.textContent = "Capture probe failed";
    await postResult(coordinator, token, {
      error: error instanceof Error ? error.message : "headless capture probe failed",
    });
  }
}

void runHeadlessProbe();

import { installCaptureRuntime } from "./runtime.js";

const canvas = document.querySelector<HTMLCanvasElement>("#capture-canvas");
const status = document.querySelector<HTMLElement>("#status");
if (canvas === null || status === null) {
  throw new Error("capture page is missing its required elements");
}
window.ISOMETRIC_CAPTURE = installCaptureRuntime(canvas);
status.textContent = "Capture runtime ready";

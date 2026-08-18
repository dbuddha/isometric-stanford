import type { BrowserCaptureApi } from "./runtime.js";

declare global {
  interface Window {
    ISOMETRIC_CAPTURE?: BrowserCaptureApi;
    __CAPTURE_SECRETS__?: {
      googleApiKey?: string;
    };
  }
}

export {};

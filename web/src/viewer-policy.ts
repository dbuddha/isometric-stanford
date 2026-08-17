const MOBILE_BREAKPOINT_PIXELS = 640;
const MOBILE_DECODED_BUDGET_BYTES = 96 * 1_024 * 1_024;
const DESKTOP_DECODED_BUDGET_BYTES = 256 * 1_024 * 1_024;
const DECODED_RGBA_TILE_BYTES = 512 * 512 * 4;
const CACHE_HEADROOM_DIVISOR = 2;

export interface ViewerPolicy {
  constrainedDevice: boolean;
  decodedBudgetBytes: number;
  maxImageCacheCount: number;
  initialZoomFactor: number;
}

export function viewerPolicy(
  viewportWidth: number,
  deviceMemory?: number,
  coarsePointer = false,
): ViewerPolicy {
  const constrainedDevice =
    viewportWidth <= MOBILE_BREAKPOINT_PIXELS || (deviceMemory ?? 8) <= 4 || coarsePointer;
  const decodedBudgetBytes = constrainedDevice
    ? MOBILE_DECODED_BUDGET_BYTES
    : DESKTOP_DECODED_BUDGET_BYTES;
  const maxImageCacheCount = Math.floor(
    decodedBudgetBytes / CACHE_HEADROOM_DIVISOR / DECODED_RGBA_TILE_BYTES,
  );

  return {
    constrainedDevice,
    decodedBudgetBytes,
    maxImageCacheCount,
    initialZoomFactor: viewportWidth <= MOBILE_BREAKPOINT_PIXELS ? 2.25 : 1,
  };
}

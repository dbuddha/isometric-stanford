const GIBIBYTE = 1_024 * 1_024 * 1_024;
const HOST_RESERVE_BYTES = 2 * GIBIBYTE;
const MAXIMUM_CAPTURE_WORKERS = 4;
const MEMORY_SHARE_NUMERATOR = 3;
const MEMORY_SHARE_DENOMINATOR = 4;

export function captureWorkerEnvelopeBytes(
  totalWidthPixels: number,
  measuredMinimumBytes = 0,
): number {
  if (!Number.isSafeInteger(measuredMinimumBytes) || measuredMinimumBytes < 0) {
    throw new Error("capture measured memory envelope must be a non-negative safe integer");
  }
  let dimensionEnvelope: number;
  if (totalWidthPixels <= 1_280) {
    dimensionEnvelope = GIBIBYTE;
  } else if (totalWidthPixels <= 2_560) {
    dimensionEnvelope = (5 * GIBIBYTE) / 4;
  } else {
    throw new Error("capture memory policy has no measured envelope for this grid");
  }
  return Math.max(dimensionEnvelope, measuredMinimumBytes);
}

export function deriveCaptureWorkerCount(
  hostTotalMemoryBytes: number,
  totalWidthPixels: number,
  measuredMinimumBytes = 0,
): number {
  if (!Number.isSafeInteger(hostTotalMemoryBytes) || hostTotalMemoryBytes < 1) {
    throw new Error("capture host memory must be a positive safe integer");
  }
  const envelope = captureWorkerEnvelopeBytes(totalWidthPixels, measuredMinimumBytes);
  const reservedAvailable = Math.max(0, hostTotalMemoryBytes - HOST_RESERVE_BYTES);
  const proportionalAvailable = Math.floor(
    (hostTotalMemoryBytes * MEMORY_SHARE_NUMERATOR) / MEMORY_SHARE_DENOMINATOR,
  );
  const available = Math.min(reservedAvailable, proportionalAvailable);
  return Math.min(MAXIMUM_CAPTURE_WORKERS, Math.floor(available / envelope));
}

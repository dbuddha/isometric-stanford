export interface OrthographicFrustum {
  bottom: number;
  left: number;
  right: number;
  top: number;
}

export function registeredOrthographicFrustum(
  widthMeters: number,
  heightMeters: number,
  centerX: number,
  centerY: number,
): OrthographicFrustum {
  if (
    !Number.isFinite(widthMeters) ||
    !Number.isFinite(heightMeters) ||
    !Number.isFinite(centerX) ||
    !Number.isFinite(centerY) ||
    widthMeters <= 0 ||
    heightMeters <= 0
  ) {
    throw new Error("registered orthographic frustum requires finite positive dimensions");
  }
  return {
    bottom: centerY - heightMeters / 2,
    left: centerX - widthMeters / 2,
    right: centerX + widthMeters / 2,
    top: centerY + heightMeters / 2,
  };
}

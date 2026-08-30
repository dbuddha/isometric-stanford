import {
  CAPTURE_SCHEMA,
  totalHeight,
  totalWidth,
  validateCaptureRequest,
  type CaptureRequest,
} from "../contracts.js";

const WGS84_SEMI_MAJOR_METERS = 6_378_137;
const WGS84_FLATTENING = 1 / 298.257_223_563;
const WGS84_ECCENTRICITY_SQUARED =
  WGS84_FLATTENING * (2 - WGS84_FLATTENING);
const DEGREES_TO_RADIANS = Math.PI / 180;
const RADIANS_TO_DEGREES = 180 / Math.PI;

interface Vector3 {
  x: number;
  y: number;
  z: number;
}

interface Geodetic {
  altitudeMeters: number;
  latitudeRadians: number;
  longitudeRadians: number;
}

interface CameraAxes {
  right: Vector3;
  up: Vector3;
}

export interface RegisteredGridCandidateReport {
  actualCenterOffsetPixels: { x: number; y: number };
  centerLatitudeE7: number;
  centerLongitudeE7: number;
  expectedCenterOffsetPixels: { x: number; y: number };
  maximumPixelCenterErrorPixels: number;
  role: "left" | "right";
}

export interface RegisteredGridReport {
  anchorLatitudeE7: number;
  anchorLongitudeE7: number;
  cameraScreenRightBearingMillidegrees: number;
  candidates: [RegisteredGridCandidateReport, RegisteredGridCandidateReport];
  checkedSavedPixelCenters: number;
  maximumPixelCenterErrorPixels: number;
  schema: "isometric-registered-grid-report/v1";
}

export interface RegisteredOverlapRequests {
  grid: RegisteredGridReport;
  left: CaptureRequest;
  monolithic: CaptureRequest;
  ordered: CaptureRequest[];
  right: CaptureRequest;
}

function add(left: Vector3, right: Vector3): Vector3 {
  return { x: left.x + right.x, y: left.y + right.y, z: left.z + right.z };
}

function subtract(left: Vector3, right: Vector3): Vector3 {
  return { x: left.x - right.x, y: left.y - right.y, z: left.z - right.z };
}

function scale(vector: Vector3, factor: number): Vector3 {
  return { x: vector.x * factor, y: vector.y * factor, z: vector.z * factor };
}

function dot(left: Vector3, right: Vector3): number {
  return left.x * right.x + left.y * right.y + left.z * right.z;
}

function geodeticToEcef(position: Geodetic): Vector3 {
  const sinLatitude = Math.sin(position.latitudeRadians);
  const cosLatitude = Math.cos(position.latitudeRadians);
  const sinLongitude = Math.sin(position.longitudeRadians);
  const cosLongitude = Math.cos(position.longitudeRadians);
  const primeVerticalRadius =
    WGS84_SEMI_MAJOR_METERS /
    Math.sqrt(1 - WGS84_ECCENTRICITY_SQUARED * sinLatitude * sinLatitude);
  return {
    x: (primeVerticalRadius + position.altitudeMeters) * cosLatitude * cosLongitude,
    y: (primeVerticalRadius + position.altitudeMeters) * cosLatitude * sinLongitude,
    z:
      (primeVerticalRadius * (1 - WGS84_ECCENTRICITY_SQUARED) +
        position.altitudeMeters) *
      sinLatitude,
  };
}

function ecefToGeodetic(position: Vector3): Geodetic {
  const longitudeRadians = Math.atan2(position.y, position.x);
  const horizontal = Math.hypot(position.x, position.y);
  let latitudeRadians = Math.atan2(
    position.z,
    horizontal * (1 - WGS84_ECCENTRICITY_SQUARED),
  );
  let altitudeMeters = 0;
  for (let iteration = 0; iteration < 12; iteration += 1) {
    const sinLatitude = Math.sin(latitudeRadians);
    const primeVerticalRadius =
      WGS84_SEMI_MAJOR_METERS /
      Math.sqrt(1 - WGS84_ECCENTRICITY_SQUARED * sinLatitude * sinLatitude);
    altitudeMeters = horizontal / Math.cos(latitudeRadians) - primeVerticalRadius;
    const denominator =
      horizontal *
      (1 -
        (WGS84_ECCENTRICITY_SQUARED * primeVerticalRadius) /
          (primeVerticalRadius + altitudeMeters));
    const next = Math.atan2(position.z, denominator);
    if (Math.abs(next - latitudeRadians) < 1e-15) {
      latitudeRadians = next;
      break;
    }
    latitudeRadians = next;
  }
  return { altitudeMeters, latitudeRadians, longitudeRadians };
}

function localBasis(position: Geodetic): { east: Vector3; north: Vector3; up: Vector3 } {
  const sinLatitude = Math.sin(position.latitudeRadians);
  const cosLatitude = Math.cos(position.latitudeRadians);
  const sinLongitude = Math.sin(position.longitudeRadians);
  const cosLongitude = Math.cos(position.longitudeRadians);
  return {
    east: { x: -sinLongitude, y: cosLongitude, z: 0 },
    north: {
      x: -sinLatitude * cosLongitude,
      y: -sinLatitude * sinLongitude,
      z: cosLatitude,
    },
    up: {
      x: cosLatitude * cosLongitude,
      y: cosLatitude * sinLongitude,
      z: sinLatitude,
    },
  };
}

function cameraAxes(position: Geodetic, request: CaptureRequest): CameraAxes {
  const basis = localBasis(position);
  const azimuth =
    request.camera.azimuthMillidegrees * DEGREES_TO_RADIANS / 1_000;
  const elevation =
    request.camera.elevationMillidegrees * DEGREES_TO_RADIANS / 1_000;
  const right = add(scale(basis.east, Math.cos(azimuth)), scale(basis.north, -Math.sin(azimuth)));
  const horizontalUp = add(
    scale(basis.east, Math.sin(azimuth) * Math.sin(elevation)),
    scale(basis.north, Math.cos(azimuth) * Math.sin(elevation)),
  );
  return {
    right,
    up: add(horizontalUp, scale(basis.up, Math.cos(elevation))),
  };
}

function geodeticFor(request: CaptureRequest): Geodetic {
  return {
    altitudeMeters: request.camera.targetAltitudeMm / 1_000,
    latitudeRadians:
      request.tile.centerLatitudeE7 * DEGREES_TO_RADIANS / 10_000_000,
    longitudeRadians:
      request.tile.centerLongitudeE7 * DEGREES_TO_RADIANS / 10_000_000,
  };
}

function cloneHalfRequest(
  base: CaptureRequest,
  role: "left" | "right",
  centerLongitudeE7: number,
  centerLatitudeE7: number,
): CaptureRequest {
  const request = structuredClone(base);
  const halfCoreWidth = base.tile.coreWidthPx / 2;
  request.bundleId = `${base.bundleId}-${role}`;
  request.tile.column = role === "left" ? base.tile.column * 2 : base.tile.column * 2 + 1;
  request.tile.coreWidthPx = halfCoreWidth;
  request.tile.centerLongitudeE7 = centerLongitudeE7;
  request.tile.centerLatitudeE7 = centerLatitudeE7;
  request.camera.orthographicWidthMm = totalWidth(request) * request.tile.millimetersPerPixel;
  request.camera.orthographicHeightMm = totalHeight(request) * request.tile.millimetersPerPixel;
  validateCaptureRequest(request);
  return request;
}

function roundedTarget(
  anchorEcef: Vector3,
  right: Vector3,
  offsetMeters: number,
): { latitudeE7: number; longitudeE7: number } {
  const shifted = ecefToGeodetic(add(anchorEcef, scale(right, offsetMeters)));
  return {
    latitudeE7: Math.round(shifted.latitudeRadians * RADIANS_TO_DEGREES * 10_000_000),
    longitudeE7: Math.round(shifted.longitudeRadians * RADIANS_TO_DEGREES * 10_000_000),
  };
}

function candidateGridReport(
  role: "left" | "right",
  candidate: CaptureRequest,
  anchorRequest: CaptureRequest,
  expectedCenterX: number,
): RegisteredGridCandidateReport {
  const anchor = geodeticFor(anchorRequest);
  const anchorEcef = geodeticToEcef(anchor);
  const anchorAxes = cameraAxes(anchor, anchorRequest);
  const candidatePosition = geodeticFor(candidate);
  const candidateEcef = geodeticToEcef(candidatePosition);
  const candidateAxes = cameraAxes(candidatePosition, candidate);
  const centerDelta = subtract(candidateEcef, anchorEcef);
  const millimetersPerPixel = candidate.tile.millimetersPerPixel;
  const actualCenterX = dot(centerDelta, anchorAxes.right) * 1_000 / millimetersPerPixel;
  const actualCenterY = dot(centerDelta, anchorAxes.up) * 1_000 / millimetersPerPixel;
  const halfWidth = candidate.tile.coreWidthPx / 2;
  const halfHeight = candidate.tile.coreHeightPx / 2;
  let maximumPixelCenterErrorPixels = 0;
  for (let row = 0; row < candidate.tile.coreHeightPx; row += 1) {
    const localY = (row + 0.5 - halfHeight) * millimetersPerPixel / 1_000;
    for (let column = 0; column < candidate.tile.coreWidthPx; column += 1) {
      const localX = (column + 0.5 - halfWidth) * millimetersPerPixel / 1_000;
      const point = add(
        add(candidateEcef, scale(candidateAxes.right, localX)),
        scale(candidateAxes.up, localY),
      );
      const delta = subtract(point, anchorEcef);
      const actualX = dot(delta, anchorAxes.right) * 1_000 / millimetersPerPixel;
      const actualY = dot(delta, anchorAxes.up) * 1_000 / millimetersPerPixel;
      const expectedX = expectedCenterX + column + 0.5 - halfWidth;
      const expectedY = row + 0.5 - halfHeight;
      maximumPixelCenterErrorPixels = Math.max(
        maximumPixelCenterErrorPixels,
        Math.hypot(actualX - expectedX, actualY - expectedY),
      );
    }
  }
  return {
    actualCenterOffsetPixels: { x: actualCenterX, y: actualCenterY },
    centerLatitudeE7: candidate.tile.centerLatitudeE7,
    centerLongitudeE7: candidate.tile.centerLongitudeE7,
    expectedCenterOffsetPixels: { x: expectedCenterX, y: 0 },
    maximumPixelCenterErrorPixels,
    role,
  };
}

export function deriveRegisteredOverlapRequests(baseValue: unknown): RegisteredOverlapRequests {
  validateCaptureRequest(baseValue);
  const base = structuredClone(baseValue);
  const validPilot =
    base.schema === CAPTURE_SCHEMA &&
    base.provider === "google-photorealistic-3d-tiles" &&
    base.tile.coreWidthPx === 2_048 &&
    base.tile.coreHeightPx === 1_024 &&
    base.tile.guardPx === 128 &&
    base.tile.millimetersPerPixel === 250 &&
    base.camera.azimuthMillidegrees === 330_000 &&
    base.camera.elevationMillidegrees === 42_000;
  if (!validPilot) {
    throw new Error("registered overlap pilot requires the approved 2048 by 1024 Hoover grid");
  }
  base.bundleId = `${base.bundleId}-monolithic`;
  validateCaptureRequest(base);
  const anchor = geodeticFor(base);
  const anchorEcef = geodeticToEcef(anchor);
  const axes = cameraAxes(anchor, base);
  const halfCoreWidth = base.tile.coreWidthPx / 2;
  const halfCandidateWidth = halfCoreWidth / 2;
  const centerOffsetMeters =
    halfCandidateWidth * base.tile.millimetersPerPixel / 1_000;
  const leftTarget = roundedTarget(anchorEcef, axes.right, -centerOffsetMeters);
  const rightTarget = roundedTarget(anchorEcef, axes.right, centerOffsetMeters);
  const left = cloneHalfRequest(baseValue, "left", leftTarget.longitudeE7, leftTarget.latitudeE7);
  const right = cloneHalfRequest(baseValue, "right", rightTarget.longitudeE7, rightTarget.latitudeE7);
  const leftReport = candidateGridReport("left", left, base, -halfCandidateWidth);
  const rightReport = candidateGridReport("right", right, base, halfCandidateWidth);
  const maximumPixelCenterErrorPixels = Math.max(
    leftReport.maximumPixelCenterErrorPixels,
    rightReport.maximumPixelCenterErrorPixels,
  );
  if (maximumPixelCenterErrorPixels > 0.5) {
    throw new Error(
      `registered overlap grid exceeds 0.5 source pixel: ${maximumPixelCenterErrorPixels}`,
    );
  }
  const bearing = (base.camera.azimuthMillidegrees + 90_000) % 360_000;
  return {
    grid: {
      anchorLatitudeE7: base.tile.centerLatitudeE7,
      anchorLongitudeE7: base.tile.centerLongitudeE7,
      cameraScreenRightBearingMillidegrees: bearing,
      candidates: [leftReport, rightReport],
      checkedSavedPixelCenters:
        left.tile.coreWidthPx * left.tile.coreHeightPx +
        right.tile.coreWidthPx * right.tile.coreHeightPx,
      maximumPixelCenterErrorPixels,
      schema: "isometric-registered-grid-report/v1",
    },
    left,
    monolithic: base,
    ordered: [base, left, right],
    right,
  };
}

import { GoogleCloudAuthPlugin } from "3d-tiles-renderer/core/plugins";
import { TilesRenderer, WGS84_ELLIPSOID } from "3d-tiles-renderer/three";
import { CAMERA_FRAME } from "3d-tiles-renderer/src/three/renderer/math/Ellipsoid.js";
import { GLTFExtensionsPlugin, TileCompressionPlugin } from "3d-tiles-renderer/three/plugins";
import {
  Color,
  MathUtils,
  Matrix4,
  Mesh,
  OrthographicCamera,
  Scene,
  Vector3,
  WebGLRenderer,
} from "three";
import { DRACOLoader } from "three/addons/loaders/DRACOLoader.js";
import type { CaptureRequest, SceneDiagnostics } from "../contracts.js";
import { TileReadiness, waitForStableReadiness } from "../readiness.js";
import type { RegisteredScene } from "./pass-renderer.js";
import { registeredOrthographicFrustum } from "./registered-camera.js";

const DRACO_DECODER_PATH = "https://www.gstatic.com/draco/versioned/decoders/1.5.7/";
const MINIMUM_TILE_CACHE_BYTES = 128 * 1_024 * 1_024;
const MAXIMUM_TILE_CACHE_BYTES = 256 * 1_024 * 1_024;

function targetPosition(request: CaptureRequest, groupMatrix: Matrix4): Vector3 {
  const frame = new Matrix4();
  WGS84_ELLIPSOID.getObjectFrame(
    (request.tile.centerLatitudeE7 / 10_000_000) * MathUtils.DEG2RAD,
    (request.tile.centerLongitudeE7 / 10_000_000) * MathUtils.DEG2RAD,
    request.camera.targetAltitudeMm / 1_000,
    0,
    Math.PI / 2,
    0,
    frame,
    CAMERA_FRAME,
  );
  frame.premultiply(groupMatrix);
  return new Vector3().setFromMatrixPosition(frame);
}

function framePosition(
  request: CaptureRequest,
  azimuthMillidegrees: number,
  elevationMillidegrees: number,
  distanceMeters: number,
  groupMatrix: Matrix4,
): Vector3 {
  const frame = new Matrix4();
  WGS84_ELLIPSOID.getObjectFrame(
    (request.tile.centerLatitudeE7 / 10_000_000) * MathUtils.DEG2RAD,
    (request.tile.centerLongitudeE7 / 10_000_000) * MathUtils.DEG2RAD,
    request.camera.targetAltitudeMm / 1_000,
    (azimuthMillidegrees / 1_000) * MathUtils.DEG2RAD,
    -(elevationMillidegrees / 1_000) * MathUtils.DEG2RAD,
    0,
    frame,
    CAMERA_FRAME,
  );
  frame.multiply(new Matrix4().makeTranslation(0, 0, distanceMeters));
  frame.premultiply(groupMatrix);
  return new Vector3().setFromMatrixPosition(frame);
}

export function createGoogleScene(
  canvas: HTMLCanvasElement,
  request: CaptureRequest,
  apiKey: string,
): RegisteredScene {
  if (apiKey.length < 6) {
    throw new Error("Google tile credential is missing");
  }
  const initialWidth = request.tile.coreWidthPx + 2 * request.tile.guardPx;
  const initialHeight = request.tile.coreHeightPx + 2 * request.tile.guardPx;
  const renderer = new WebGLRenderer({ antialias: false, canvas, preserveDrawingBuffer: false });
  renderer.setPixelRatio(1);
  renderer.setSize(initialWidth, initialHeight, false);
  renderer.setClearColor(0x000000, 1);

  const scene = new Scene();
  scene.background = new Color(0x000000);
  const tiles = new TilesRenderer();
  tiles.lruCache.minBytesSize = MINIMUM_TILE_CACHE_BYTES;
  tiles.lruCache.maxBytesSize = MAXIMUM_TILE_CACHE_BYTES;
  tiles.lruCache.unloadPercent = 0.25;
  const draco = new DRACOLoader().setDecoderPath(DRACO_DECODER_PATH);
  tiles.registerPlugin(
    new GoogleCloudAuthPlugin({
      apiToken: apiKey,
      autoRefreshToken: false,
      useRecommendedSettings: true,
    }),
  );
  tiles.registerPlugin(new TileCompressionPlugin({ disableMipmaps: true, generateNormals: true }));
  tiles.registerPlugin(new GLTFExtensionsPlugin({ dracoLoader: draco }));
  tiles.group.rotation.x = -Math.PI / 2;
  tiles.group.updateMatrixWorld(true);
  scene.add(tiles.group);

  const horizontalMeters = request.camera.orthographicWidthMm / 1_000;
  const verticalMeters = request.camera.orthographicHeightMm / 1_000;
  const camera = new OrthographicCamera(
    -horizontalMeters / 2,
    horizontalMeters / 2,
    verticalMeters / 2,
    -verticalMeters / 2,
    request.camera.nearMm / 1_000,
    request.camera.farMm / 1_000,
  );
  WGS84_ELLIPSOID.getObjectFrame(
    (request.tile.centerLatitudeE7 / 10_000_000) * MathUtils.DEG2RAD,
    (request.tile.centerLongitudeE7 / 10_000_000) * MathUtils.DEG2RAD,
    request.camera.targetAltitudeMm / 1_000,
    (request.camera.azimuthMillidegrees / 1_000) * MathUtils.DEG2RAD,
    -(request.camera.elevationMillidegrees / 1_000) * MathUtils.DEG2RAD,
    0,
    camera.matrixWorld,
    CAMERA_FRAME,
  );
  camera.matrixWorld.multiply(
    new Matrix4().makeTranslation(0, 0, request.camera.cameraDistanceMm / 1_000),
  );
  camera.matrixWorld.premultiply(tiles.group.matrixWorld);
  camera.matrixWorld.decompose(camera.position, camera.quaternion, camera.scale);
  camera.updateMatrixWorld(true);
  const anchorTarget = targetPosition(request, tiles.group.matrixWorld);
  const cameraRight = new Vector3(1, 0, 0).applyQuaternion(camera.quaternion);
  const cameraUp = new Vector3(0, 1, 0).applyQuaternion(camera.quaternion);
  const sunPosition = new Vector3();
  const sunTarget = new Vector3();
  const anchorSunPosition = framePosition(
    request,
    request.lighting.sunAzimuthMillidegrees,
    request.lighting.sunElevationMillidegrees,
    Math.max(2_000, Math.max(horizontalMeters, verticalMeters) * 4),
    tiles.group.matrixWorld,
  );
  const fixedSunVector = anchorSunPosition.sub(anchorTarget);
  sunTarget.copy(anchorTarget);
  sunPosition.copy(anchorTarget).add(fixedSunVector);
  const applyCamera = (next: CaptureRequest): void => {
    const width = next.tile.coreWidthPx + 2 * next.tile.guardPx;
    const height = next.tile.coreHeightPx + 2 * next.tile.guardPx;
    const nextHorizontalMeters = next.camera.orthographicWidthMm / 1_000;
    const nextVerticalMeters = next.camera.orthographicHeightMm / 1_000;
    const centerDelta = targetPosition(next, tiles.group.matrixWorld).sub(anchorTarget);
    const centerX = centerDelta.dot(cameraRight);
    const centerY = centerDelta.dot(cameraUp);
    const frustum = registeredOrthographicFrustum(
      nextHorizontalMeters,
      nextVerticalMeters,
      centerX,
      centerY,
    );
    renderer.setSize(width, height, false);
    camera.left = frustum.left;
    camera.right = frustum.right;
    camera.top = frustum.top;
    camera.bottom = frustum.bottom;
    camera.near = next.camera.nearMm / 1_000;
    camera.far = next.camera.farMm / 1_000;
    camera.updateProjectionMatrix();
    tiles.setCamera(camera);
    tiles.setResolution(camera, width, height);
  };
  applyCamera(request);

  let currentRequest = request;
  let readiness = new TileReadiness(request.readiness);
  let rootLoaded = false;
  let loading = true;
  let loadedModels = 0;
  let terminalError: Error | undefined;
  tiles.addEventListener("load-root-tileset", () => {
    rootLoaded = true;
    readiness.rootLoaded();
  });
  tiles.addEventListener("tiles-load-start", () => {
    loading = true;
    readiness.loadStarted();
  });
  tiles.addEventListener("tiles-load-end", () => {
    loading = false;
    readiness.loadEnded();
  });
  tiles.addEventListener("load-error", () => {
    readiness.loadFailed();
    terminalError = new Error("Google tile load failed");
  });
  tiles.addEventListener("load-model", ({ scene: model }) => {
    loadedModels += 1;
    model.traverse((object) => {
      if (object instanceof Mesh) {
        object.castShadow = true;
        object.receiveShadow = true;
      }
    });
  });
  tiles.addEventListener("dispose-model", () => {
    loadedModels = Math.max(0, loadedModels - 1);
  });

  const waitUntilReady = async () => {
    const ready = await waitForStableReadiness({
      nextFrame: async () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())),
      now: () => performance.now(),
      sample: (now) => {
        tiles.update();
        renderer.render(scene, camera);
        const signature = `${tiles.visibleTiles.size}:${loadedModels}:${tiles.loadProgress.toFixed(6)}`;
        const snapshot = readiness.observe(signature, tiles.visibleTiles.size, now);
        if (terminalError !== undefined) {
          throw terminalError;
        }
        return snapshot;
      },
      timeoutMs: currentRequest.readiness.timeoutMs,
    });
    return {
      elapsedMs: ready.elapsedMs,
      stableFrames: ready.snapshot.stableFrames,
      visibleTiles: ready.snapshot.visibleTiles,
    };
  };

  return {
    attributions(): string[] {
      const values = tiles
        .getAttributions()
        .map(({ type, value }) => `${type}:${String(value)}`)
        .filter((value) => value.length > 1 && value.length <= 2_048);
      return ["Google Maps", ...new Set(values)].slice(0, 64);
    },
    camera,
    diagnostics(): SceneDiagnostics {
      const cache = tiles.lruCache as typeof tiles.lruCache & {
        cachedBytes: number;
        itemSet: Set<unknown>;
      };
      return {
        cachedBytes: cache.cachedBytes,
        cachedTiles: cache.itemSet.size,
        errorTarget: tiles.errorTarget,
        geometries: renderer.info.memory.geometries,
        maxCachedBytes: cache.maxBytesSize,
        textures: renderer.info.memory.textures,
        triangles: renderer.info.render.triangles,
      };
    },
    dispose(): void {
      tiles.dispose();
      draco.dispose();
      renderer.dispose();
    },
    renderer,
    reframe(next: CaptureRequest): void {
      const sameRegistration =
        next.provider === request.provider &&
        next.sourceEpoch === request.sourceEpoch &&
        next.tile.regionId === request.tile.regionId &&
        next.tile.millimetersPerPixel === request.tile.millimetersPerPixel &&
        next.camera.projection === request.camera.projection &&
        next.camera.azimuthMillidegrees === request.camera.azimuthMillidegrees &&
        next.camera.elevationMillidegrees === request.camera.elevationMillidegrees &&
        next.camera.targetAltitudeMm === request.camera.targetAltitudeMm &&
        next.camera.nearMm === request.camera.nearMm &&
        next.camera.farMm === request.camera.farMm &&
        next.camera.cameraDistanceMm === request.camera.cameraDistanceMm &&
        next.lighting.sunAzimuthMillidegrees === request.lighting.sunAzimuthMillidegrees &&
        next.lighting.sunElevationMillidegrees === request.lighting.sunElevationMillidegrees;
      if (!sameRegistration) {
        throw new Error("probe reframe may change only its registered target and bounded grid");
      }
      currentRequest = next;
      readiness = new TileReadiness(next.readiness);
      if (rootLoaded) {
        readiness.rootLoaded();
      }
      if (!loading) {
        readiness.loadEnded();
      }
      terminalError = undefined;
      applyCamera(next);
    },
    scene,
    shadowGrid: {
      heightPx: initialHeight,
      horizontalMeters,
      verticalMeters,
      widthPx: initialWidth,
    },
    sunPosition,
    sunTarget,
    waitUntilReady,
  };
}

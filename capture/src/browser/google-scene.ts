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

const DRACO_DECODER_PATH = "https://www.gstatic.com/draco/versioned/decoders/1.5.7/";

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
  const width = request.tile.coreWidthPx + 2 * request.tile.guardPx;
  const height = request.tile.coreHeightPx + 2 * request.tile.guardPx;
  const renderer = new WebGLRenderer({ antialias: false, canvas, preserveDrawingBuffer: false });
  renderer.setPixelRatio(1);
  renderer.setSize(width, height, false);
  renderer.setClearColor(0x000000, 1);

  const scene = new Scene();
  scene.background = new Color(0x000000);
  const tiles = new TilesRenderer();
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
  const applyCamera = (next: CaptureRequest): void => {
    camera.near = next.camera.nearMm / 1_000;
    camera.far = next.camera.farMm / 1_000;
    WGS84_ELLIPSOID.getObjectFrame(
      (next.tile.centerLatitudeE7 / 10_000_000) * MathUtils.DEG2RAD,
      (next.tile.centerLongitudeE7 / 10_000_000) * MathUtils.DEG2RAD,
      next.camera.targetAltitudeMm / 1_000,
      (next.camera.azimuthMillidegrees / 1_000) * MathUtils.DEG2RAD,
      -(next.camera.elevationMillidegrees / 1_000) * MathUtils.DEG2RAD,
      0,
      camera.matrixWorld,
      CAMERA_FRAME,
    );
    camera.matrixWorld.multiply(
      new Matrix4().makeTranslation(0, 0, next.camera.cameraDistanceMm / 1_000),
    );
    camera.matrixWorld.premultiply(tiles.group.matrixWorld);
    camera.matrixWorld.decompose(camera.position, camera.quaternion, camera.scale);
    camera.updateMatrixWorld(true);
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

  const targetFrame = new Matrix4();
  WGS84_ELLIPSOID.getObjectFrame(
    (request.tile.centerLatitudeE7 / 10_000_000) * MathUtils.DEG2RAD,
    (request.tile.centerLongitudeE7 / 10_000_000) * MathUtils.DEG2RAD,
    request.camera.targetAltitudeMm / 1_000,
    0,
    Math.PI / 2,
    0,
    targetFrame,
    CAMERA_FRAME,
  );
  targetFrame.premultiply(tiles.group.matrixWorld);
  const sunDistanceMeters = Math.max(2_000, horizontalMeters * 4);

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
      const sameGrid =
        next.tile.centerLatitudeE7 === request.tile.centerLatitudeE7 &&
        next.tile.centerLongitudeE7 === request.tile.centerLongitudeE7 &&
        next.tile.coreWidthPx === request.tile.coreWidthPx &&
        next.tile.coreHeightPx === request.tile.coreHeightPx &&
        next.tile.guardPx === request.tile.guardPx &&
        next.tile.millimetersPerPixel === request.tile.millimetersPerPixel &&
        next.camera.orthographicWidthMm === request.camera.orthographicWidthMm &&
        next.camera.orthographicHeightMm === request.camera.orthographicHeightMm;
      if (!sameGrid) {
        throw new Error("probe camera may not change its registered target or pixel grid");
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
    sunPosition: framePosition(
      request,
      request.lighting.sunAzimuthMillidegrees,
      request.lighting.sunElevationMillidegrees,
      sunDistanceMeters,
      tiles.group.matrixWorld,
    ),
    sunTarget: new Vector3().setFromMatrixPosition(targetFrame),
    waitUntilReady,
  };
}

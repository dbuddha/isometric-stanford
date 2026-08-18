import {
  BoxGeometry,
  Color,
  DirectionalLight,
  Group,
  Mesh,
  MeshLambertMaterial,
  OrthographicCamera,
  PlaneGeometry,
  Scene,
  Vector3,
  WebGLRenderer,
} from "three";
import type { CaptureRequest } from "../contracts.js";
import type { RegisteredScene } from "./pass-renderer.js";

export function createSyntheticScene(canvas: HTMLCanvasElement, request: CaptureRequest): RegisteredScene {
  const width = request.tile.coreWidthPx + 2 * request.tile.guardPx;
  const height = request.tile.coreHeightPx + 2 * request.tile.guardPx;
  const renderer = new WebGLRenderer({ antialias: false, canvas, preserveDrawingBuffer: false });
  renderer.setPixelRatio(1);
  renderer.setSize(width, height, false);
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
  camera.position.set(180, 180, 180);
  camera.lookAt(0, 0, 0);
  camera.updateMatrixWorld(true);
  camera.updateProjectionMatrix();

  const scene = new Scene();
  scene.background = new Color(0x5f7d96);
  const group = new Group();
  const ground = new Mesh(
    new PlaneGeometry(horizontalMeters * 4, verticalMeters * 4),
    new MeshLambertMaterial({ color: 0x73915e }),
  );
  ground.rotation.x = -Math.PI / 2;
  ground.receiveShadow = true;
  group.add(ground);
  const tower = new Mesh(new BoxGeometry(28, 90, 28), new MeshLambertMaterial({ color: 0xb88f65 }));
  tower.position.y = 45;
  tower.castShadow = true;
  tower.receiveShadow = true;
  group.add(tower);
  scene.add(group);
  const fill = new DirectionalLight(0xffffff, 2);
  fill.position.set(-100, 200, 100);
  scene.add(fill);

  return {
    attributions: () => ["fixture:synthetic"],
    camera,
    dispose(): void {
      renderer.dispose();
      ground.geometry.dispose();
      (ground.material as MeshLambertMaterial).dispose();
      tower.geometry.dispose();
      (tower.material as MeshLambertMaterial).dispose();
    },
    renderer,
    scene,
    sunPosition: new Vector3(-150, 300, 150),
    sunTarget: new Vector3(0, 0, 0),
    async waitUntilReady() {
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      return { elapsedMs: 1, stableFrames: request.readiness.stableFrames, visibleTiles: 1 };
    },
  };
}

import {
  AmbientLight,
  Color,
  DirectionalLight,
  LinearSRGBColorSpace,
  MeshBasicMaterial,
  MeshLambertMaterial,
  MeshNormalMaterial,
  NoToneMapping,
  OrthographicCamera,
  RGBAFormat,
  Scene,
  ShaderMaterial,
  UnsignedByteType,
  Vector3,
  WebGLRenderer,
  WebGLRenderTarget,
} from "three";
import type { CaptureRequest, LayerName, SceneDiagnostics } from "../contracts.js";

export interface RegisteredScene {
  attributions(): string[];
  camera: OrthographicCamera;
  diagnostics?(): SceneDiagnostics;
  dispose(): void;
  reframe?(request: CaptureRequest): void;
  renderer: WebGLRenderer;
  scene: Scene;
  sunPosition: Vector3;
  sunTarget: Vector3;
  waitUntilReady(): Promise<{ elapsedMs: number; stableFrames: number; visibleTiles: number }>;
}

export interface LayerUpload {
  bytes: Uint8Array;
  height: number;
  name: LayerName;
  pixelFormat: "gray8" | "rgba8" | "u32le-millimeters";
  width: number;
}

const DEPTH_VERTEX_SHADER = `
  varying highp float viewDepth;
  void main() {
    highp vec4 viewPosition = modelViewMatrix * vec4(position, 1.0);
    viewDepth = -viewPosition.z;
    gl_Position = projectionMatrix * viewPosition;
  }
`;

const DEPTH_FRAGMENT_SHADER = `
  varying highp float viewDepth;
  void main() {
    highp float millimeters = floor(max(viewDepth, 0.0) * 1000.0 + 0.5);
    highp float lowByte = mod(millimeters, 256.0);
    millimeters = floor(millimeters / 256.0);
    highp float middleByte = mod(millimeters, 256.0);
    highp float highByte = mod(floor(millimeters / 256.0), 256.0);
    gl_FragColor = vec4(lowByte, middleByte, highByte, 255.0) / 255.0;
  }
`;

function flipRows(bytes: Uint8Array, width: number, height: number): void {
  const stride = width * 4;
  const temporary = new Uint8Array(stride);
  for (let top = 0; top < Math.floor(height / 2); top += 1) {
    const bottom = height - 1 - top;
    const topOffset = top * stride;
    const bottomOffset = bottom * stride;
    temporary.set(bytes.subarray(topOffset, topOffset + stride));
    bytes.copyWithin(topOffset, bottomOffset, bottomOffset + stride);
    bytes.set(temporary, bottomOffset);
  }
}

function grayscale(bytes: Uint8Array): Uint8Array {
  const output = new Uint8Array(bytes.length / 4);
  for (let source = 0, target = 0; source < bytes.length; source += 4, target += 1) {
    const red = bytes[source] ?? 0;
    const green = bytes[source + 1] ?? 0;
    const blue = bytes[source + 2] ?? 0;
    output[target] = Math.round((red * 54 + green * 183 + blue * 19) / 256);
  }
  return output;
}

function depthMillimeters(bytes: Uint8Array, width: number, height: number): Uint8Array {
  const output = new Uint8Array(16 + width * height * 4);
  output.set(new TextEncoder().encode("ISOD32V1"));
  const data = new DataView(output.buffer);
  data.setUint32(8, width, true);
  data.setUint32(12, height, true);
  for (let source = 0, target = 16; source < bytes.length; source += 4, target += 4) {
    const depth =
      (bytes[source] ?? 0) |
      ((bytes[source + 1] ?? 0) << 8) |
      ((bytes[source + 2] ?? 0) << 16);
    data.setUint32(target, depth, true);
  }
  return output;
}

function coreCoverageBasisPoints(
  coverage: Uint8Array,
  request: CaptureRequest,
  width: number,
): number {
  let valid = 0;
  const { coreHeightPx, coreWidthPx, guardPx } = request.tile;
  for (let y = guardPx; y < guardPx + coreHeightPx; y += 1) {
    const start = y * width + guardPx;
    for (let x = 0; x < coreWidthPx; x += 1) {
      if ((coverage[start + x] ?? 0) >= 128) {
        valid += 1;
      }
    }
  }
  return Math.floor((valid * 10_000) / (coreWidthPx * coreHeightPx));
}

export async function renderRegisteredLayers(
  registered: RegisteredScene,
  request: CaptureRequest,
  upload: (layer: LayerUpload) => Promise<void>,
): Promise<number> {
  const width = request.tile.coreWidthPx + request.tile.guardPx * 2;
  const height = request.tile.coreHeightPx + request.tile.guardPx * 2;
  const { camera, renderer, scene } = registered;
  const target = new WebGLRenderTarget(width, height, {
    depthBuffer: true,
    format: RGBAFormat,
    stencilBuffer: false,
    type: UnsignedByteType,
  });
  target.texture.generateMipmaps = false;
  const neutral = new MeshLambertMaterial({ color: 0xc4c1ba });
  const normal = new MeshNormalMaterial();
  const coverage = new MeshBasicMaterial({ color: 0xffffff });
  const shadow = new MeshLambertMaterial({ color: 0xffffff });
  const depth = new ShaderMaterial({
    fragmentShader: DEPTH_FRAGMENT_SHADER,
    vertexShader: DEPTH_VERTEX_SHADER,
  });
  const ambient = new AmbientLight(0xffffff, 1.35);
  const sun = new DirectionalLight(0xffffff, 2.6);
  sun.position.copy(registered.sunPosition);
  sun.target.position.copy(registered.sunTarget);
  sun.castShadow = true;
  sun.shadow.mapSize.set(
    Math.min(4_096, 2 ** Math.ceil(Math.log2(width))),
    Math.min(4_096, 2 ** Math.ceil(Math.log2(height))),
  );
  const horizontalMeters = request.camera.orthographicWidthMm / 1_000;
  const verticalMeters = request.camera.orthographicHeightMm / 1_000;
  sun.shadow.camera.left = -horizontalMeters / 2;
  sun.shadow.camera.right = horizontalMeters / 2;
  sun.shadow.camera.top = verticalMeters / 2;
  sun.shadow.camera.bottom = -verticalMeters / 2;
  sun.shadow.camera.near = 0.1;
  sun.shadow.camera.far = Math.max(5_000, registered.sunPosition.distanceTo(registered.sunTarget) * 2);
  sun.shadow.bias = -0.0001;
  sun.shadow.camera.updateProjectionMatrix();
  renderer.shadowMap.enabled = true;
  renderer.toneMapping = NoToneMapping;
  renderer.outputColorSpace = LinearSRGBColorSpace;

  const renderRgba = (): Uint8Array => {
    const pixels = new Uint8Array(width * height * 4);
    renderer.setRenderTarget(target);
    renderer.clear(true, true, true);
    renderer.render(scene, camera);
    renderer.readRenderTargetPixels(target, 0, 0, width, height, pixels);
    flipRows(pixels, width, height);
    return pixels;
  };

  let coverageBasisPoints = 0;
  try {
    scene.background = new Color(0x000000);
    scene.overrideMaterial = null;
    await upload({ bytes: renderRgba(), height, name: "color", pixelFormat: "rgba8", width });

    scene.add(ambient, sun, sun.target);
    scene.overrideMaterial = neutral;
    await upload({ bytes: renderRgba(), height, name: "whitebox", pixelFormat: "rgba8", width });

    scene.overrideMaterial = depth;
    const depthBytes = depthMillimeters(renderRgba(), width, height);
    await upload({
      bytes: depthBytes,
      height,
      name: "linear-depth",
      pixelFormat: "u32le-millimeters",
      width,
    });

    scene.overrideMaterial = normal;
    await upload({ bytes: renderRgba(), height, name: "view-normal", pixelFormat: "rgba8", width });

    scene.overrideMaterial = shadow;
    const shadowBytes = grayscale(renderRgba());
    await upload({ bytes: shadowBytes, height, name: "fixed-shadow", pixelFormat: "gray8", width });

    scene.overrideMaterial = coverage;
    const coverageBytes = grayscale(renderRgba());
    coverageBasisPoints = coreCoverageBasisPoints(coverageBytes, request, width);
    await upload({ bytes: coverageBytes, height, name: "coverage", pixelFormat: "gray8", width });
  } finally {
    renderer.setRenderTarget(null);
    scene.overrideMaterial = null;
    scene.remove(ambient, sun, sun.target);
    neutral.dispose();
    normal.dispose();
    coverage.dispose();
    shadow.dispose();
    depth.dispose();
    target.dispose();
  }
  return coverageBasisPoints;
}

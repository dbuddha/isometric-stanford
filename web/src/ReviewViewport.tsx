import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent,
  type WheelEvent,
} from "react";

import {
  REFERENCE_LAYER_LABELS,
  depthPreviewPixels,
  type LoadedReferenceLayer,
  type ReferenceLayerKind,
} from "./reference-bundle";

export interface ReviewTransform {
  panX: number;
  panY: number;
  zoom: number;
}

interface LayerAsset {
  kind: ReferenceLayerKind;
  url: string;
}

interface ReviewViewportProps {
  label?: string;
  layer: LoadedReferenceLayer;
  onFailure: (message: string) => void;
  onFitScale?: (scale: number) => void;
  overlay?: LoadedReferenceLayer;
  overlayLabel?: string;
  transform: ReviewTransform;
  updateTransform: (update: (current: ReviewTransform) => ReviewTransform) => void;
  wipePercent?: number;
}

function clampZoom(value: number): number {
  return Math.min(32, Math.max(1 / 32, value));
}

async function layerAsset(layer: LoadedReferenceLayer): Promise<LayerAsset> {
  if (layer.record.kind !== "linear-depth") {
    const bytes = layer.bytes.slice().buffer;
    return {
      kind: layer.record.kind,
      url: URL.createObjectURL(new Blob([bytes], { type: "image/png" })),
    };
  }
  const canvas = document.createElement("canvas");
  canvas.width = layer.record.width_px;
  canvas.height = layer.record.height_px;
  const context = canvas.getContext("2d");
  if (!context) {
    throw new Error("depth preview canvas is unavailable");
  }
  const preview = depthPreviewPixels(layer);
  const previewBuffer = new ArrayBuffer(preview.length);
  const imagePixels = new Uint8ClampedArray(previewBuffer);
  imagePixels.set(preview);
  context.putImageData(
    new ImageData(imagePixels, layer.record.width_px, layer.record.height_px),
    0,
    0,
  );
  const blob = await new Promise<Blob>((resolve, reject) =>
    canvas.toBlob(
      (value) => (value ? resolve(value) : reject(new Error("depth preview encoding failed"))),
      "image/png",
    ),
  );
  return { kind: layer.record.kind, url: URL.createObjectURL(blob) };
}

function useLayerAsset(
  layer: LoadedReferenceLayer | undefined,
  onFailure: (message: string) => void,
): LayerAsset | null {
  const [state, setState] = useState<{
    asset: LayerAsset;
    source: LoadedReferenceLayer;
  } | null>(null);
  useEffect(() => {
    if (!layer) {
      return;
    }
    let disposed = false;
    let created: LayerAsset | null = null;
    void layerAsset(layer)
      .then((next) => {
        created = next;
        if (disposed) {
          URL.revokeObjectURL(next.url);
        } else {
          setState({ asset: next, source: layer });
        }
      })
      .catch((reason: unknown) => {
        if (!disposed) {
          onFailure(reason instanceof Error ? reason.message : "Layer preview preparation failed.");
        }
      });
    return () => {
      disposed = true;
      if (created) {
        URL.revokeObjectURL(created.url);
      }
    };
  }, [layer, onFailure]);
  return state && state.source === layer ? state.asset : null;
}

function LayerImage({
  asset,
  height,
  onFailure,
  scale,
  transform,
  width,
}: {
  asset: LayerAsset | null;
  height: number;
  onFailure: (message: string) => void;
  scale: number;
  transform: ReviewTransform;
  width: number;
}) {
  if (!asset) {
    return <span className="review-viewport__loading">Preparing layer</span>;
  }
  return (
    <img
      alt=""
      className="review-viewport__image"
      data-layer-kind={asset.kind}
      draggable={false}
      height={height}
      onError={() => onFailure(`${asset.kind} image decoding failed`)}
      onLoad={(event) => {
        if (
          event.currentTarget.naturalWidth !== width ||
          event.currentTarget.naturalHeight !== height
        ) {
          onFailure(`${asset.kind} decoded dimensions do not match the registered grid`);
        }
      }}
      src={asset.url}
      style={{
        height: `${height}px`,
        transform: `translate(-50%, -50%) scale(${scale}) translate(${transform.panX}px, ${transform.panY}px)`,
        width: `${width}px`,
      }}
      width={width}
    />
  );
}

export function ReviewViewport({
  label,
  layer,
  onFailure,
  onFitScale,
  overlay,
  overlayLabel,
  transform,
  updateTransform,
  wipePercent = 50,
}: ReviewViewportProps) {
  const host = useRef<HTMLDivElement>(null);
  const drag = useRef<{ pointerId: number; x: number; y: number } | null>(null);
  const [fitScale, setFitScale] = useState(1);
  const primary = useLayerAsset(layer, onFailure);
  const secondary = useLayerAsset(overlay, onFailure);
  const width = layer.record.width_px;
  const height = layer.record.height_px;
  const scale = fitScale * transform.zoom;

  useLayoutEffect(() => {
    const element = host.current;
    if (!element) {
      return;
    }
    const measure = () => {
      const bounds = element.getBoundingClientRect();
      const next = Math.min(bounds.width / width, bounds.height / height);
      if (Number.isFinite(next) && next > 0) {
        setFitScale(next);
        onFitScale?.(next);
      }
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [height, onFitScale, width]);

  const zoom = (factor: number) =>
    updateTransform((current) => ({ ...current, zoom: clampZoom(current.zoom * factor) }));

  const handlePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { pointerId: event.pointerId, x: event.clientX, y: event.clientY };
  };

  const handlePointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const previous = drag.current;
    if (!previous || previous.pointerId !== event.pointerId) {
      return;
    }
    const deltaX = (event.clientX - previous.x) / scale;
    const deltaY = (event.clientY - previous.y) / scale;
    drag.current = { pointerId: event.pointerId, x: event.clientX, y: event.clientY };
    updateTransform((current) => ({
      ...current,
      panX: current.panX + deltaX,
      panY: current.panY + deltaY,
    }));
  };

  const handlePointerUp = (event: PointerEvent<HTMLDivElement>) => {
    if (drag.current?.pointerId === event.pointerId) {
      drag.current = null;
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  };

  const handleWheel = (event: WheelEvent<HTMLDivElement>) => {
    event.preventDefault();
    zoom(event.deltaY < 0 ? 1.25 : 0.8);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const sourceStep = 32 / scale;
    const movement: Partial<Record<string, [number, number]>> = {
      ArrowDown: [0, -sourceStep],
      ArrowLeft: [sourceStep, 0],
      ArrowRight: [-sourceStep, 0],
      ArrowUp: [0, sourceStep],
    };
    const delta = movement[event.key];
    if (delta) {
      event.preventDefault();
      updateTransform((current) => ({
        ...current,
        panX: current.panX + delta[0],
        panY: current.panY + delta[1],
      }));
    } else if (event.key === "+" || event.key === "=") {
      event.preventDefault();
      zoom(1.25);
    } else if (event.key === "-") {
      event.preventDefault();
      zoom(0.8);
    } else if (event.key === "0") {
      event.preventDefault();
      updateTransform(() => ({ panX: 0, panY: 0, zoom: 1 }));
    }
  };

  return (
    <div
      ref={host}
      aria-label={`${label ?? REFERENCE_LAYER_LABELS[layer.record.kind]} registered layer`}
      className="review-viewport"
      data-fit-scale={fitScale.toFixed(6)}
      data-pan-x={transform.panX.toFixed(3)}
      data-pan-y={transform.panY.toFixed(3)}
      data-testid="review-viewport"
      data-zoom={transform.zoom.toFixed(4)}
      onKeyDown={handleKeyDown}
      onPointerCancel={handlePointerUp}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onWheel={handleWheel}
      role="application"
      tabIndex={0}
    >
      <LayerImage
        asset={primary}
        height={height}
        onFailure={onFailure}
        scale={scale}
        transform={transform}
        width={width}
      />
      {overlay && (
        <div
          className="review-viewport__wipe"
          data-testid="wipe-overlay"
          style={{ clipPath: `inset(0 ${100 - wipePercent}% 0 0)` }}
        >
          <LayerImage
            asset={secondary}
            height={height}
            onFailure={onFailure}
            scale={scale}
            transform={transform}
            width={width}
          />
        </div>
      )}
      <span className="review-viewport__label">
        {label ?? REFERENCE_LAYER_LABELS[layer.record.kind]}
      </span>
      {overlay && (
        <span className="review-viewport__label review-viewport__label--right">
          {overlayLabel ?? REFERENCE_LAYER_LABELS[overlay.record.kind]}
        </span>
      )}
      {overlay && (
        <span className="review-viewport__wipe-line" style={{ left: `${wipePercent}%` }} />
      )}
    </div>
  );
}

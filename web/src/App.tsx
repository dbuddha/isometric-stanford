import { useCallback, useEffect, useRef, useState } from "react";
import type OpenSeadragon from "openseadragon";

import { viewerPolicy } from "./viewer-policy";

const RELEASE_DZI = import.meta.env.VITE_DZI_URL as string | undefined;

export function App() {
  const host = useRef<HTMLDivElement>(null);
  const viewer = useRef<OpenSeadragon.Viewer | null>(null);
  const [status, setStatus] = useState(RELEASE_DZI ? "Loading artwork" : "Qualification in progress");
  const [canRetry, setCanRetry] = useState(false);

  useEffect(() => {
    if (!host.current || !RELEASE_DZI) {
      return;
    }

    let disposed = false;
    let terminalTileFailure = false;
    let hasOpened = false;
    let contextCanvas: HTMLCanvasElement | null = null;
    const handleContextLost = (event: Event) => {
      event.preventDefault();
      setCanRetry(true);
      setStatus("Artwork display interrupted. Retry available.");
    };
    const handleContextRestored = () => {
      viewer.current?.forceRedraw();
      setCanRetry(false);
      setStatus("Artwork ready");
    };
    const detachContextHandlers = () => {
      contextCanvas?.removeEventListener("contextlost", handleContextLost);
      contextCanvas?.removeEventListener("contextrestored", handleContextRestored);
      contextCanvas = null;
    };

    void import("openseadragon")
      .then(({ default: createViewer }) => {
        if (disposed || !host.current) {
          return;
        }
        const memory = (navigator as Navigator & { deviceMemory?: number }).deviceMemory;
        const coarsePointer = window.matchMedia("(pointer: coarse)").matches;
        const policy = viewerPolicy(window.innerWidth, memory, coarsePointer);
        host.current.dataset.cacheTileLimit = String(policy.maxImageCacheCount);
        host.current.dataset.decodedBudgetBytes = String(policy.decodedBudgetBytes);
        const instance = createViewer({
          element: host.current,
          tileSources: RELEASE_DZI,
          drawer: "canvas",
          showNavigationControl: false,
          showNavigator: false,
          keyboardNavEnabled: true,
          imageSmoothingEnabled: false,
          preserveViewport: true,
          maxImageCacheCount: policy.maxImageCacheCount,
          immediateRender: false,
          blendTime: 0.08,
          animationTime: 0.45,
          visibilityRatio: 0.7,
          constrainDuringPan: true,
          tileRetryMax: 2,
          tileRetryDelay: 500,
          gestureSettingsTouch: {
            pinchRotate: false,
            flickEnabled: true,
          },
        });
        instance.addHandler("open", () => {
          terminalTileFailure = false;
          if (!hasOpened && policy.initialZoomFactor > 1) {
            instance.viewport.zoomBy(policy.initialZoomFactor);
            instance.viewport.applyConstraints();
          }
          hasOpened = true;
          detachContextHandlers();
          contextCanvas = host.current?.querySelector("canvas") ?? null;
          contextCanvas?.addEventListener("contextlost", handleContextLost);
          contextCanvas?.addEventListener("contextrestored", handleContextRestored);
          setCanRetry(false);
          setStatus("Rendering artwork");
          instance.whenFullyLoaded(() => {
            if (!disposed && !terminalTileFailure) {
              setStatus("Artwork ready");
            }
          });
        });
        instance.addHandler("open-failed", () => {
          setCanRetry(true);
          setStatus("Artwork unavailable. Retry available.");
        });
        instance.addHandler("tile-load-failed", (event) => {
          if (event.maxReached) {
            terminalTileFailure = true;
            setCanRetry(true);
            setStatus("Some artwork tiles failed. Retry available.");
          }
        });
        viewer.current = instance;
      })
      .catch(() => {
        setCanRetry(true);
        setStatus("Artwork viewer failed to start. Retry available.");
      });

    return () => {
      disposed = true;
      detachContextHandlers();
      viewer.current?.destroy();
      viewer.current = null;
    };
  }, []);

  const zoom = useCallback((factor: number) => {
    viewer.current?.viewport.zoomBy(factor);
    viewer.current?.viewport.applyConstraints();
  }, []);

  const home = useCallback(() => viewer.current?.viewport.goHome(), []);

  const retry = useCallback(() => {
    if (!RELEASE_DZI || !viewer.current) {
      window.location.reload();
      return;
    }
    setCanRetry(false);
    setStatus("Loading artwork");
    viewer.current.open(RELEASE_DZI as unknown as OpenSeadragon.TileSourceSpecifier);
  }, []);

  return (
    <main className="app-shell">
      <header className="masthead">
        <div>
          <p className="eyebrow">A procedural campus portrait</p>
          <h1>Isometric Stanford</h1>
        </div>
        <p className="status" role="status">
          <span aria-hidden="true" />
          {status}
        </p>
      </header>

      <section
        className="viewer-frame"
        aria-label="Isometric Stanford map"
        aria-describedby="map-instructions"
      >
        <p id="map-instructions" className="visually-hidden">
          Use arrow keys to pan, plus or minus to zoom, or the map controls to reset the view.
        </p>
        <div className="viewer-grid" aria-hidden="true" />
        <div ref={host} className="viewer" data-testid="viewer" />
        {!RELEASE_DZI && (
          <div className="empty-state">
            <p className="coordinate">37.4195° N to 37.4375° N</p>
            <h2>The map is being built from the world up.</h2>
            <p>
              The public viewer is ready. Original Stanford artwork will appear only after
              the vertical slice passes provenance, visual, seam, determinism, and mobile gates.
            </p>
          </div>
        )}
        <nav className="map-controls" aria-label="Map controls">
          <button type="button" onClick={() => zoom(1.35)} aria-label="Zoom in">
            +
          </button>
          <button type="button" onClick={() => zoom(1 / 1.35)} aria-label="Zoom out">
            −
          </button>
          <button type="button" onClick={home} aria-label="Reset map view">
            Home
          </button>
          {canRetry && (
            <button type="button" onClick={retry} aria-label="Retry artwork">
              Retry
            </button>
          )}
        </nav>
      </section>

      <footer>
        <p>Original procedural artwork. No captured people or vehicles.</p>
        <a href="https://github.com/dbuddha/isometric-stanford">Source and evidence</a>
      </footer>
    </main>
  );
}

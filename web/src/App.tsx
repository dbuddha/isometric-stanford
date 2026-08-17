import { useCallback, useEffect, useRef, useState } from "react";
import type OpenSeadragon from "openseadragon";

const RELEASE_DZI = import.meta.env.VITE_DZI_URL as string | undefined;

function cacheLimit(): number {
  const memory = (navigator as Navigator & { deviceMemory?: number }).deviceMemory;
  return memory !== undefined && memory <= 4 ? 48 : 128;
}

export function App() {
  const host = useRef<HTMLDivElement>(null);
  const viewer = useRef<OpenSeadragon.Viewer | null>(null);
  const [status, setStatus] = useState(RELEASE_DZI ? "Loading artwork" : "Qualification in progress");

  useEffect(() => {
    if (!host.current || !RELEASE_DZI) {
      return;
    }

    let disposed = false;
    void import("openseadragon").then(({ default: createViewer }) => {
      if (disposed || !host.current) {
        return;
      }
      const instance = createViewer({
        element: host.current,
        tileSources: RELEASE_DZI,
        prefixUrl: "https://cdnjs.cloudflare.com/ajax/libs/openseadragon/6.1.0/images/",
        showNavigationControl: false,
        showNavigator: false,
        imageSmoothingEnabled: false,
        maxImageCacheCount: cacheLimit(),
        immediateRender: false,
        blendTime: 0.08,
        animationTime: 0.45,
        visibilityRatio: 0.7,
        constrainDuringPan: true,
        gestureSettingsTouch: {
          pinchRotate: false,
          flickEnabled: true,
        },
      });
      instance.addHandler("open", () => setStatus("Artwork ready"));
      instance.addHandler("open-failed", () => setStatus("Artwork unavailable. Retry shortly."));
      viewer.current = instance;
    });

    return () => {
      disposed = true;
      viewer.current?.destroy();
      viewer.current = null;
    };
  }, []);

  const zoom = useCallback((factor: number) => {
    viewer.current?.viewport.zoomBy(factor);
    viewer.current?.viewport.applyConstraints();
  }, []);

  const home = useCallback(() => viewer.current?.viewport.goHome(), []);

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

      <section className="viewer-frame" aria-label="Isometric Stanford map">
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
        </nav>
      </section>

      <footer>
        <p>Original procedural artwork. No captured people or vehicles.</p>
        <a href="https://github.com/dbuddha/isometric-stanford">Source and evidence</a>
      </footer>
    </main>
  );
}

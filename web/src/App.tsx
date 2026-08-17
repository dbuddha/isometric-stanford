import { useCallback, useEffect, useRef, useState } from "react";
import type OpenSeadragon from "openseadragon";

import { loadReleaseMetadata, type ReleaseMetadata } from "./release-metadata";
import {
  REVIEW_VIEWS,
  reviewHash,
  reviewRectangle,
  reviewViewFromHash,
  supportsLandmarkReview,
  type ReviewViewId,
} from "./review-views";
import { viewerPolicy } from "./viewer-policy";

const RELEASE_DZI = import.meta.env.VITE_DZI_URL as string | undefined;
const RELEASE_MANIFEST = import.meta.env.VITE_RELEASE_URL as string | undefined;

function applyReviewViewport(
  instance: OpenSeadragon.Viewer,
  createViewer: typeof OpenSeadragon,
  release: ReleaseMetadata,
  view: ReviewViewId,
) {
  const rectangle = reviewRectangle(view, release);
  if (!rectangle) {
    instance.viewport.goHome(true);
    return;
  }
  instance.viewport.fitBounds(
    new createViewer.Rect(rectangle.x, rectangle.y, rectangle.width, rectangle.height),
    true,
  );
}

export function App() {
  const host = useRef<HTMLDivElement>(null);
  const viewer = useRef<OpenSeadragon.Viewer | null>(null);
  const viewerFactory = useRef<typeof OpenSeadragon | null>(null);
  const [status, setStatus] = useState(
    RELEASE_DZI
      ? RELEASE_MANIFEST
        ? "Loading artwork"
        : "Artwork evidence configuration missing."
      : "Qualification in progress",
  );
  const [canRetry, setCanRetry] = useState(false);
  const [release, setRelease] = useState<ReleaseMetadata | null>(null);
  const [activeReview, setActiveReview] = useState<ReviewViewId>(() =>
    reviewViewFromHash(window.location.hash),
  );

  useEffect(() => {
    if (!host.current || !RELEASE_DZI) {
      return;
    }
    if (!RELEASE_MANIFEST) {
      return;
    }

    let disposed = false;
    let terminalTileFailure = false;
    let hasOpened = false;
    let handlePopState: (() => void) | null = null;
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

    const metadata = loadReleaseMetadata(RELEASE_MANIFEST);
    void Promise.all([import("openseadragon"), metadata])
      .then(([{ default: createViewer }, loadedRelease]) => {
        if (disposed || !host.current) {
          return;
        }
        setRelease(loadedRelease);
        viewerFactory.current = createViewer;
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
          if (!hasOpened) {
            const requested = reviewViewFromHash(window.location.hash);
            const supported =
              requested === "campus" || supportsLandmarkReview(loadedRelease);
            const initialReview = supported ? requested : "campus";
            if (window.location.hash !== reviewHash(initialReview)) {
              window.history.replaceState(null, "", reviewHash(initialReview));
            }
            setActiveReview(initialReview);
            if (initialReview === "campus" && policy.initialZoomFactor > 1) {
              instance.viewport.zoomBy(policy.initialZoomFactor);
              instance.viewport.applyConstraints();
            } else if (initialReview !== "campus") {
              applyReviewViewport(instance, createViewer, loadedRelease, initialReview);
            }
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
        handlePopState = () => {
          let requested = reviewViewFromHash(window.location.hash);
          if (requested !== "campus" && !supportsLandmarkReview(loadedRelease)) {
            requested = "campus";
          }
          if (window.location.hash !== reviewHash(requested)) {
            window.history.replaceState(null, "", reviewHash(requested));
          }
          applyReviewViewport(instance, createViewer, loadedRelease, requested);
          setActiveReview(requested);
          setStatus("Rendering artwork");
          instance.whenFullyLoaded(() => {
            if (!disposed && !terminalTileFailure) {
              setStatus("Artwork ready");
            }
          });
        };
        window.addEventListener("popstate", handlePopState);
      })
      .catch(() => {
        setCanRetry(true);
        setStatus("Artwork or evidence failed to load. Retry available.");
      });

    return () => {
      disposed = true;
      if (handlePopState) {
        window.removeEventListener("popstate", handlePopState);
      }
      detachContextHandlers();
      viewer.current?.destroy();
      viewer.current = null;
      viewerFactory.current = null;
    };
  }, []);

  const zoom = useCallback((factor: number) => {
    viewer.current?.viewport.zoomBy(factor);
    viewer.current?.viewport.applyConstraints();
  }, []);

  const selectReview = useCallback(
    (view: ReviewViewId) => {
      const instance = viewer.current;
      const createViewer = viewerFactory.current;
      if (!instance || !createViewer || !release) {
        return;
      }
      if (view !== "campus" && !supportsLandmarkReview(release)) {
        return;
      }
      applyReviewViewport(instance, createViewer, release, view);
      setActiveReview(view);
      window.history.pushState(null, "", reviewHash(view));
      setStatus("Rendering artwork");
      instance.whenFullyLoaded(() => setStatus("Artwork ready"));
    },
    [release],
  );

  const home = useCallback(() => selectReview("campus"), [selectReview]);

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

      <aside
        className={`preview-notice${release ? "" : " preview-notice--pending"}`}
        role={release ? "note" : undefined}
        aria-hidden={release ? undefined : true}
        data-testid="release-evidence"
        data-style-id={release?.styleId}
        data-style-sha256={release?.styleSha256}
        data-world-sha256={release?.worldSha256}
        data-tile-set-sha256={release?.tileSetSha256}
      >
        <strong>Unqualified engineering preview</strong>
        <span>
          {release
            ? `Candidate C has not received final visual or landmark approval. ${release.width.toLocaleString()} x ${release.height.toLocaleString()} pixels, ${release.tileCount} deterministic tiles.`
            : "Release evidence is being verified before the artwork is displayed."}
        </span>
      </aside>

      <section
        className="viewer-frame"
        aria-label="Isometric Stanford map"
        aria-describedby="map-instructions"
      >
        <p id="map-instructions" className="visually-hidden">
          Use arrow keys to pan, plus or minus to zoom, or the map controls to reset the view.
        </p>
        <div className="viewer-grid" aria-hidden="true" />
        <div
          ref={host}
          className="viewer"
          data-testid="viewer"
          data-review-view={activeReview}
        />
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
        {release && supportsLandmarkReview(release) && (
          <nav className="review-controls" aria-label="Landmark review views">
            {REVIEW_VIEWS.map((view) => (
              <button
                key={view.id}
                type="button"
                aria-pressed={activeReview === view.id}
                onClick={() => selectReview(view.id)}
              >
                {view.label}
              </button>
            ))}
          </nav>
        )}
      </section>

      <footer>
        <p>Original deterministic procedural artwork. No captured people or vehicles.</p>
        <a href="https://github.com/dbuddha/isometric-stanford">Source and evidence</a>
      </footer>
    </main>
  );
}

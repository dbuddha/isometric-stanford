import { useCallback, useEffect, useMemo, useState } from "react";

import {
  REFERENCE_LAYERS,
  REFERENCE_LAYER_LABELS,
  loadReferenceBundle,
  type LoadedReferenceBundle,
  type ReferenceLayerKind,
} from "./reference-bundle";
import { ReviewViewport, type ReviewTransform } from "./ReviewViewport";
import "./reference-review.css";

const REFERENCE_MANIFEST = import.meta.env.VITE_REFERENCE_URL as string | undefined;
const INITIAL_TRANSFORM: ReviewTransform = { panX: 0, panY: 0, zoom: 1 };

function bytes(value: number): string {
  return new Intl.NumberFormat("en", { maximumFractionDigits: 1 }).format(value / (1024 * 1024));
}

function shortHash(value: string): string {
  return `${value.slice(0, 12)}…${value.slice(-8)}`;
}

function coordinate(value: number): string {
  return (value / 10_000_000).toFixed(7);
}

export function ReviewApp() {
  const [bundle, setBundle] = useState<LoadedReferenceBundle | null>(null);
  const [error, setError] = useState<string | null>(() =>
    REFERENCE_MANIFEST
      ? null
      : "No registered reference bundle is configured for this review session.",
  );
  const [leftKind, setLeftKind] = useState<ReferenceLayerKind>("color");
  const [rightKind, setRightKind] = useState<ReferenceLayerKind>("whitebox");
  const [mode, setMode] = useState<"split" | "wipe">("split");
  const [wipePercent, setWipePercent] = useState(50);
  const [transform, setTransform] = useState<ReviewTransform>(INITIAL_TRANSFORM);
  const [fitScale, setFitScale] = useState(1);

  useEffect(() => {
    if (!REFERENCE_MANIFEST) {
      return;
    }
    const controller = new AbortController();
    void loadReferenceBundle(REFERENCE_MANIFEST, controller.signal)
      .then(setBundle)
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) {
          setError(reason instanceof Error ? reason.message : "Registered reference loading failed.");
        }
      });
    return () => controller.abort();
  }, []);

  const updateTransform = useCallback(
    (update: (current: ReviewTransform) => ReviewTransform) => setTransform(update),
    [],
  );
  const failViewport = useCallback((message: string) => setError(message), []);
  const layers = useMemo(() => bundle?.layers, [bundle]);
  const left = layers?.get(leftKind);
  const right = layers?.get(rightKind);
  const manifest = bundle?.manifest;

  const resetView = () => setTransform(INITIAL_TRANSFORM);
  const nativeView = () =>
    setTransform((current) => ({
      ...current,
      zoom: Math.min(32, Math.max(1 / 32, 1 / fitScale)),
    }));

  return (
    <main
      className="review-shell"
      data-bundle-id={manifest?.bundle_id}
      data-manifest-sha256={bundle?.manifestSha256}
      data-testid="reference-review"
    >
      <header className="review-header">
        <div>
          <a className="review-back" href="./">
            Isometric Stanford
          </a>
          <p className="review-kicker">Registered source evidence</p>
          <h1>Reference review workbench</h1>
        </div>
        <p
          className={`review-status ${error ? "review-status--error" : bundle ? "review-status--verified" : ""}`}
          role="status"
        >
          <span aria-hidden="true" />
          {error ? "Bundle rejected" : bundle ? "Bundle verified" : "Verifying six layers"}
        </p>
      </header>

      {error && (
        <section className="review-failure" role="alert">
          <p className="review-kicker">Fail-closed review state</p>
          <h2>The registered bundle cannot be displayed.</h2>
          <p>{error}</p>
          <code>REFERENCE_BUNDLE_DIRECTORY=/absolute/bundle/path npm --prefix web run dev</code>
        </section>
      )}

      {!bundle && !error && (
        <section className="review-loading" aria-label="Reference bundle verification">
          <span />
          <p>Reading, hashing, and checking layer registration in canonical order.</p>
        </section>
      )}

      {bundle && !error && manifest && left && right && (
        <>
          <section className="review-toolbar" aria-label="Reference review controls">
            <label>
              Left layer
              <select
                aria-label="Left layer"
                onChange={(event) => setLeftKind(event.target.value as ReferenceLayerKind)}
                value={leftKind}
              >
                {REFERENCE_LAYERS.map((kind) => (
                  <option key={kind} value={kind}>
                    {REFERENCE_LAYER_LABELS[kind]}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Right layer
              <select
                aria-label="Right layer"
                onChange={(event) => setRightKind(event.target.value as ReferenceLayerKind)}
                value={rightKind}
              >
                {REFERENCE_LAYERS.map((kind) => (
                  <option key={kind} value={kind}>
                    {REFERENCE_LAYER_LABELS[kind]}
                  </option>
                ))}
              </select>
            </label>
            <div className="review-segmented" aria-label="Comparison mode">
              <button
                aria-pressed={mode === "split"}
                onClick={() => setMode("split")}
                type="button"
              >
                Split
              </button>
              <button
                aria-pressed={mode === "wipe"}
                onClick={() => setMode("wipe")}
                type="button"
              >
                Wipe
              </button>
            </div>
            <div className="review-zoom" aria-label="Pixel navigation">
              <button onClick={resetView} type="button">
                Fit
              </button>
              <button onClick={nativeView} type="button">
                1:1 pixels
              </button>
              <button
                aria-label="Zoom out"
                onClick={() =>
                  setTransform((current) => ({
                    ...current,
                    zoom: Math.max(1 / 32, current.zoom * 0.8),
                  }))
                }
                type="button"
              >
                −
              </button>
              <button
                aria-label="Zoom in"
                onClick={() =>
                  setTransform((current) => ({
                    ...current,
                    zoom: Math.min(32, current.zoom * 1.25),
                  }))
                }
                type="button"
              >
                +
              </button>
            </div>
            {mode === "wipe" && (
              <label className="review-wipe-control">
                Wipe {wipePercent}%
                <input
                  aria-label="Comparison wipe"
                  max="100"
                  min="0"
                  onChange={(event) => setWipePercent(Number(event.target.value))}
                  type="range"
                  value={wipePercent}
                />
              </label>
            )}
          </section>

          <section
            className={`review-canvas-grid ${mode === "wipe" ? "review-canvas-grid--wipe" : ""}`}
            aria-label="Synchronized registered layers"
          >
            {mode === "split" ? (
              <>
                <ReviewViewport
                  layer={left}
                  onFailure={failViewport}
                  onFitScale={setFitScale}
                  transform={transform}
                  updateTransform={updateTransform}
                />
                <ReviewViewport
                  layer={right}
                  onFailure={failViewport}
                  transform={transform}
                  updateTransform={updateTransform}
                />
              </>
            ) : (
              <ReviewViewport
                layer={left}
                onFailure={failViewport}
                onFitScale={setFitScale}
                overlay={right}
                transform={transform}
                updateTransform={updateTransform}
                wipePercent={wipePercent}
              />
            )}
          </section>

          <section className="review-evidence" aria-label="Registered bundle evidence">
            <div>
              <p className="review-kicker">Current immutable experiment input</p>
              <h2>{manifest.bundle_id}</h2>
              <dl className="review-facts">
                <div>
                  <dt>Provider</dt>
                  <dd>{manifest.capture.provider}</dd>
                </div>
                <div>
                  <dt>Source epoch</dt>
                  <dd>{manifest.capture.source_epoch}</dd>
                </div>
                <div>
                  <dt>Registered grid</dt>
                  <dd>
                    {left.record.width_px} × {left.record.height_px} px
                  </dd>
                </div>
                <div>
                  <dt>Core and guard</dt>
                  <dd>
                    {manifest.tile.core_width_px} × {manifest.tile.core_height_px} + {manifest.tile.guard_px}px
                  </dd>
                </div>
                <div>
                  <dt>Ground scale</dt>
                  <dd>{manifest.tile.millimeters_per_pixel} mm/px</dd>
                </div>
                <div>
                  <dt>Center</dt>
                  <dd>
                    {coordinate(manifest.tile.center_latitude_e7)}, {coordinate(manifest.tile.center_longitude_e7)}
                  </dd>
                </div>
                <div>
                  <dt>Coverage</dt>
                  <dd>{(manifest.core_coverage_basis_points / 100).toFixed(2)}%</dd>
                </div>
                <div>
                  <dt>Layer bytes</dt>
                  <dd>{bytes(bundle.totalLayerBytes)} MiB</dd>
                </div>
                <div>
                  <dt>Camera</dt>
                  <dd>
                    {(manifest.camera.azimuth_millidegrees / 1_000).toFixed(1)}° / {(manifest.camera.elevation_millidegrees / 1_000).toFixed(1)}°
                  </dd>
                </div>
                <div>
                  <dt>Fixed sun</dt>
                  <dd>
                    {(manifest.lighting.sun_azimuth_millidegrees / 1_000).toFixed(1)}° / {(manifest.lighting.sun_elevation_millidegrees / 1_000).toFixed(1)}°
                  </dd>
                </div>
              </dl>
            </div>
            <div className="review-hashes">
              <p className="review-kicker">Verified content chain</p>
              <code title={bundle.manifestSha256}>manifest {shortHash(bundle.manifestSha256)}</code>
              {manifest.layers.map((layer) => (
                <code key={layer.kind} title={layer.sha256}>
                  {layer.kind} {shortHash(layer.sha256)}
                </code>
              ))}
              <p className="review-attribution">{manifest.capture.attributions.join(" · ")}</p>
            </div>
          </section>

          <aside className="review-next" role="note">
            <strong>Capture inspection only.</strong> Semantic masks, obstruction repair, and Rust art will appear as separately hashed experiment layers in later pilot tasks. This screen never permits final-pixel painting.
          </aside>
        </>
      )}
    </main>
  );
}

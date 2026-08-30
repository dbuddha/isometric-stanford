import { useCallback, useEffect, useMemo, useState } from "react";
import type { LoadedReferenceLayer } from "./reference-bundle";
import {
  loadQualityImage,
  loadQualityReviewReport,
  type QualityCandidateId,
  type QualityReviewReport,
} from "./quality-evidence";
import { ReviewViewport, type ReviewTransform } from "./ReviewViewport";
import "./reference-review.css";

const QUALITY_REPORT = import.meta.env.VITE_QUALITY_REPORT_URL as string | undefined;
const INITIAL_TRANSFORM: ReviewTransform = { panX: 0, panY: 0, zoom: 1 };

const CROP_TARGETS = [
  { id: "tower", label: "Hoover", x: 0.5, y: 0.5, zoom: 3.2 },
  { id: "trees", label: "Trees", x: 0.4, y: 0.27, zoom: 4 },
  { id: "roofs", label: "Roofs", x: 0.76, y: 0.15, zoom: 4 },
  { id: "construction", label: "Construction", x: 0.76, y: 0.65, zoom: 4 },
] as const;

function bytes(value: number): string {
  return value < 1_048_576
    ? `${(value / 1_024).toFixed(1)} KiB`
    : `${(value / 1_048_576).toFixed(1)} MiB`;
}

function number(value: number): string {
  return new Intl.NumberFormat("en").format(value);
}

export function QualityReviewApp() {
  const [report, setReport] = useState<QualityReviewReport | null>(null);
  const [layers, setLayers] = useState<Map<QualityCandidateId, LoadedReferenceLayer>>(
    () => new Map(),
  );
  const [error, setError] = useState<string | null>(() =>
    QUALITY_REPORT ? null : "No Hoover quality experiment is configured for this review session.",
  );
  const [leftId, setLeftId] = useState<QualityCandidateId>("baseline-sse20-250mm");
  const [rightId, setRightId] = useState<QualityCandidateId>("lod-sse8-250mm");
  const [mode, setMode] = useState<"split" | "wipe">("wipe");
  const [wipePercent, setWipePercent] = useState(50);
  const [transform, setTransform] = useState<ReviewTransform>(INITIAL_TRANSFORM);
  const [fitScale, setFitScale] = useState(1);

  useEffect(() => {
    if (!QUALITY_REPORT) {
      return;
    }
    const controller = new AbortController();
    void loadQualityReviewReport(QUALITY_REPORT, controller.signal)
      .then(setReport)
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) {
          setError(reason instanceof Error ? reason.message : "Quality evidence loading failed.");
        }
      });
    return () => controller.abort();
  }, []);

  const candidates = useMemo(
    () => new Map(report?.candidates.map((candidate) => [candidate.image.candidateId, candidate])),
    [report],
  );

  useEffect(() => {
    if (!QUALITY_REPORT || !report) {
      return;
    }
    const controller = new AbortController();
    for (const id of new Set([leftId, rightId])) {
      if (layers.has(id)) {
        continue;
      }
      const candidate = candidates.get(id);
      if (!candidate) {
        continue;
      }
      void loadQualityImage(QUALITY_REPORT, candidate, controller.signal)
        .then((layer) => setLayers((current) => new Map(current).set(id, layer)))
        .catch((reason: unknown) => {
          if (!controller.signal.aborted) {
            setError(reason instanceof Error ? reason.message : "Quality image loading failed.");
          }
        });
    }
    return () => controller.abort();
  }, [candidates, layers, leftId, report, rightId]);

  const left = layers.get(leftId);
  const right = layers.get(rightId);
  const updateTransform = useCallback(
    (update: (current: ReviewTransform) => ReviewTransform) => setTransform(update),
    [],
  );
  const nativeView = () =>
    setTransform((current) => ({
      ...current,
      zoom: Math.min(32, Math.max(1 / 32, 1 / fitScale)),
    }));
  const focus = (x: number, y: number, zoom: number) => {
    const width = left?.record.width_px ?? 1_024;
    const height = left?.record.height_px ?? 1_024;
    setTransform({
      panX: (0.5 - x) * width,
      panY: (0.5 - y) * height,
      zoom,
    });
  };

  return (
    <main className="review-shell" data-testid="quality-review">
      <header className="review-header">
        <div>
          <a className="review-back" href="./overlap">Overlap evidence</a>
          <p className="review-kicker">Maximum-detail source qualification</p>
          <h1>Hoover reference quality lab</h1>
        </div>
        <p className={`review-status ${error ? "review-status--error" : report ? "review-status--verified" : ""}`} role="status">
          <span aria-hidden="true" />
          {error ? "Evidence rejected" : report ? "LOD ceiling measured" : "Verifying quality evidence"}
        </p>
      </header>

      {error && (
        <section className="review-failure" role="alert">
          <p className="review-kicker">Fail-closed review state</p>
          <h2>The quality experiment cannot be displayed.</h2>
          <p>{error}</p>
          <code>QUALITY_EVIDENCE_DIRECTORY=/absolute/experiment/path npm --prefix web run dev</code>
        </section>
      )}

      {report && !error && (
        <>
          <section className="review-toolbar" aria-label="Quality review controls">
            <label>
              Left evidence
              <select aria-label="Left quality evidence" onChange={(event) => { setLeftId(event.target.value as QualityCandidateId); setTransform(INITIAL_TRANSFORM); }} value={leftId}>
                {report.candidates.map((candidate) => <option key={candidate.image.candidateId} value={candidate.image.candidateId}>{candidate.label}</option>)}
              </select>
            </label>
            <label>
              Right evidence
              <select aria-label="Right quality evidence" onChange={(event) => setRightId(event.target.value as QualityCandidateId)} value={rightId}>
                {report.candidates.map((candidate) => <option key={candidate.image.candidateId} value={candidate.image.candidateId}>{candidate.label}</option>)}
              </select>
            </label>
            <div className="review-segmented" aria-label="Comparison mode" role="group">
              <button aria-pressed={mode === "split"} onClick={() => setMode("split")} type="button">Split</button>
              <button aria-pressed={mode === "wipe"} onClick={() => setMode("wipe")} type="button">Wipe</button>
            </div>
            <div className="review-zoom" aria-label="Pixel navigation" role="group">
              <button onClick={() => setTransform(INITIAL_TRANSFORM)} type="button">Fit</button>
              <button onClick={nativeView} type="button">1:1 pixels</button>
              <button aria-label="Zoom out" onClick={() => setTransform((current) => ({ ...current, zoom: Math.max(1 / 32, current.zoom * 0.8) }))} type="button">−</button>
              <button aria-label="Zoom in" onClick={() => setTransform((current) => ({ ...current, zoom: Math.min(32, current.zoom * 1.25) }))} type="button">+</button>
            </div>
            <div className="review-segmented quality-crops" aria-label="Inspection crops" role="group">
              {CROP_TARGETS.map((crop) => <button key={crop.id} onClick={() => focus(crop.x, crop.y, crop.zoom)} type="button">{crop.label}</button>)}
            </div>
            {mode === "wipe" && (
              <label className="review-wipe-control">
                Wipe {wipePercent}%
                <input aria-label="Quality comparison wipe" max="100" min="0" onChange={(event) => setWipePercent(Number(event.target.value))} type="range" value={wipePercent} />
              </label>
            )}
          </section>

          {left && right ? (
            <section className={`review-canvas-grid ${mode === "wipe" ? "review-canvas-grid--wipe" : ""}`} aria-label="Synchronized quality evidence">
              {mode === "split" ? (
                <>
                  <ReviewViewport label={candidates.get(leftId)?.label} layer={left} onFailure={setError} onFitScale={setFitScale} transform={transform} updateTransform={updateTransform} />
                  <ReviewViewport label={candidates.get(rightId)?.label} layer={right} onFailure={setError} transform={transform} updateTransform={updateTransform} />
                </>
              ) : (
                <ReviewViewport label={candidates.get(leftId)?.label} layer={left} onFailure={setError} onFitScale={setFitScale} overlay={right} overlayLabel={candidates.get(rightId)?.label} transform={transform} updateTransform={updateTransform} wipePercent={wipePercent} />
              )}
            </section>
          ) : (
            <section className="review-loading" aria-label="Quality image verification"><span /><p>Hashing selected source candidates.</p></section>
          )}

          <section className="review-evidence" aria-label="Quality experiment evidence">
            <div>
              <p className="review-kicker">Measured result</p>
              <h2>SSE 8 reaches Google’s available Stanford LOD ceiling.</h2>
              <dl className="review-facts">
                <div><dt>Recommended source</dt><dd>125 mm/px · SSE 8</dd></div>
                <div><dt>Baseline</dt><dd>250 mm/px · SSE 20</dd></div>
                <div><dt>Google requests</dt><dd>{report.network.attempted} / {report.network.requestLimit}</dd></div>
                <div><dt>Root sessions</dt><dd>{report.network.billableRootRequests}</dd></div>
                <div><dt>Failed / blocked</dt><dd>{report.network.failed} / {report.network.blocked}</dd></div>
                <div><dt>Peak process RSS</dt><dd>{bytes(report.runtime.processTree.peak.treeBytes)}</dd></div>
                <div><dt>Camera</dt><dd>330° azimuth · 42° elevation</dd></div>
                <div><dt>Physical footprint</dt><dd>320 × 320 m guarded</dd></div>
                <div><dt>Historical selector</dt><dd>Not available</dd></div>
                <div><dt>SSE 4 benefit</dt><dd>0 requests · identical pixels</dd></div>
              </dl>
            </div>
            <div className="review-hashes">
              <p className="review-kicker">Interpretation</p>
              <code>LOD refinement improves source geometry and textures.</code>
              <code>125 mm/px improves raster sampling, not source geometry.</code>
              <code>Remaining tree facets and construction are source defects.</code>
              <code>Masking and stylization must repair or replace those defects.</code>
            </div>
          </section>

          <section className="overlap-metrics" aria-label="Quality candidate metrics">
            <p className="review-kicker">Controlled candidate measurements</p>
            <div
              aria-label="Scrollable quality candidate measurements"
              className="overlap-metrics__scroll"
              role="region"
              tabIndex={0}
            >
              <table>
                <thead><tr><th>Candidate</th><th>Scale</th><th>SSE</th><th>Requests added</th><th>Visible tiles</th><th>Triangles</th><th>Max tile depth</th><th>Cache</th><th>Coverage</th><th>Capture</th></tr></thead>
                <tbody>
                  {report.candidates.map((candidate) => (
                    <tr key={candidate.image.candidateId}>
                      <th>{candidate.label}</th>
                      <td>{candidate.request.tile.millimetersPerPixel} mm/px</td>
                      <td>{candidate.request.quality.maxScreenSpaceErrorPx}px</td>
                      <td>{number(candidate.image.requestDelta)}</td>
                      <td>{number(candidate.evidence.visibleTiles)}</td>
                      <td>{number(candidate.evidence.diagnostics.triangles)}</td>
                      <td>{candidate.evidence.diagnostics.visibleTileDepthMaximum}</td>
                      <td>{bytes(candidate.evidence.diagnostics.cachedBytes)}</td>
                      <td>{(candidate.evidence.coreCoverageBasisPoints / 100).toFixed(2)}%</td>
                      <td>{(candidate.evidence.elapsedMs / 1_000).toFixed(2)}s</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>

          <aside className="review-next" role="note">
            <strong>Source qualification only.</strong> The higher-detail view is still photogrammetry, not finished art. Trees remain faceted, construction remains visible, and no historical date can be selected. Those are inputs to the mask, repair, and deterministic art stages.
          </aside>
        </>
      )}
    </main>
  );
}

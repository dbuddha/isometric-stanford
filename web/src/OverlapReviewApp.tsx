import { useCallback, useEffect, useMemo, useState } from "react";
import type { LoadedReferenceLayer } from "./reference-bundle";
import {
  CORE_IMAGE_IDS,
  OVERLAP_IMAGE_IDS,
  OVERLAP_IMAGE_LABELS,
  loadOverlapEvidence,
  type LoadedOverlapEvidence,
  type LoadedOverlapImage,
  type OverlapImageId,
} from "./overlap-evidence";
import { ReviewViewport, type ReviewTransform } from "./ReviewViewport";
import "./reference-review.css";

const OVERLAP_REPORT = import.meta.env.VITE_OVERLAP_REPORT_URL as string | undefined;
const INITIAL_TRANSFORM: ReviewTransform = { panX: 0, panY: 0, zoom: 1 };

function asReferenceLayer(image: LoadedOverlapImage): LoadedReferenceLayer {
  return {
    bytes: image.bytes,
    record: {
      byte_length: image.record.byte_length,
      encoding: "png-rgba8",
      height_px: image.record.height_px,
      kind: "color",
      path: image.record.path,
      sha256: image.record.sha256,
      width_px: image.record.width_px,
    },
  };
}

function bytes(value: number): string {
  return value < 1_048_576
    ? `${(value / 1_024).toFixed(1)} KiB`
    : `${(value / 1_048_576).toFixed(1)} MiB`;
}

function pixels(value: number): string {
  return new Intl.NumberFormat("en").format(value);
}

export function OverlapReviewApp() {
  const [evidence, setEvidence] = useState<LoadedOverlapEvidence | null>(null);
  const [error, setError] = useState<string | null>(() =>
    OVERLAP_REPORT ? null : "No registered overlap experiment is configured for this review session.",
  );
  const [scope, setScope] = useState<"core" | "overlap">("core");
  const [leftId, setLeftId] = useState<OverlapImageId>("joined-core");
  const [rightId, setRightId] = useState<OverlapImageId>("monolithic-core");
  const [mode, setMode] = useState<"split" | "wipe">("wipe");
  const [wipePercent, setWipePercent] = useState(50);
  const [transform, setTransform] = useState<ReviewTransform>(INITIAL_TRANSFORM);
  const [fitScale, setFitScale] = useState(1);

  useEffect(() => {
    if (!OVERLAP_REPORT) {
      return;
    }
    const controller = new AbortController();
    void loadOverlapEvidence(OVERLAP_REPORT, controller.signal)
      .then(setEvidence)
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) {
          setError(reason instanceof Error ? reason.message : "Overlap evidence loading failed.");
        }
      });
    return () => controller.abort();
  }, []);

  const available = scope === "core" ? CORE_IMAGE_IDS : OVERLAP_IMAGE_IDS;
  const layers = useMemo(() => {
    if (!evidence) {
      return null;
    }
    return new Map(
      Array.from(evidence.images, ([id, image]) => [id, asReferenceLayer(image)]),
    );
  }, [evidence]);
  const left = layers?.get(leftId);
  const right = layers?.get(rightId);
  const updateTransform = useCallback(
    (update: (current: ReviewTransform) => ReviewTransform) => setTransform(update),
    [],
  );
  const selectScope = (next: "core" | "overlap") => {
    setScope(next);
    setTransform(INITIAL_TRANSFORM);
    if (next === "core") {
      setLeftId("joined-core");
      setRightId("monolithic-core");
    } else {
      setLeftId("overlap-left");
      setRightId("overlap-right");
    }
  };
  const nativeView = () =>
    setTransform((current) => ({
      ...current,
      zoom: Math.min(32, Math.max(1 / 32, 1 / fitScale)),
    }));

  const report = evidence?.report;
  return (
    <main className="review-shell" data-testid="overlap-review">
      <header className="review-header">
        <div>
          <a className="review-back" href="../review">
            Registered layers
          </a>
          <p className="review-kicker">Independent capture qualification</p>
          <h1>Supertile overlap workbench</h1>
        </div>
        <p
          className={`review-status ${error ? "review-status--error" : report?.comparison.passed ? "review-status--verified" : ""}`}
          role="status"
        >
          <span aria-hidden="true" />
          {error
            ? "Evidence rejected"
            : report
              ? report.comparison.passed
                ? "Overlap qualified"
                : report.comparison.gates.source.independent_seam
                  ? "Source seam reproduced"
                  : "Mismatch classified"
              : "Verifying overlap evidence"}
        </p>
      </header>

      {error && (
        <section className="review-failure" role="alert">
          <p className="review-kicker">Fail-closed review state</p>
          <h2>The overlap experiment cannot be displayed.</h2>
          <p>{error}</p>
          <code>OVERLAP_EVIDENCE_DIRECTORY=/absolute/experiment/path npm --prefix web run dev</code>
        </section>
      )}

      {!report && !error && (
        <section className="review-loading" aria-label="Overlap evidence verification">
          <span />
          <p>Hashing seven derived images and checking the one-session experiment contract.</p>
        </section>
      )}

      {report && !error && left && right && (
        <>
          <section className="review-toolbar" aria-label="Overlap review controls">
            <div className="review-segmented" aria-label="Evidence scope">
              <button aria-pressed={scope === "core"} onClick={() => selectScope("core")} type="button">
                Saved cores
              </button>
              <button aria-pressed={scope === "overlap"} onClick={() => selectScope("overlap")} type="button">
                Guard overlap
              </button>
            </div>
            <label>
              Left evidence
              <select aria-label="Left evidence" onChange={(event) => setLeftId(event.target.value as OverlapImageId)} value={leftId}>
                {available.map((id) => <option key={id} value={id}>{OVERLAP_IMAGE_LABELS[id]}</option>)}
              </select>
            </label>
            <label>
              Right evidence
              <select aria-label="Right evidence" onChange={(event) => setRightId(event.target.value as OverlapImageId)} value={rightId}>
                {available.map((id) => <option key={id} value={id}>{OVERLAP_IMAGE_LABELS[id]}</option>)}
              </select>
            </label>
            <div className="review-segmented" aria-label="Comparison mode">
              <button aria-pressed={mode === "split"} onClick={() => setMode("split")} type="button">Split</button>
              <button aria-pressed={mode === "wipe"} onClick={() => setMode("wipe")} type="button">Wipe</button>
            </div>
            <div className="review-zoom" aria-label="Pixel navigation">
              <button onClick={() => setTransform(INITIAL_TRANSFORM)} type="button">Fit</button>
              <button onClick={nativeView} type="button">1:1 pixels</button>
              <button aria-label="Zoom out" onClick={() => setTransform((current) => ({ ...current, zoom: Math.max(1 / 32, current.zoom * 0.8) }))} type="button">−</button>
              <button aria-label="Zoom in" onClick={() => setTransform((current) => ({ ...current, zoom: Math.min(32, current.zoom * 1.25) }))} type="button">+</button>
            </div>
            {mode === "wipe" && (
              <label className="review-wipe-control">
                Wipe {wipePercent}%
                <input aria-label="Comparison wipe" max="100" min="0" onChange={(event) => setWipePercent(Number(event.target.value))} type="range" value={wipePercent} />
              </label>
            )}
          </section>

          <section className={`review-canvas-grid ${mode === "wipe" ? "review-canvas-grid--wipe" : ""}`} aria-label="Synchronized overlap evidence">
            {mode === "split" ? (
              <>
                <ReviewViewport label={OVERLAP_IMAGE_LABELS[leftId]} layer={left} onFailure={setError} onFitScale={setFitScale} transform={transform} updateTransform={updateTransform} />
                <ReviewViewport label={OVERLAP_IMAGE_LABELS[rightId]} layer={right} onFailure={setError} transform={transform} updateTransform={updateTransform} />
              </>
            ) : (
              <ReviewViewport label={OVERLAP_IMAGE_LABELS[leftId]} layer={left} onFailure={setError} onFitScale={setFitScale} overlay={right} overlayLabel={OVERLAP_IMAGE_LABELS[rightId]} transform={transform} updateTransform={updateTransform} wipePercent={wipePercent} />
            )}
          </section>

          <section className="review-evidence" aria-label="Overlap experiment evidence">
            <div>
              <p className="review-kicker">Measured one-session experiment</p>
              <h2>
                {report.comparison.passed
                  ? "Registered join passed"
                  : report.comparison.gates.source.independent_seam
                    ? "Source join passed; lighting unqualified"
                    : "Source join requires remediation"}
              </h2>
              <dl className="review-facts">
                <div><dt>Google requests</dt><dd>{report.network.attempted} / {report.network.requestLimit}</dd></div>
                <div><dt>Root sessions</dt><dd>{report.network.billableRootRequests}</dd></div>
                <div><dt>Completed / failed</dt><dd>{report.network.completed} / {report.network.failed}</dd></div>
                <div><dt>Response formats</dt><dd>{report.network.formats.glb} GLB · {report.network.formats.json} JSON</dd></div>
                <div><dt>Content-Length lower bound</dt><dd>{bytes(report.network.responseBodyBytes)}</dd></div>
                <div><dt>Grid checks</dt><dd>{pixels(report.grid.checkedSavedPixelCenters)}</dd></div>
                <div><dt>Max grid error</dt><dd>{report.grid.maximumPixelCenterErrorPixels.toFixed(4)} px</dd></div>
                <div><dt>Screen-right bearing</dt><dd>{(report.grid.cameraScreenRightBearingMillidegrees / 1_000).toFixed(1)}°</dd></div>
                <div><dt>Fixed camera</dt><dd>{report.cameraRegistration.fixedWorldMatrix ? "verified" : "failed"}</dd></div>
                <div><dt>Source scale</dt><dd>{report.cameraRegistration.horizontalPixelsPerMeter.toFixed(2)} px/m</dd></div>
                <div><dt>Boundary edges</dt><dd>{pixels(report.comparison.boundary_structural_edge_pixels)}</dd></div>
                <div><dt>Registration baseline</dt><dd>{report.comparison.registration_search.baseline_above_tolerance_ppm} ppm</dd></div>
                <div><dt>Best registration</dt><dd>{report.comparison.registration_search.best_dx_px}, {report.comparison.registration_search.best_dy_px} px · {report.comparison.registration_search.best_above_tolerance_ppm} ppm</dd></div>
                <div><dt>Process tree peak</dt><dd>{bytes(report.runtime.processTree.peakProcessTreeRssBytes)}</dd></div>
                <div><dt>Worker envelope</dt><dd>{bytes(report.runtime.workerEnvelopeBytes)}</dd></div>
                <div><dt>Failure classes</dt><dd>{report.comparison.failure_classifications.join(", ") || "none"}</dd></div>
                <div><dt>Independent source seam</dt><dd>{report.comparison.gates.source.independent_seam ? "pass" : "fail"}</dd></div>
                <div><dt>Monolithic source oracle</dt><dd>{report.comparison.gates.source.monolithic_seam ? "pass" : "fail"}</dd></div>
                <div><dt>Captured lighting seam</dt><dd>{report.comparison.gates.lighting_seam ? "pass" : "fail"}</dd></div>
              </dl>
            </div>
            <div className="review-hashes">
              <p className="review-kicker">Candidate coverage and readiness</p>
              {report.candidates.map((candidate) => (
                <code key={candidate.candidateId}>
                  {candidate.candidateId} {(candidate.evidence.coreCoverageBasisPoints / 100).toFixed(2)}% · {candidate.evidence.visibleTiles} tiles · {(candidate.evidence.elapsedMs / 1_000).toFixed(2)}s
                </code>
              ))}
            </div>
          </section>

          <section className="overlap-metrics" aria-label="Layer comparison metrics">
            <p className="review-kicker">Exact and bounded seam oracle</p>
            <div className="overlap-metrics__scroll">
              <table>
                  <thead><tr><th>Layer</th><th>Independent exact mismatch</th><th>Full overlap above tolerance</th><th>Saved seam corridor</th><th>Joined seam oracle</th><th>All seam relations</th></tr></thead>
                <tbody>
                  {Object.entries(report.comparison.layers).map(([name, metrics]) => (
                    <tr key={name}>
                      <th>{name}</th>
                      <td>{pixels(metrics.left_vs_right_overlap.exact_mismatch_pixels)}</td>
                      <td>{metrics.left_vs_right_overlap.pixels_above_tolerance_ppm} ppm</td>
                      <td>{metrics.left_vs_right_seam_corridor.pixels_above_tolerance_ppm} ppm</td>
                      <td>{metrics.joined_boundary_vs_monolithic.pixels_above_tolerance_ppm} ppm</td>
                      <td>{metrics.left_vs_right_seam_corridor.passed && metrics.joined_boundary_vs_monolithic.passed ? "pass" : "fail"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>

          <aside className="review-next" role="note">
            <strong>Reference capture only.</strong> Red heatmap pixels are depth, normal, or coverage disagreement. Green records whitebox or fixed-shadow disagreement. Blue records color-only disagreement. No mask or art transform participates in this experiment.
          </aside>
        </>
      )}
    </main>
  );
}

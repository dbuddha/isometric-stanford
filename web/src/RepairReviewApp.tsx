import { useCallback, useEffect, useMemo, useState } from "react";
import {
  loadRepairImage,
  loadRepairReviewReport,
  type RepairImageId,
  type RepairReviewReport,
} from "./repair-evidence";
import type { LoadedReferenceLayer } from "./reference-bundle";
import { ReviewViewport, type ReviewTransform } from "./ReviewViewport";
import "./reference-review.css";

const REPAIR_REPORT = import.meta.env.VITE_REPAIR_REPORT_URL as string | undefined;
const INITIAL_TRANSFORM: ReviewTransform = { panX: 0, panY: 0, zoom: 1 };
const CROPS = [
  { label: "Hoover", x: 0.5, y: 0.5, zoom: 3.2 },
  { label: "Trees", x: 0.42, y: 0.28, zoom: 4 },
  { label: "Construction", x: 0.75, y: 0.62, zoom: 4 },
  { label: "Cars", x: 0.79, y: 0.86, zoom: 5 },
] as const;

function number(value: number): string {
  return new Intl.NumberFormat("en").format(value);
}
function bytes(value: number): string {
  return `${(value / 1_048_576).toFixed(1)} MiB`;
}

export function RepairReviewApp() {
  const [report, setReport] = useState<RepairReviewReport | null>(null);
  const [layers, setLayers] = useState<Map<RepairImageId, LoadedReferenceLayer>>(() => new Map());
  const [error, setError] = useState<string | null>(() =>
    REPAIR_REPORT ? null : "No reference-repair experiment is configured for this review session.",
  );
  const [leftId, setLeftId] = useState<RepairImageId>("source-logical");
  const [rightId, setRightId] = useState<RepairImageId>("candidate-c-canopy-repair");
  const [mode, setMode] = useState<"split" | "wipe">("wipe");
  const [wipePercent, setWipePercent] = useState(50);
  const [transform, setTransform] = useState<ReviewTransform>(INITIAL_TRANSFORM);
  const [fitScale, setFitScale] = useState(1);

  useEffect(() => {
    if (!REPAIR_REPORT) return;
    const controller = new AbortController();
    void loadRepairReviewReport(REPAIR_REPORT, controller.signal)
      .then(setReport)
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) setError(reason instanceof Error ? reason.message : "Repair evidence failed.");
      });
    return () => controller.abort();
  }, []);

  const records = useMemo(() => new Map(report?.images.map((image) => [image.id, image])), [report]);
  useEffect(() => {
    if (!REPAIR_REPORT || !report) return;
    const controller = new AbortController();
    for (const id of new Set([leftId, rightId])) {
      if (layers.has(id)) continue;
      const image = records.get(id);
      if (!image) continue;
      void loadRepairImage(REPAIR_REPORT, image, controller.signal)
        .then((layer) => setLayers((current) => new Map(current).set(id, layer)))
        .catch((reason: unknown) => {
          if (!controller.signal.aborted) setError(reason instanceof Error ? reason.message : "Repair image failed.");
        });
    }
    return () => controller.abort();
  }, [layers, leftId, records, report, rightId]);

  const left = layers.get(leftId);
  const right = layers.get(rightId);
  const updateTransform = useCallback(
    (update: (current: ReviewTransform) => ReviewTransform) => setTransform(update),
    [],
  );
  const focus = (x: number, y: number, zoom: number) => {
    const width = left?.record.width_px ?? 1_024;
    const height = left?.record.height_px ?? 1_024;
    setTransform({ panX: (0.5 - x) * width, panY: (0.5 - y) * height, zoom });
  };
  const nativeView = () => setTransform((current) => ({
    ...current,
    zoom: Math.min(32, Math.max(1 / 32, 1 / fitScale)),
  }));

  return (
    <main className="review-shell" data-testid="repair-review">
      <header className="review-header">
        <div>
          <a className="review-back" href="./quality">Source quality</a>
          <p className="review-kicker">Deterministic CV qualification</p>
          <h1>Hoover reference repair lab</h1>
        </div>
        <p className={`review-status ${error ? "review-status--error" : report ? "" : ""}`} role="status">
          <span aria-hidden="true" />
          {error ? "Evidence rejected" : report ? "Pilot not qualified" : "Verifying repair evidence"}
        </p>
      </header>

      {error && <section className="review-failure" role="alert"><h2>The repair experiment cannot be displayed.</h2><p>{error}</p><code>REPAIR_EVIDENCE_DIRECTORY=/absolute/experiment/path npm --prefix web run dev</code></section>}

      {report && !error && <>
        <section className="review-toolbar" aria-label="Repair review controls">
          <label>Left evidence<select aria-label="Left repair evidence" value={leftId} onChange={(event) => { setLeftId(event.target.value as RepairImageId); setTransform(INITIAL_TRANSFORM); }}>{report.images.map((image) => <option key={image.id} value={image.id}>{image.label}</option>)}</select></label>
          <label>Right evidence<select aria-label="Right repair evidence" value={rightId} onChange={(event) => setRightId(event.target.value as RepairImageId)}>{report.images.map((image) => <option key={image.id} value={image.id}>{image.label}</option>)}</select></label>
          <div className="review-segmented" role="group" aria-label="Comparison mode"><button type="button" aria-pressed={mode === "split"} onClick={() => setMode("split")}>Split</button><button type="button" aria-pressed={mode === "wipe"} onClick={() => setMode("wipe")}>Wipe</button></div>
          <div className="review-zoom" role="group" aria-label="Pixel navigation"><button type="button" onClick={() => setTransform(INITIAL_TRANSFORM)}>Fit</button><button type="button" onClick={nativeView}>1:1 pixels</button><button type="button" aria-label="Zoom out" onClick={() => setTransform((current) => ({ ...current, zoom: current.zoom * 0.8 }))}>−</button><button type="button" aria-label="Zoom in" onClick={() => setTransform((current) => ({ ...current, zoom: current.zoom * 1.25 }))}>+</button></div>
          <div className="review-segmented quality-crops" role="group" aria-label="Inspection crops">{CROPS.map((crop) => <button type="button" key={crop.label} onClick={() => focus(crop.x, crop.y, crop.zoom)}>{crop.label}</button>)}</div>
          {mode === "wipe" && <label className="review-wipe-control">Wipe {wipePercent}%<input aria-label="Repair comparison wipe" type="range" min="0" max="100" value={wipePercent} onChange={(event) => setWipePercent(Number(event.target.value))} /></label>}
        </section>

        {left && right ? <section className={`review-canvas-grid ${mode === "wipe" ? "review-canvas-grid--wipe" : ""}`} aria-label="Synchronized repair evidence">
          {mode === "split" ? <><ReviewViewport label={records.get(leftId)?.label} layer={left} onFailure={setError} onFitScale={setFitScale} transform={transform} updateTransform={updateTransform} /><ReviewViewport label={records.get(rightId)?.label} layer={right} onFailure={setError} transform={transform} updateTransform={updateTransform} /></> : <ReviewViewport label={records.get(leftId)?.label} layer={left} onFailure={setError} onFitScale={setFitScale} overlay={right} overlayLabel={records.get(rightId)?.label} transform={transform} updateTransform={updateTransform} wipePercent={wipePercent} />}
        </section> : <section className="review-loading"><span /><p>Hashing selected repair images.</p></section>}

        <section className="review-evidence" aria-label="Repair experiment evidence">
          <div><p className="review-kicker">Measured result</p><h2>Deterministic filtering is viable, but this pilot does not qualify expansion.</h2><dl className="review-facts"><div><dt>Source / logical scale</dt><dd>{report.source_millimeters_per_pixel} / {report.logical_millimeters_per_pixel} mm</dd></div><div><dt>Camera</dt><dd>{report.camera_azimuth_millidegrees / 1_000}° / {report.camera_elevation_millidegrees / 1_000}°</dd></div><div><dt>Canopy repaired</dt><dd>{number(report.canopy_pixels)} px</dd></div><div><dt>Structural evidence</dt><dd>{number(report.structural_edge_pixels)} px</dd></div><div><dt>Working estimate</dt><dd>{bytes(report.estimated_peak_working_bytes)}</dd></div><div><dt>Cars</dt><dd>preserved by policy</dd></div></dl></div>
          <div className="review-hashes"><p className="review-kicker">Blocking findings</p>{report.blocking_findings.map((finding) => <code key={finding}>{finding}</code>)}</div>
        </section>

        <section className="overlap-metrics" aria-label="Repair candidate metrics"><p className="review-kicker">Controlled candidate measurements</p><div className="overlap-metrics__scroll" role="region" tabIndex={0}><table><thead><tr><th>Candidate</th><th>Colors</th><th>Structural recall</th><th>Canopy edge density</th><th>Non-structural edges</th></tr></thead><tbody>{report.candidates.map((candidate) => <tr key={candidate.candidate_id}><th>{candidate.candidate_id}</th><td>{candidate.colors_used}</td><td>{(candidate.structural_edge_recall_basis_points / 100).toFixed(2)}%</td><td>{number(candidate.canopy_interior_edge_ppm)} ppm</td><td>{number(candidate.non_structural_edge_ppm)} ppm</td></tr>)}</tbody></table></div></section>

        <aside className="review-next" role="note"><strong>Interpretation.</strong> Candidate A is the filter ceiling. Candidate B proves depth and normal edges can be retained. Candidate C reduces canopy texture noise and preserves registered passenger cars. Construction remains review-blocking because no accepted instance mask exists.</aside>
      </>}
    </main>
  );
}

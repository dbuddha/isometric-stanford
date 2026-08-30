# Project decisions

## Include

1. Pin 330 degrees azimuth, 42 degrees elevation, SSE 8, 20 meter target
   altitude, 2,000 meter camera distance, and a 1 to 5,000 meter clipping
   interval. Use 125 millimeters per pixel for the maximum-detail review master
   and 250 millimeters per pixel for the efficient reference.
2. Construct one world camera per bounded macroblock. Move adjacent source
   cells only with off-axis orthographic frusta.
3. Capture at least a 1,024-pixel core with a 128-pixel guard. Prefer a 2,048
   by 1,024 macroblock where memory permits, then crop publication cells.
4. Use TypeScript, Three.js, `3d-tiles-renderer`, and pinned Chromium for Google
   streaming and registered GPU passes.
5. Use safe Rust for raw archive validation, source comparison, masks, repair,
   deterministic relighting, stylization, seam oracles, hashing, and DZI.
6. Use React for a local hash-validating review surface. Use OpenSeadragon only
   for the static final image pyramid.
7. Use the cache and worker envelope pinned by each capture spec. The SSE 8
   maximum-detail probe permits up to 2 GiB renderer cache and uses a measured
   2 GiB process-tree envelope. Lower-detail overlap evidence retains its 128
   MiB target, 256 MiB ceiling, and 1.25 GiB envelope.
8. Freeze accepted raw layer hashes before any nondeterministic model inference
   or deterministic Rust transform.

## Exclude

1. Rebuilding the Google client, glTF decoder, Draco decoder, or browser GPU
   renderer in Rust.
2. A live Google 3D Tiles frontend for final users.
3. Camera recentering per source cell.
4. Treating captured color brightness or Google shadowing as final art light.
5. Calling the current monolithic or lighting relationship qualified.
6. Using Qwen to solve source capture or source seams.
7. Collecting the 600-meter hero or full campus before the remaining issue
   #167 exit evidence is resolved or deliberately changed by an approved ADR.
8. Treating an API key or informal permission as a written exception to Google
   Map Tiles policy.

## Experiment next

1. Run one later fixed-camera retest after the current unshadowed-whitebox and
   block-anchored-shadow implementation reaches an exact reviewed revision.
2. Add the vertical neighbor relation and a three-cell horizontal chain using
   frozen or one-session evidence.
3. Investigate whether reusable tile selection or a fixed refinement floor can
   make the monolithic oracle comparable without cache starvation.
4. Measure streamed body bytes without buffering or cloning Google responses.
5. Qualify a macroblock resume path, partial rerun, and source-epoch boundary.

## Defer

- Art-style conversion, semantic detectors, and obstruction repair until the
  registered reference dataset is accepted.
- GPU Rust stylization until the canonical CPU path misses a measured budget.
- deck.gl or Cesium migration until the current exporter lacks a required
  capability under measurement.

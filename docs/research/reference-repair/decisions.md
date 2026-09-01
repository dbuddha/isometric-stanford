# Project decisions

## Include now

1. Keep `reference-repair-rust/v1` as the deterministic comparison baseline.
2. Keep Candidate A and Candidate B as controls. Never optimize them away.
3. Keep Candidate C as the first accepted engineering baseline, not an approved
   art style.
4. Preserve registered passenger cars. Do not spend mask or repair effort on
   them by default.
5. Freeze and hash any learned model output before canonical Rust processing.
6. Require source, candidate, mask, edge, metric, and blocker visibility in the
   dashboard for every repair experiment.
7. Require split and correctly oriented wipe comparisons on desktop and mobile.
8. Keep all Google and transformed pixels private under ignored artifact
   storage until publication rights are recorded.

## Implement next

1. Create accepted mask fixtures for construction, source artifacts, roof
   planes, facade openings, roads, paths, markings, canopy, people, bicycles,
   buses, trucks, and persistent passenger cars.
2. Benchmark Mask2Former or an equivalent dense model against those fixtures.
   Use SAM-style prompting only to refine or correct masks.
3. Add a deterministic surface graph from depth, normals, connected planes,
   line intersections, and accepted semantic instances.
4. Repair construction and broken-source pixels only when an accepted
   underlying surface is supported. Otherwise preserve `unknown` and block.
5. Regularize roofs and facade openings inside accepted instance boundaries.
6. Add road, path, curb, and marking grammar only after their masks pass.
7. Re-run the same A/B/C evidence protocol on Hoover, Main Quad, and a road
   scene before another capture expansion.

## Exclude

1. More generic sharpening, denoising, or palette tuning presented as semantic
   repair.
2. Qwen or another generative model for pilot final pixels.
3. Automatic acceptance of pretrained segmentation without Stanford fixtures.
4. Manual final-pixel painting.
5. Another paid Google capture merely to avoid defects already visible in the
   accepted frozen bundle.
6. GPU or WGPU migration while the measured CPU transform remains far inside
   the budget.

## Escalation rule

Evaluate a tightly controlled generative asset or image-edit path only if the
reviewed semantic surface graph and two deliberate material-grammar iterations
still cannot achieve owner-approved landmark and ordinary-scene quality. That
decision requires a new ADR and must preserve the deterministic non-generative
baseline as comparison evidence.

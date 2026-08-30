---
title: "Registered Google capture, quality, and overlap"
slug: "registered-capture"
status: "active"
research_kind: "source archaeology and controlled differential experiment"
question: "Which camera, capture grid, renderer boundary, and join contract can produce clean registered Stanford reference mosaics without spending campus-scale quota?"
decision_owner: "dbuddha"
last_reviewed: "2026-08-30"
review_due: "Before the first campus macroblock collection or after a capture-renderer revision"
evidence_level: "E3"
upstream_revisions:
  - "Google Maps Tile API documentation, last updated 2026-08-25"
  - "cannoneyed/isometric-nyc@008446357ec67512c4329d25edefb6c508c7b24d"
  - "NASA-AMMOS/3DTilesRendererJS@10e9dc969ba5fdd27a83fd47149a2b8eae841741"
  - "visgl/deck.gl@a91c56d3a4ba22fbfaa520bba2421e7309db1689"
  - "CesiumGS/cesium@93c82442d0733002e48c2dfde7b1a43d75da57dc"
related_issues: [167, 171]
related_requirements: [141]
supersedes: []
---

# Registered Google capture, quality, and overlap

## Decision answer

Use a fixed 330 degree azimuth, 42 degree elevation orthographic camera. Use
SSE 8 and 125 millimeters per output pixel for maximum-detail review. Use SSE 8
and 250 millimeters per pixel for the smaller efficient reference. Keep the
camera world matrix fixed across a bounded macroblock and move adjacent
captures with off-axis projection windows. Camera distance remains 2,000
meters only for clipping safety. It does not control orthographic scale.

The quality experiment separated source LOD from output sampling. Moving from
SSE 20 to 8 increased selected triangles from 237,150 to 1,370,554. Moving
from SSE 8 to 4 added no requests and produced byte-identical images. Doubling
the output dimensions at 125 millimeters per pixel improved native inspection
without adding source geometry. Remaining faceted trees, construction, and
missing photogrammetry are source defects for the later mask and repair stages.
The API has no documented historical 3D Tiles selector.

Keep Google scene streaming and GPU capture in TypeScript, Three.js, and
`3d-tiles-renderer`. Keep immutable validation, comparison, future masks,
stylization, stitching, hashes, and DZI publication in safe Rust. Keep the
local evidence workbench in React. The final public browser remains a static
OpenSeadragon DZI viewer rather than a live 3D Tiles client.

The fixed-camera experiment reproduced a visually clean two-cell Hoover join.
Within the 64-pixel saved seam corridor, color, coverage, linear depth, and
view normals passed their bounded source gates. A camera-recentered control
failed badly because screen-space traversal selected different Google levels
of detail. Captured whitebox and shadow layers also failed, so source
registration is reproduced but the complete requirement is not qualified.
Issue #167 stays open.

## Renderer and application comparison

| Option | Strength | Cost for this workload | Disposition |
| --- | --- | --- | --- |
| Three.js plus `3d-tiles-renderer` | Direct camera, framebuffer, material override, Google auth, glTF and Draco access | Requires owning readiness, memory, and export evidence | Selected for registered multipass capture |
| deck.gl `Tile3DLayer` | Excellent interactive overlays, attribution traversal, 512 MB adaptive-memory precedent | Its higher-level layer lifecycle is less direct for six exact offscreen passes | Study adaptive memory and overlay UX; do not migrate |
| CesiumJS | Most mature globe, navigation, geocoder, collision, and tileset product | Default Google helper allows roughly 2.5 GiB cache plus overflow and brings a larger globe stack | Useful comparator; too heavy for the exporter |
| Google 3D Storytelling and 3D Area Explorer | Good product patterns for places, camera navigation, panels, and overlays | Perspective applications, not registered image pipelines | Borrow review UX ideas only. This is the relevant "storytelling" precedent, not the Storybook component tool. |
| Custom Rust/WGPU 3D Tiles client | Potential native control | Would duplicate Google session, 3D Tiles traversal, glTF, Draco, LOD, attribution, and GPU rendering before masks begin | Reject for acquisition; use Rust immediately after frozen capture |
| Isometric NYC stack | Closest public orthographic capture, generation dashboard, quadrant planning, and DZI precedent | Qwen, ordered infill, and manual correction are nondeterministic art operations | Reuse workflow lessons, not its art dependency graph |

## Scope

Included:

- Current Google Photorealistic 3D Tiles protocol, quota, and renderer boundary.
- Hoover camera orientation, source LOD, output sampling, geographic stepping,
  and two-cell overlap.
- Source formats, cache behavior, memory, readiness, and local review.
- Isometric NYC capture and infill behavior at one pinned revision.
- `3d-tiles-renderer`, deck.gl, CesiumJS, and Google solution precedents.

Excluded:

- Art-style approval or proof that deterministic stylization matches Isometric NYC.
- Semantic segmentation, vehicle removal, or obstruction repair.
- Campus-scale collection, long-session residency, or public release rights.
- A claim that the full raw image equals a separately traversed monolithic render.

## Evidence that would change the decision

- A fixed-camera retest in which unshadowed whitebox and block-anchored shadows
  fail the independent seam corridor.
- A renderer revision that offers fixed, reusable tile selection and proves an
  exact monolithic oracle without increasing memory or source loss.
- A Rust 3D Tiles client that matches the current browser stack's glTF, Draco,
  Google session, attribution, LOD, and multipass behavior with lower measured
  cost.
- Fixed-device evidence that OpenSeadragon cannot meet the final interaction
  budget after ordinary tuning.

## Package map

- [Source map](source-map.md)
- [Findings](findings.md)
- [Experiments](experiments.md)
- [Project decisions](alpine-decisions.md)
- [Bibliography](references.bib)

# ADR 0008: Google-only geographic source

- Status: Accepted by explicit owner direction
- Date: 2026-08-30
- Supersedes: ADR 0003 for the active masking and stylization pipeline

## Context

The procedural open-data prototype proved deterministic rendering, publication,
and browser delivery but did not preserve enough visible Stanford geometry or
detail to support the intended reference-derived art. The accepted recovery
plan requires Google Photorealistic 3D Tiles to supply all geographic geometry,
textures, object placement, terrain, buildings, roads, and vegetation.

## Decision

Use registered Google Photorealistic 3D Tiles as the sole geographic source for
the active pipeline. Compile frozen overlapping captures into one canonical
ReferenceAtlas before masks, repair, stylization, or seam validation.

Open-source libraries, pretrained CV weights, and original non-geographic art
assets may process that atlas. Qwen and other image generators do not produce
final pixels. Existing OSM, Overture, NAIP, and LiDAR artifacts remain only as
historical evidence for the rejected procedural baseline and cannot enter the
active production lineage.

## Consequences

The active pipeline cannot use geographic priors from the legacy world. It must
derive surfaces, objects, masks, repairs, and visual detail from registered
Google color, depth, normals, whitebox, coverage, and reviewed corrections.
Public release remains blocked until the asserted authorization is retained in
the provenance record for the exact transformed-publication workflow.

CI enforces the dependency boundary. The legacy baseline stays buildable so
accepted deterministic and browser evidence is not erased or misrepresented as
new production input.

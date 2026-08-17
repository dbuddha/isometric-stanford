# Isometric Stanford

Isometric Stanford is an evidence-driven project to build an original,
deterministic isometric artwork and web map of Stanford campus. Licensed
geospatial sources compile into a versioned semantic world. A procedural Rust
renderer turns that world into crisp, late-1990s city-builder-style pixel art,
then a static OpenSeadragon viewer serves a DZI/WebP pyramid efficiently on
desktop and mobile.

The project begins with a qualification slice bounded by:

| Edge | Coordinate |
| --- | ---: |
| West | -122.1900 |
| East | -122.1580 |
| South | 37.4195 |
| North | 37.4375 |

The slice includes Lake Lagunita, the Main Quad, Memorial Church, Hoover Tower,
central campus, Stanford Stadium, parking, paths, athletic surfaces, dense
vegetation, and dry terrain. Full-estate work will not begin until this slice
passes semantic, visual, determinism, seam, performance, provenance, and web
qualification.

## Non-negotiable boundaries

- Final artwork is procedural and does not contain captured source pixels.
- People, cars, buses, cranes, and temporary equipment are excluded from the
  final world and renderer assets.
- Open data is the production baseline.
- Google-derived content is prohibited unless written permission explicitly
  permits the intended derivative production and publication workflow.
- Qwen does not produce final artwork.
- A deterministic fixed-point CPU renderer is the v1 rendering boundary.
- OpenSeadragon and static DZI/WebP are the v1 browser boundary.
- Style approval and release publication remain human-owned decisions.

## Status

Repository bootstrap is in progress. The vertical slice is not yet qualified
and no map release has been published.

## Licensing

Rust, Python, TypeScript, and other source code are available under either the
MIT License or Apache License 2.0, at your option. Original project artwork and
documentation are licensed under CC BY 4.0. Third-party data and assets retain
their original licenses and must be recorded before use. See
[ATTRIBUTION.md](ATTRIBUTION.md) for the governing policy.

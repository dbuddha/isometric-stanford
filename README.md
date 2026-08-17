# Isometric Stanford

Isometric Stanford is an evidence-driven project to build an original,
deterministic isometric artwork and web map of Stanford campus. Licensed
geospatial sources compile into a versioned semantic world. A procedural Rust
renderer turns that world into crisp, late-1990s city-builder-style pixel art,
then a static OpenSeadragon viewer serves a DZI/WebP pyramid efficiently on
desktop and mobile.

The project now begins with a prototype hero area bounded by:

| Edge | Coordinate |
| --- | ---: |
| West | -122.1722 |
| East | -122.1653 |
| South | 37.4245 |
| North | 37.4299 |

The continuous approximately 600 by 600 meter prototype contains Hoover Tower,
the Main Quad, Memorial Church, roads, paths, ordinary campus buildings, and
vegetation. It must prove the complete deterministic source-to-browser path
before the original 2.8 by 2.0 kilometer qualification slice resumes.

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

Repository foundation and the prototype-first delivery model are established.
The Rust compiler now turns the locked OSM and Overture vectors into a
deterministic, inspectable hero world. The current artifact contains 2,820
objects across 72 spatial partitions, including measured Hoover Tower geometry
and OSM geometry for Memorial Church. The deterministic renderer now publishes
a complete 7,623 by 3,325 lossless WebP DZI candidate that has been exercised
in the responsive viewer. NAIP and LiDAR evidence, style approval, fixed-device
qualification, and release publication remain unfinished. The prototype is not
qualified, and no map release has been published.

## Development

Prerequisites are Rust 1.94.0, Python 3.12, Node.js 24, mdBook 0.5.4, and
`cargo-deny`. Install Python and web development dependencies once:

```sh
python3.12 -m venv perception/.venv
perception/.venv/bin/python -m pip install -r perception/requirements-dev.lock pip-audit==2.10.1
npm --prefix web install
```

Run the complete local acceptance gate:

```sh
scripts/check.sh
```

Generate the original synthetic regression preview:

```sh
cargo run --locked -- render fixture artifacts/reference.ppm
```

Synchronize the pinned prototype source bundle, an approximately 450 MB
transfer, into the ignored content-addressed cache:

```sh
cargo run --locked -- source sync
```

Compile and inspect the current vector-only hero world:

```sh
cargo run --release --locked -- world compile
cargo run --locked -- world inspect
```

Compilation validates the complete source lock, verifies the two consumed
vector artifacts, and writes ignored artifacts under `artifacts/world/`. The
committed `world.manifest.json` freezes
the expected vector-world hash and reports 387,096 ppm unknown coverage. That
large unknown fraction is deliberate evidence that NAIP and LiDAR compilation
must happen before qualification.

Render the vector-only Stanford preview:

```sh
cargo run --release --locked -- render region artifacts/render/hero.ppm
```

The current 1,950 by 873 preview contains real campus footprints, paths,
roads, empty parking, ground surfaces, flat roofs, directional facades,
faceted tree groves, hard shadows, crisp outlines, and world-anchored material
patterns. Parameterized procedural grammar now gives Hoover Tower a stepped
crown, Memorial Church a gabled roof and facade, and the Main Quad low arcade
walls with repeated openings. It is deterministic and recognizably Stanford,
but it is not yet an approved style candidate.

Publish and validate a local viewer pyramid after compiling the world:

```sh
cargo run --release --locked -- publish dzi artifacts/dzi/hero
cargo run --release --locked -- validate release artifacts/dzi/hero
```

Publication is atomic and fails if the destination already exists. The
candidate retains canonical indexed tiles for exact validation and serves only
the lossless WebP pyramid to OpenSeadragon.

Other remaining CLI command names are reserved and fail closed until their
tracked implementation tasks merge. See the
[engineering guide](https://dbuddha.github.io/isometric-stanford/) and
[ARCHITECTURE.md](ARCHITECTURE.md) for the implemented boundary.

## Licensing

Rust, Python, TypeScript, and other source code are available under either the
MIT License or Apache License 2.0, at your option. Original project artwork and
documentation are licensed under CC BY 4.0. Third-party data and assets retain
their original licenses and must be recorded before use. See
[ATTRIBUTION.md](ATTRIBUTION.md) for the governing policy.

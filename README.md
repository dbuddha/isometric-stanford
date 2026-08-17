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
The Rust compiler now fuses locked OSM, Overture, NAIP, and USGS LiDAR into a
deterministic, inspectable hero world. The current artifact contains 2,820
objects across 72 spatial partitions, including measured Hoover Tower geometry
and OSM geometry for Memorial Church. Frozen model-free perception reduces
unknown coverage from 387,096 to 5,202 ppm while retaining no source pixels or
transient semantic classes. The deterministic renderer publishes a complete
7,623 by 3,325 lossless WebP DZI candidate with an explicit style identity.
Candidate C has been exercised end to end in the responsive viewer. Style
approval, fixed-device qualification, and release publication remain
unfinished. The prototype is not qualified, and no map release has been
published.

Candidate A can now be generated as a complete visual-review pack. It proves
the deterministic review boundary, but its recorded evidence shows that the
art remains materially sparser and more diagrammatic than the intended
Isometric NYC analogue. Its engineering evidence remains preserved, but it was
rejected as the final style and is not approved.

Candidate B addresses the measured first-pass deficiencies with deterministic
ordinary windows, doors, convex hip roofs, a broader original material palette,
denser varied canopy, and distinct parking grammar. It raises fixed-scene edge
density by 15 to 50 percent while preserving Candidate A byte for byte. It is a
preserved, rejected review candidate, not an approved or published style.

Candidate C is the final bounded procedural iteration. It adds world-anchored
roof-tile cadence to simple and complex roofs, object-stable facade variation,
clearer landmark openings, and restrained semantic circulation treatment. It
preserves Candidates A and B byte for byte. Its engineering evidence is
implemented, while the final visual decision remains pending.

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

Synchronize the pinned prototype source bundle, an approximately 440 MB network
transfer plus one committed 7.2 MB licensed NAIP fixture, into the ignored
content-addressed cache:

```sh
cargo run --locked -- source sync
```

Source synchronization retries only bounded transient network failures. It
uses fixed connection, response-header, and response-body deadlines, starts
each retry from a new partial file, verifies the locked length and SHA-256, and
reports stable source IDs plus attempt counts without exposing acquisition
URLs. Permanent HTTP responses and integrity failures fail immediately.

Recompile the frozen semantic evidence from the exact approximately 450 MB
source cache when source or compiler verification is required:

```sh
cargo run --release --locked -- perceive run artifacts/perception
```

Ordinary development and CI consume the small locked evidence artifact and do
not download or rerun raw perception. Compile and inspect the fused hero world:

```sh
cargo run --release --locked -- world compile
cargo run --locked -- world inspect
```

Compilation validates the complete source and perception locks, verifies the
two vector artifacts plus the frozen NAIP and LiDAR evidence, and writes
ignored artifacts under `artifacts/world/`. The committed
`world.manifest.json` freezes every source and artifact hash and reports 5,202
ppm unknown coverage. Five cells with dominant unmapped building evidence stay
explicitly unknown instead of receiving invented geometry.

Render the fused Stanford preview:

```sh
cargo run --release --locked -- render region artifacts/render/hero.ppm
```

The current 1,954 by 880 preview contains real campus footprints, paths,
roads, empty parking, spectrally supported ground, LiDAR-backed canopy, flat
roofs, directional facades, faceted tree groves, hard shadows, crisp outlines,
and world-anchored material
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

Publish the unapproved Candidate C style for local visual inspection by naming
it explicitly:

```sh
cargo run --release --locked -- publish dzi artifacts/dzi/candidate-c candidate-c
cargo run --release --locked -- validate release artifacts/dzi/candidate-c
```

The generated `release.json` records `stanford_v1.candidate_c.1`, and validation
rejects the pyramid if a different known style is supplied. This preview path
does not change `style.lock.json` or imply final style approval.

Build an inspectable, explicitly unqualified browser bundle from a fresh
Candidate C artifact:

```sh
VITE_DZI_URL=/isometric-stanford/art/hero.dzi \
VITE_RELEASE_URL=/isometric-stanford/art/release.json \
  npm --prefix web run build
python3 scripts/assemble_preview.py \
  --viewer-dist web/dist \
  --dzi-artifact artifacts/dzi/candidate-c \
  --output artifacts/preview-site
cd web
npm exec vite -- preview --outDir ../artifacts/preview-site
```

The assembler verifies every DZI byte against `release.json`, requires the
current committed world hash, rejects pre-staged artwork in `web/dist`, and
writes `preview.json` with `published_release: false`. Output and staging paths
must not already exist. The dry-run workflow produces the same portable bundle
without deploying it.

The exact Candidate C artifact also enables four review controls: Whole campus,
Hoover Tower, Memorial Church, and Main Quad. Each control writes a stable
`#view=` fragment, participates in browser history, and restores a fixed crop.
Landmark controls fail closed unless the release metadata matches the pinned
world hash, style hash, and 7,623 by 3,325 dimensions, so review URLs cannot
silently point at the wrong artwork.

Generate the four-scene Candidate A review pack:

```sh
cargo run --release --locked -- style candidate-a artifacts/style/candidate-a
cargo run --release --locked -- style candidate-b artifacts/style/candidate-b
cargo run --release --locked -- style candidate-c artifacts/style/candidate-c
```

The ignored output includes native WebP scenes, landmark masks, a contact
sheet, indexed metrics, and known deviations. Hosted CI regenerates and uploads
the same evidence. Candidate A is evidence for review, not a released style.
Candidates B and C write the same stable scene set so all three procedural
iterations can be compared without crop drift.

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

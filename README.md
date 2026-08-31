# Isometric Stanford

Isometric Stanford is an evidence-driven project to build an original,
deterministic isometric artwork and web map of Stanford campus. Registered
orthographic Google 3D reference layers and reviewed masks feed a safe-Rust
stylizer that produces crisp, late-1990s
city-builder-style pixel art. A static OpenSeadragon viewer serves the accepted
DZI/WebP pyramid efficiently on desktop and mobile.

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

- Final artwork is a deterministic palette transform of registered source
  layers and accepted masks. Raw reference tiles and unmodified photographic
  regions are never publication artifacts.
- People, cars, buses, cranes, and temporary equipment are excluded from the
  final world and renderer assets.
- Google Photorealistic 3D Tiles are the sole geographic source for the active
  masking and stylization pipeline.
- Open-source libraries, pretrained CV weights, and original non-geographic art
  assets may process registered Google layers.
- Qwen does not produce final artwork.
- A deterministic fixed-point CPU renderer is the v1 rendering boundary.
- OpenSeadragon and static DZI/WebP are the v1 browser boundary.
- Style approval and release publication remain human-owned decisions.

## Status

Repository foundation and the prototype-first delivery model are established.
The historical Rust compiler fuses locked OSM, Overture, NAIP, and USGS LiDAR into a
deterministic, inspectable hero world. The current artifact contains 2,820
objects across 72 spatial partitions, including measured Hoover Tower geometry
and OSM geometry for Memorial Church. Frozen model-free perception reduces
unknown coverage from 387,096 to 5,202 ppm while retaining no source pixels or
transient semantic classes. The deterministic renderer publishes a complete
7,623 by 3,325 lossless WebP DZI candidate with an explicit style identity.
Candidate C has been exercised end to end in the responsive viewer. Style
approval, fixed-device qualification, and release publication remain
unfinished. The prototype is not qualified, and no map release has been
published. That procedural world is retained as rejected comparison evidence
and cannot enter the active Google-only production lineage.

The registered reference contract and browser capture runtime are now
implemented. The pinned Three.js path holds one orthographic camera across six
Google 3D Tiles passes, waits for a stable complete visible set, transfers raw
layers through a tokenized loopback endpoint, and atomically promotes only a
bundle accepted by the Rust validator. Ordinary CI proves this path with a
synthetic 3D scene and never requests Google content. A private bounded Hoover
probe has now produced three Rust-validated registered bundles and a working
layer-review report. A controlled maximum-detail experiment now holds that
camera and 320-meter footprint fixed while varying Google screen-space error
and output sampling. It shows that SSE 8, rather than the renderer plugin's SSE
20 recommendation, reaches Google's available Stanford source LOD. The
maximum-detail review master uses 125 millimeters per output pixel at SSE 8.
SSE 4 adds no requests, geometry, or pixels. This is capture evidence only,
not accepted art or a public Google artifact. The safe-Rust mask artifact
contract is also implemented.

A second bounded experiment now derives fixed-grid neighbors, keeps one camera
world matrix fixed, shifts only orthographic frusta, and compares two
independent Hoover cores with a larger monolithic view. The independent source
seam passes its color, coverage, depth, and normal gates and is visually clean
at 1:1. Captured lighting and the separately traversed monolithic source oracle
remain diagnostic failures. The active requirement now compiles frozen
registered captures into one exact canonical atlas. The local
overlap workbench exposes the scoped result instead of presenting it as a full
pass.
It registers every mask to an exact reference manifest, preserves transient
classes only in evidence and repair inputs, streams validation without loading
the raster, and makes transient or source-artifact pixels invalid in persistent
output.

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

Candidate C is the final bounded procedural-only iteration. It adds world-anchored
roof-tile cadence to simple and complex roofs, object-stable facade variation,
clearer landmark openings, and restrained semantic circulation treatment. It
preserves Candidates A and B byte for byte. Its engineering evidence is
implemented and preserved as the reference-driven pilot baseline. The active
pilot now qualifies registered multipass capture, masks, obstruction repair,
Rust stylization, and guarded stitching before any campus expansion.

## Development

Prerequisites are Rust 1.94.0, Python 3.12, Node.js 24, mdBook 0.5.4, and
`cargo-deny`. Install Python and web development dependencies once:

```sh
python3.12 -m venv perception/.venv
perception/.venv/bin/python -m pip install -r perception/requirements-dev.lock pip-audit==2.10.1
npm --prefix web install
npm --prefix capture install
```

Run the complete local acceptance gate:

```sh
scripts/check.sh
```

Validate a captured multipass reference bundle before masks or stylization can
consume it:

```sh
cargo run --locked -- reference inspect artifacts/reference/hoover
```

The inspector requires registered color, whitebox, linear-depth, view-normal,
fixed-shadow, and coverage layers with one camera, complete hashes, and at
least 99.5 percent valid core coverage.

Compile frozen neighboring bundles into the canonical atlas consumed by masks
and stylization:

```sh
cargo run --locked -- reference atlas-compile \
  artifacts/reference-atlas/request.json \
  artifacts/reference-atlas/hoover
cargo run --locked -- reference atlas-inspect artifacts/reference-atlas/hoover
```

The compiler streams each registered layer, assigns every saved pixel to one
source bundle by a stable overlap priority, writes an ownership tile for every
atlas cell, and atomically publishes only a complete validated atlas. Repeating
the build from the same frozen bundles produces the same manifest and tile
hashes.

Inspect a frozen semantic mask without loading the complete raster into memory:

```sh
cargo run --locked -- mask inspect artifacts/masks/hoover
```

The mask inspector verifies reference registration, the complete 24-class
ontology, confidence and evidence encoding, instance-class consistency,
summary counts, byte length, and SHA-256. A persistent artifact containing a
person, vehicle, construction object, or broken-source pixel fails closed.
Safe-Rust geometry kernels now provide deterministic depth and normal edges,
morphology, connected components, chamfer distance, watershed, and line
evidence. Finite-radius operations have guarded-crop equivalence tests, while
connectivity and watershed remain whole-supertile stages before cell slicing.

The configured Hoover capture command and its credential isolation, readiness,
and failure contracts are documented in [`capture/README.md`](capture/README.md).

Review any accepted local bundle without copying its source artifacts into the
web application or a deployable build:

```sh
REFERENCE_BUNDLE_DIRECTORY="$PWD/artifacts/reference/hoover" npm --prefix web run dev
```

Open `http://127.0.0.1:5173/isometric-stanford/review`. The workbench verifies
all six lengths, hashes, encodings, and registered dimensions before showing
anything. It decodes only the selected comparison pair, keeps pan and zoom in
shared source-pixel coordinates, supports a wipe view and 1:1 inspection, and
displays the immutable camera, source, attribution, coverage, and hash evidence.

Review a complete private overlap experiment and its hashed comparison images:

```sh
OVERLAP_EVIDENCE_DIRECTORY="$PWD/artifacts/google-overlap/<run-id>" \
  npm --prefix web run dev -- --host 127.0.0.1
```

Open `http://127.0.0.1:5173/isometric-stanford/review/overlap`. The workbench
shows joined versus monolithic cores, independent guards, mismatch heatmaps,
source and lighting gates, response formats, camera registration, grid error,
coverage, and process memory. It rejects a corrupt report or image before
displaying a viewport.

Compare the five frozen source-quality candidates on the same physical
footprint:

```sh
QUALITY_EVIDENCE_DIRECTORY="$PWD/artifacts/google-quality/hoover-quality-2026-08-30" \
  npm --prefix web run dev -- --host 127.0.0.1
```

Open `http://127.0.0.1:5173/isometric-stanford/review/quality`. The lab verifies
every image hash before exposing synchronized split, wipe, 1:1, Hoover, tree,
roof, and construction inspection. Its measurements separate source LOD from
output raster sampling and state the remaining source defects explicitly.

Generate the original synthetic regression preview:

```sh
cargo run --locked -- render fixture artifacts/reference.ppm
```

The following commands reproduce the rejected historical open-data comparison.
They are prohibited from feeding the active Google-only masking, stylization,
or publication path.

Synchronize that historical source bundle into the ignored content-addressed
cache:

```sh
cargo run --locked -- source sync
```

Source synchronization retries only bounded transient network failures. It
uses fixed connection and receive deadlines, continues a partial range only
against a locked entity tag and exact response bounds, verifies the locked
length and SHA-256, and reports stable source IDs plus attempt counts without
exposing acquisition URLs. Permanent HTTP responses and integrity failures fail
immediately.

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

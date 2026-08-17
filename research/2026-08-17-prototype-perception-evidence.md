# Prototype perception and transient-evidence record

- Date: 2026-08-17
- Parent task: P-010, issue #99
- Region: `stanford-hero-v1`
- Status: implemented engineering baseline, not final visual approval

## Decision

The hero prototype uses a deterministic, model-free Rust compiler before any
GPU model is admitted. The compiler combines four-band NAIP spectral evidence
with streamed USGS LiDAR classifications. This is enough to resolve the
prototype's open ground and canopy cells without generating final pixels,
retaining source imagery, or adding a nondeterministic model dependency.

This does not claim that heuristic perception is the best full-estate
classifier. A learned model remains contingent on a benchmark showing that the
model-free compiler misses accepted semantic metrics. H100 execution is not
useful for the current 600 meter prototype because compilation is already a
38-second CPU operation and rendering never consumes a model.

## Permanent and transient boundary

The vector compiler first identifies the 372 of 961 review cells that lack an
accepted persistent vector class. The remaining 589 cells are masked before
raster evidence can change their semantics. This excludes all mapped roads,
paths, parking, buildings, fields, water, and vegetation from material
inference, including any cars or people captured on those surfaces.

Within eligible cells, the compiler uses classed ground, water, building, and
medium or high vegetation LiDAR returns. It never uses unclassified returns as
persistent-class evidence. Unclassified returns between 0.5 and 4 meters above
the cell ground minimum are counted as conservative transient candidates and
discarded. The counter is intentionally overinclusive and is not represented
as vehicle geometry. Construction-tagged OSM features remain excluded. The
canonical world enum and style pack still cannot represent a person, vehicle,
bus, crane, or temporary-equipment class.

## Classification contract

- NAIP is decoded as the exact locked 1,326 by 1,168 four-band U8 GeoTIFF.
- Positive vegetation evidence uses NDVI of at least 0.10.
- Canopy requires at least 20 ASPRS class 4 or 5 returns and at least 8 percent
  of all in-cell LiDAR returns.
- A cell with at least 70 percent building-class returns remains explicit
  `unknown` instead of inventing an unmeasured footprint.
- Water requires dominant classed LiDAR water or dominant dark low-NIR NAIP
  evidence.
- Remaining cells become grass or dry-grass terrain using spectral consensus.
- Every threshold is integer or basis-point based in the frozen artifact.
- Every output cell retains sample counts, confidence, source IDs, and the
  discarded transient-candidate count.

The 20 meter output is semantic support, not a survey of individual tree crown
or facade geometry. LiDAR canopy height is bounded to 3 through 30 meters.

## Measured result

The exact artifact at `fixtures/perception/hero-evidence.json` has SHA-256
`3b6d79edebd33829a9c82f0bfdf16675eb1d6c73677db0dd9b391443850f682d`.
It records:

| Measure | Result |
| --- | ---: |
| Vector-masked cells | 589 |
| Evidence cells | 372 |
| Accepted NAIP samples | 1,065,077 |
| In-bounds streamed LiDAR points | 8,506,505 |
| Conservative discarded transient candidates | 359,349 |
| Terrain cells | 60 |
| Canopy cells | 307 |
| Explicit unknown cells | 5 |

The fused world remains 2,820 objects in 72 partitions because one stable
semantic object replaces each former unknown cell. Unknown coverage falls from
387,096 to 5,202 ppm. All seven locked source hashes and the perception
artifact hash enter `world.manifest.json`.

A release build completed the full source recompile in 38.02 seconds on the
10-logical-core arm64 development machine. `/usr/bin/time -l` measured
36,438,016 bytes maximum resident set size. The reusable LAZ chunk contains at
most 250,000 points, so working memory does not grow with total point count.

## Dependency review

The implementation adds only safe-Rust, pinned source-decoding dependencies:

| Dependency | Version | Purpose | License |
| --- | ---: | --- | --- |
| `las` | 0.11.0 | Stream LAS and LAZ records | MIT |
| `tiff` | 0.11.3 | Decode the locked GeoTIFF | MIT |
| `proj4rs` | 0.1.10 | Pure-Rust EPSG:2227 to EPSG:26910 transform | MIT OR Apache-2.0 |

`las` uses its serial `laz` feature rather than parallel decompression so the
canonical path has a fixed bounded chunk and no scheduling-dependent output.
`tiff` enables only the compression families required for safe source decoding.
`proj4rs` disables default binaries and optional CRS databases; the two audited
projection strings are source constants and a control point must match PROJ to
within two millimeters. `cargo deny check` remains authoritative for transitive
licenses, advisories, bans, and duplicate policy.

Primary implementation references:

- <https://github.com/gadomski/las-rs>
- <https://github.com/image-rs/image-tiff>
- <https://github.com/3liz/proj4rs>

## Reproduction and gates

```sh
cargo run --release --locked -- source sync
cargo run --release --locked -- perceive run artifacts/perception
cargo run --release --locked -- world compile artifacts/world
```

Ordinary CI validates the committed evidence, exact source and artifact hash
chain, semantic-world golden, fail-closed incomplete-evidence case, no-transient
contract, seam oracle, render goldens, and browser pyramid. Weekly scheduled
assurance downloads every locked source, recompiles evidence twice, requires
byte equality with the committed artifact, records GNU time output, and uploads
the evidence without publishing a release.

# Reference repair experiment

The first deterministic art experiment operates on one frozen maximum-detail
Hoover bundle. It makes no Google request and never commits Google-derived
pixels. The bundle uses the qualified 330 degree azimuth, 42 degree elevation,
SSE 8, and 125 millimeters per source pixel. The Rust output grid is 250
millimeters per logical pixel.

## Controlled candidates

| Candidate | Inputs and treatment | Purpose |
| --- | --- | --- |
| A | Color, integer edge-aware smoothing, fixed palette | Measure the ceiling of an RGB-only filter |
| B | Color, depth, normals, fixed relighting, structural outlines, fixed palette | Measure the value of registered geometry guidance |
| C | Candidate A architecture, high-confidence canopy mask, seven-band canopy replacement, structural outlines | Test a narrow semantic repair without inventing architecture |

Passenger cars remain because they are persistent accepted reference details.
People, bicycles, buses, trucks, cranes, temporary equipment, and source
artifacts remain outside persistent output.

## Measured Hoover result

| Measure | A | B | C |
| --- | ---: | ---: | ---: |
| Colors | 33 | 47 | 40 |
| Structural-edge recall | 83.33% | 92.38% | 97.30% |
| Canopy interior edge density | 164,645 ppm | 192,837 ppm | 104,147 ppm |
| Non-structural edge density | 188,935 ppm | 207,199 ppm | 169,685 ppm |

Candidate C reduces accepted canopy interior edge density by 36.7 percent
relative to Candidate A. The complete transform takes about 1.2 seconds on the
measured Apple arm64 host. Complete-process RSS was about 94 MiB and the
canonical live-buffer estimate was 66.0 MiB. Three independent runs produced
the same six PNG hashes and the same report SHA-256,
`579b664690c36994a529c183689271fcc3131a767a7b255662f5794a918b1ff0`.

## Review boundary

The local `/review/repair` route verifies the report and every image hash before
display. It provides source, A, B, C, mask, and structural-edge choices;
synchronized fit, 1:1, zoom, and crop controls; split and correctly oriented
wipe comparisons; measured metrics; and explicit blockers. Desktop and mobile
tests prevent evidence labels from overlapping and reject modified image
bytes.

This experiment does not qualify expansion. Candidate C is a meaningful
deterministic baseline, not an Isometric NYC-quality final result. It repairs
only the narrow canopy class. Construction has no accepted instance mask, and
roof planes, windows, facade cadence, paths, and markings still need reviewed
semantic evidence and material-specific grammar.

## Reproduction

```sh
cargo run --release --locked -- reference repair-study \
  artifacts/google-quality/hoover-quality-2026-08-30/bundles/sample-sse8-125mm \
  artifacts/reference-repair/<new-run-id>
cargo run --locked -- reference repair-inspect \
  artifacts/reference-repair/<new-run-id>
```

The output directory must not already exist. Every successful run is promoted
atomically and contains six allowlisted PNGs plus `repair-review.json`.

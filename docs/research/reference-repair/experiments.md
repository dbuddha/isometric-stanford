# Experiments

## Protocol

All candidates consume the same private validated Hoover core at SSE 8 and 125
millimeters per source pixel. Rust crops the registered guard, reduces color,
normal, and depth to a 250 millimeter logical grid, then runs three controlled
paths:

1. Candidate A uses integer edge-aware smoothing and a 33-color observed
   subset of the fixed base palette.
2. Candidate B adds depth and normal guidance, fixed relighting, structural
   outlines, and a geometry palette.
3. Candidate C retains Candidate A's architectural treatment, replaces only
   conservative canopy pixels with seven controlled bands, and adds accepted
   structural outlines outside canopy interiors.

The run writes six allowlisted RGBA8 PNGs and one canonical JSON report through
an atomic staging directory. The report hashes every image and records source
identity, algorithm identity, metrics, buffer estimate, passenger-car policy,
and blockers.

## Environment

| Dimension | Identity |
| --- | --- |
| Repository base | `b18f470` |
| Host | Apple arm64, 24 GiB RAM |
| Rust | 1.94.0 |
| Algorithm | `reference-repair-rust/v1` |
| Input | Private `sample-sse8-125mm` registered Google bundle |
| Camera | Orthographic, 330 degree azimuth, 42 degree elevation |
| Source / logical scale | 125 / 250 millimeters per pixel |

## E-001: controlled candidate comparison

| Metric | Candidate A | Candidate B | Candidate C |
| --- | ---: | ---: | ---: |
| Colors | 33 | 47 | 40 |
| Structural-edge recall | 83.33% | 92.38% | 97.30% |
| Canopy interior edge density | 164,645 ppm | 192,837 ppm | 104,147 ppm |
| Non-structural edge density | 188,935 ppm | 207,199 ppm | 169,685 ppm |
| Mean luminance | 73.312006 | 72.872560 | 77.519114 |

Disposition: retain Candidate C as the deterministic repair baseline. Reject
all three as final style qualification. Candidate C materially improves canopy
and structural continuity, while construction and semantic architectural
repair remain visibly absent.

## E-002: exact repeatability

Three independent output directories produced these identical hashes:

| Artifact | SHA-256 |
| --- | --- |
| Candidate A | `af4a8fe02bcbb7989799e145b34494969958fa9a953427c18f070b9348608708` |
| Candidate B | `4419e9f806aa389b895efe4eaec49f70e6fbc6588ac4e75aee07dcb18df2d016` |
| Candidate C | `ddd4f4d65581d4577e996eb87f5122b217323ffb3e41f9d87fe503ba8d4bad92` |
| Canopy mask | `2a382ada0e68cdf2d5ac745d1903c4ee27b2369587312e6234a9e0aed020355c` |
| Structural edges | `ad3efd0b0fbe4393d44d23500b4a5eaa35b5d4b06a1b3a885e8bed18912c641f` |
| Report | `579b664690c36994a529c183689271fcc3131a767a7b255662f5794a918b1ff0` |

Disposition: post-capture determinism passes for the v1 algorithm.

## E-003: performance and browser review

The measured transform completed in about 1.2 seconds with 98,516,992 bytes
maximum RSS. The canonical live-buffer estimate is 69,216,256 bytes. The review
route loaded the real private report at desktop and iPhone 14 viewports. It
exposed every candidate, crop, metric, and blocker with no browser page errors
and no automated WCAG A/AA violations. Browser dogfooding found and repaired
reversed wipe clipping and mobile label overlap. Playwright now checks clip
orientation, slider behavior, evidence-label geometry, controls, corrupt image
rejection, desktop, and mobile.

Disposition: the evidence cockpit passes after remediation. Axe could not
automatically resolve every contrast case where text overlaps imagery, so
visual contrast remains a manual check.

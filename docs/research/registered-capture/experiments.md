# Experiments

## Protocol

The test area centers Hoover Tower at 37.4276111, -122.1670000. One root
session renders a 2,048 by 1,024 monolithic core and two adjacent 1,024 by
1,024 cores. Every capture has a 128-pixel guard. The source scale is 250
millimeters per pixel. Left and right geographic centers are derived from the
same local metric grid. The test checks 2,097,152 saved pixel centers and then
compares six exact raw layers in safe Rust.

The command requires a credential in the child process environment. The
credential is retrieved from macOS Keychain and is never printed or retained:

```bash
npm --prefix capture run build
GOOGLE_MAP_TILES_API_KEY="$(security find-generic-password -a "$USER" -s isometric-stanford-map-tiles -w)" \
  npm --prefix capture run overlap -- \
  --spec "$PWD/capture/specs/hoover-overlap-probe.json" \
  --output "$PWD/artifacts/google-overlap/<new-run-id>"
```

Ordinary CI never executes this command. Frozen private raw layers can be
reanalyzed without Google access:

```bash
cargo run --quiet --locked -- reference compare-overlap \
  artifacts/google-overlap/<run-id>/comparison-request.json
```

## Environment

| Dimension | Identity |
| --- | --- |
| Repository base | `9bcd5cfd8919e104838c96a5083083ca25914827` |
| Host | Apple arm64, 24 GiB RAM |
| OS | macOS 26.6.2, Darwin 25.6.0 |
| Node | 24.19.0 |
| Rust | 1.94.0 |
| Three.js | 0.185.1 |
| `3d-tiles-renderer` | 0.5.0, git head `10e9dc969ba5fdd27a83fd47149a2b8eae841741` |
| Chromium headless shell | 151.0.7922.34, Playwright revision 1234 |
| Graphics | ANGLE over Metal |
| Readiness | root loaded, no active load, stable signature for 45 frames and 1.5 seconds, at least four visible tiles |

## E-001: camera-recentered neighbors

Method: build each neighbor's camera world matrix from its own geographic
center. Keep scale, nominal orientation, source session, dimensions, and grid
constant.

| Metric | Result |
| --- | ---: |
| Root sessions | 1 |
| Attempted / completed / failed | 368 / 296 / 72 |
| Monolithic / left / right coverage | 99.99% / 99.99% / 99.99% |
| Maximum grid-center error | 0.023233 px |
| Best bounded registration offset | 0, 0 px |
| Independent depth above tolerance | 147,698 ppm |
| Independent normal above tolerance | 171,539 ppm |
| Independent whitebox above tolerance | 173,007 ppm |
| Complete-tree peak RSS | 953,810,944 bytes |

Disposition: reject camera recentering. The zero-offset search excludes a
simple subpixel translation as the main cause. Different screen-space LOD and
the 96 MiB hard cache are the likely confounds.

## E-002: fixed camera with off-axis frusta

Method: construct the camera once at the Hoover anchor. Reuse the exact world
matrix. Shift only each orthographic projection window according to its target
offset in camera-right and camera-up axes. Raise cache policy to 128 MiB
retention and 256 MiB ceiling.

| Metric | Monolithic | Left | Right |
| --- | ---: | ---: | ---: |
| Visible tiles | 121 | 83 | 66 |
| Renderer cache | 226,785,670 B | 173,509,082 B | 158,692,473 B |
| Readiness time | 3.132 s | 1.518 s | 1.632 s |
| Core coverage | 99.99% | 99.99% | 99.99% |

Session totals:

- One root event, 428 attempted and completed requests, zero failed or blocked.
- 395 GLB and 33 JSON responses.
- Node peak 85,606,400 bytes and ingest-worker peak 98,533,376 bytes.
- Chromium peak 1,073,037,312 bytes.
- Complete-tree peak 1,254,883,328 bytes within a 1,342,177,280-byte envelope.
- The observed 27,988 response bytes are only a `Content-Length` lower bound.

## E-003: scoped reanalysis

The v3 analyzer separates independent source seam, monolithic source oracle,
and captured-lighting gates.

| Layer | Independent full overlap ppm | 64-pixel saved seam ppm | Joined seam versus monolithic ppm | Gate threshold ppm |
| --- | ---: | ---: | ---: | ---: |
| Color | 21 | 30 | 564 | 5,000 |
| Coverage | 0 | 0 | 0 | 0 |
| Linear depth | 30 | 61 | 854 | 100 |
| View normal | 146 | 91 | 2,822 | 100 |
| Whitebox | 75,354 | 100,143 | 93,811 | 250 |
| Fixed shadow | 50,817 | 62,057 | 60,028 | 1,000 |

Scoped verdict:

- Independent source seam: pass.
- Monolithic source oracle: fail.
- Captured lighting seam: fail.
- All relationships: fail.
- Failure classes: `monolithic-oracle-coverage`,
  `monolithic-oracle-level-of-detail`, and `shadow-phase`.
- Boundary corridor includes 5,944 structural depth edges, so the pass is not
  an empty or featureless seam.

The dashboard was exercised with the exact private report and seven hashed
derived images. It loaded the source pass and remaining failures, supported
fit and 1:1 core wipes, switched to the guard overlap and heatmap, and produced
no browser console error.

## E-004: maximum-detail source qualification

Method: hold the selected camera and 320 by 320 meter guarded footprint fixed.
Run five candidates in one live Google root session. Vary source SSE across 20,
8, and 4. Vary output sampling across 250 and 125 millimeters per pixel. Disable
texture mipmaps, allow 512 MiB minimum and 2 GiB maximum tile cache, and stop at
1,000 total requests. Freeze every candidate PNG and per-candidate cumulative
request count.

| Candidate | Scale | SSE | Added requests | Visible tiles | Triangles | Max depth | Cache | Elapsed |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Baseline | 250 mm/px | 20 | 266 | 73 | 237,150 | 23 | 154,261,894 B | 6.135 s |
| Finer LOD | 250 mm/px | 8 | 518 | 224 | 1,370,554 | 25 | 408,005,115 B | 4.891 s |
| Aggressive LOD | 250 mm/px | 4 | 0 | 224 | 1,370,554 | 25 | 408,005,115 B | 2.002 s |
| Two-times sampling | 125 mm/px | 8 | 0 | 224 | 1,370,554 | 25 | 408,005,115 B | 2.001 s |
| Maximum bounded | 125 mm/px | 4 | 0 | 224 | 1,370,554 | 25 | 408,005,115 B | 2.001 s |

Session totals:

- One root event, 784 attempted and completed requests, zero failed or blocked.
- 496 GLB and 288 JSON responses.
- 99.99 percent core coverage for every candidate.
- SSE 8 and SSE 4 PNGs were byte identical at the matching sampling scale.
- Node peak 88,670,208 bytes and Chromium-family peak 1,595,899,904 bytes.
- Complete-tree peak 1,819,934,720 bytes within the corrected 2 GiB envelope.
- Raw report SHA-256 `a4fb75e2af5e8a2fbf0f3388ddb7b452f1ba40abc5743c41a03d00e6a49659b3`.
- Derived review SHA-256 `c0c79a87135d8ac2c57f9ff62106effa96a6205ba3c3b1776765b290675fb9ac`.

Disposition: select SSE 8. Use 125 millimeters per pixel for the
maximum-detail review master and 250 millimeters per pixel for smaller
processing inputs where native supersampling is unnecessary. Do not expect
SSE 4 or additional framebuffer pixels to repair faceted trees, construction,
thin-object loss, or missing photogrammetry.

The local dashboard was exercised at desktop and 390 by 844 mobile viewports.
It loaded the exact five frozen candidates, changed crop and comparison state,
showed no browser exception or failed request, and exposed a keyboard-focusable
scroll container for the wide evidence table.

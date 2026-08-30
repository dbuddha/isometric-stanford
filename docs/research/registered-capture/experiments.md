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

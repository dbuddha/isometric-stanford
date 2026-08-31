# Registered reference capture

This workspace captures the noncanonical Google reference inputs used by the
masking and Rust stylization pipeline. It does not generate final art.

One request fixes the geographic target, orthographic camera, pixel grid,
guard, lighting, readiness thresholds, and source epoch. The browser then
renders the following registered passes without moving the camera:

1. Textured color
2. Neutral whitebox
3. Linear camera depth in integer millimeters
4. View-space normals
5. Fixed project lighting and shadows
6. Source coverage

Provider attribution is displayed in the capture page and frozen into the
manifest alongside the renderer identity.

Camera and sun elevations are stored as positive angles above the horizon. The
Three.js frame uses the corresponding negative look direction so the camera
and light origin remain above the target rather than below the terrain.

Each pass is transferred as raw bytes to a random-token loopback server in a
credential-free ingest worker. The worker invokes the bounded safe-Rust PNG
encoder, emits depth files into a private staging directory, hashes them,
writes `reference.manifest.json`, and invokes the safe-Rust `reference inspect`
validator. Only a fully valid bundle is atomically renamed to its requested
destination. Existing bundles are immutable and never overwritten.

## Local validation

```bash
npm --prefix capture ci
npm --prefix capture run check
npm --prefix capture test
npm --prefix capture run build
npm --prefix capture run test:e2e
```

The browser test uses a synthetic Three.js scene. It exercises the real pass
renderer, upload protocol, encoders, registration checks, and atomic writer
without making an external request or requiring a credential.

## Configured Hoover capture

Build the Node entry point, install the pinned Chromium headless shell once,
and run the pinned request:

```bash
npm --prefix capture run build
npm --prefix capture exec -- playwright install chromium
GOOGLE_MAP_TILES_API_KEY='<local credential>' npm --prefix capture run capture -- \
  --spec specs/hoover-pilot.json \
  --output ../artifacts/reference/hoover
```

The bounded camera probe reuses one Google root session for three exact
orthographic orientations and produces a private HTML comparison report:

```bash
probe_key="$(security find-generic-password -a "$USER" -s isometric-stanford-map-tiles -w)"
GOOGLE_MAP_TILES_API_KEY="$probe_key" npm --prefix capture run probe -- \
  --spec specs/hoover-camera-probe.json \
  --output ../artifacts/google-probe/hoover
unset probe_key
```

The one-camera pilot supertile uses a 2,048 by 2,048 core, a 256-pixel guard,
and the same 250 millimeter source scale:

```bash
probe_key="$(security find-generic-password -a "$USER" -s isometric-stanford-map-tiles -w)"
GOOGLE_MAP_TILES_API_KEY="$probe_key" npm --prefix capture run probe -- \
  --spec specs/hoover-pilot-supertile.json \
  --output ../artifacts/google-probe/hoover-pilot-supertile
unset probe_key
```

The independent-overlap experiment derives two geographic neighbors from one
metric grid, holds the camera world matrix fixed, and changes only each
orthographic projection window. It uses one root session for one 2,048 by
1,024 monolithic core and two 1,024 by 1,024 adjacent cores:

```bash
overlap_key="$(security find-generic-password -a "$USER" -s isometric-stanford-map-tiles -w)"
GOOGLE_MAP_TILES_API_KEY="$overlap_key" npm --prefix capture run overlap -- \
  --spec specs/hoover-overlap-probe.json \
  --output ../artifacts/google-overlap/<new-run-id>
unset overlap_key
```

The command fails if the new output already exists. It stores private exact raw
layers, validated PNG bundles, one combined report, joined source previews, and
mismatch heatmaps. The safe-Rust comparator can rerun against the raw archive
without another Google request. The current fixed-camera result passes the
independent color, coverage, depth, and normal seam gate. It does not pass the
monolithic-oracle or captured-lighting gates, so it is evidence rather than a
campus-collection approval.

The maximum-detail probe keeps the selected camera and 320-meter guarded
footprint fixed while changing source LOD and output sampling. It requires one
new immutable output directory:

```bash
quality_key="$(security find-generic-password -a "$USER" -s isometric-stanford-map-tiles -w)"
GOOGLE_MAP_TILES_API_KEY="$quality_key" npm --prefix capture run probe -- \
  --spec specs/hoover-quality-probe.json \
  --output ../artifacts/google-quality/<new-run-id>
unset quality_key
npm --prefix capture run quality-review -- \
  ../artifacts/google-quality/<new-run-id>
```

The 2026-08-30 run used one root session and completed 784 requests with no
failures or blocked requests. SSE 20 selected 73 visible tiles and 237,150
triangles. SSE 8 selected 224 tiles and 1,370,554 triangles. SSE 4 then added
zero requests and produced the exact same PNG bytes as SSE 8 at each sampling
scale. Doubling the output grid from 250 to 125 millimeters per pixel added
image samples but no source geometry. Use SSE 8 and 125 millimeters per pixel
for maximum-detail inspection. Use SSE 8 and 250 millimeters per pixel when a
smaller reference is sufficient.

The canonical Hoover atlas capture uses that accepted SSE 8 and 125 millimeter
profile for four 2,048-pixel cores. It creates the Google root tileset once,
holds one camera world matrix, shifts four off-axis projection windows, and
compiles all promoted bundles into one 4,096 by 4,096 ReferenceAtlas:

```bash
atlas_key="$(security find-generic-password -a "$USER" -s isometric-stanford-map-tiles -w)"
GOOGLE_MAP_TILES_API_KEY="$atlas_key" npm --prefix capture run atlas-capture -- \
  --spec specs/hoover-atlas-capture.json \
  --output ../artifacts/google-atlas/<new-run-id>
unset atlas_key
```

The private output contains four validated registered bundles,
`atlas-request.json`, the canonical tiled `atlas/` directory, and
`atlas-capture-report.json`. The report contains only a SHA-256 digest of the
root response, a non-secret session identity, fixed-camera and projection
proofs, coverage, request counts, timing, and process-tree memory. It stores no
key, session URL, response body, or request URL. The command rejects a second
root request, a missing root digest, camera movement, projection drift, coverage
below 99.5 percent, output reuse, or a process tree above 2 GiB.

The atlas profile has a hard ceiling of 4,000 attempted Google tile requests.
The first live 2 by 2 attempt exhausted the former 1,000-request ceiling after
987 successful responses and retained no output. The higher ceiling does not
change SSE, sampling, camera, or coverage requirements. It remains far below
Google's documented 12,000 renderer requests per minute service limit, and the
browser still blocks the next request before exceeding the configured ceiling.

Billing and root quota are separate. A root tileset request starts the reusable
session and counts toward the 10,000 per-day root quota. Every successful
returned 3D tile is a billable event. The current public price sheet lists a
1,000-event monthly free usage cap and USD 6 per 1,000 events after that tier.
The 4,000-attempt ceiling therefore represents at most USD 24 when no free tier
remains. Account-wide usage and negotiated pricing may differ.

The 400-request camera ceiling was measured rather than guessed. A 100-request run
loaded 4.12 MiB successfully and then blocked 26 required child requests. The
accepted Hoover run used 282 tile requests and one root tileset request. The probe
stores no request URLs, keys, or session values.

Live capture launches the browser directly and retains no Playwright protocol
session. A token-authenticated loopback coordinator transfers the key into page
memory after navigation. The key never enters the browser command line, child
environment, artifacts, URLs, or diagnostics. A browser-side request budget is
installed before the Google scene is created.

The measured 1,280-pixel worker envelope is 1 GiB, with a 79 MiB Node peak and
849 MiB complete-tree peak. The measured 2,560-pixel envelope is 1.25 GiB, with
a 79 MiB Node peak and 1,014 MiB complete-tree peak. The versioned memory policy
reserves host memory and caps parallel acquisition at four workers.

The high-LOD quality run retained 389.1 MiB of renderer data and reached a
1,819,934,720-byte complete-tree peak. Its spec therefore pins a 2 GiB worker
envelope. Future reports calculate scheduling from the largest candidate grid
and this measured minimum instead of the baseline candidate alone.

The larger fixed-camera overlap run raised the renderer cache from the old 96
MiB ceiling to a 128 MiB retention target and 256 MiB ceiling. It completed 428
of 428 responses, including 395 GLB and 33 JSON, and reached a 1,254,883,328
byte complete-tree peak inside the 1.25 GiB envelope. The direct browser's
`responseBodyBytes` sums only visible `Content-Length` headers and must be
treated as a lower bound.

After Rust validation accepts the immutable bundle, inspect its six registered
layers through the local-only review route:

```bash
REFERENCE_BUNDLE_DIRECTORY="$PWD/artifacts/reference/hoover" npm --prefix web run dev
```

Then open `http://127.0.0.1:5173/isometric-stanford/review`. The development
server exposes only the seven allowlisted bundle files, with no caching, from
the configured directory. It does not copy them into `web/public`, `dist`, or
any release artifact.

Use `OVERLAP_EVIDENCE_DIRECTORY` and the `/review/overlap` route for an
overlap experiment. That route verifies the report and seven comparison image
hashes before showing any source pixels.

Use `QUALITY_EVIDENCE_DIRECTORY` and the `/review/quality` route for a
maximum-detail experiment. The route verifies the derived report plus all five
candidate PNG hashes, supports matched physical-footprint comparison, and
labels source LOD separately from output raster sampling.

The live command is deliberately absent from ordinary CI. A capture timeout,
tile-load error, missing pass, camera mismatch, low coverage result, wrong hash,
or Rust validation failure leaves no accepted output directory.

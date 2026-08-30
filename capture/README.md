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

The 400-request camera ceiling was measured rather than guessed. A 100-request run
loaded 4.12 MiB successfully and then blocked 26 required child requests. The
accepted Hoover run used 282 requests and one billable root session. The probe
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

After Rust validation accepts the immutable bundle, inspect its six registered
layers through the local-only review route:

```bash
REFERENCE_BUNDLE_DIRECTORY="$PWD/artifacts/reference/hoover" npm --prefix web run dev
```

Then open `http://127.0.0.1:5173/isometric-stanford/review`. The development
server exposes only the seven allowlisted bundle files, with no caching, from
the configured directory. It does not copy them into `web/public`, `dist`, or
any release artifact.

The live command is deliberately absent from ordinary CI. A capture timeout,
tile-load error, missing pass, camera mismatch, low coverage result, wrong hash,
or Rust validation failure leaves no accepted output directory.

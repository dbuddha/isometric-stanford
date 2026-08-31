# Google reference capture

Google Photorealistic 3D Tiles are streamed 3D scenes, not precomposed image
tiles. The root request returns an OGC 3D Tiles hierarchy and a session. Child
JSON records select geometry and textured GLB payloads containing glTF data. A renderer must
load that hierarchy, position a camera, resolve the visible level of detail,
and draw pixels. The official [3D Tiles overview](https://developers.google.com/maps/documentation/tile/3d-tiles-overview)
and [renderer guide](https://developers.google.com/maps/documentation/tile/use-renderer)
describe this boundary.

The capture client uses TypeScript, Three.js, and `3d-tiles-renderer` because
they already implement Google session handling, 3D Tiles traversal, glTF and
Draco decoding, GPU rasterization, and dynamic attribution. Reimplementing
that browser boundary in Rust would add a second 3D Tiles and WebGL stack
without improving canonical art determinism. Safe Rust begins after capture:
it validates immutable bundles and will own masks, repair, stylization,
stitching, and publication.

The browser client is built once and served by a local allowlisted static
server. A direct pinned Chromium headless-shell process receives a one-time
loopback coordinator URL in its fragment. The page immediately removes that
fragment, authenticates to the coordinator, receives the credential in memory,
and installs the request ceiling before its first Google request. The key never
enters a browser command line, artifact, manifest, child environment, or
diagnostic. One probe reuses one Google root session while changing only the
camera orientation. Ordinary CI uses synthetic and local protocol fixtures and
never contacts Google.

## Hoover camera probe

The pinned probe centers the source grid on the Wikidata coordinate for Hoover
Tower, 37.4276111, -122.1670000. It captures a 1,024 by 1,024 core with a
128-pixel guard on every side at 250 millimeters per source pixel. The complete
registered view therefore spans 320 meters and the saved core spans 256
meters. This is close to Isometric NYC's public 300-meter, 1,024-pixel camera
experiment while retaining a larger explicit guard.

The private 2026-08-30 run measured three cameras:

| Camera | Visible tiles | Cached geometry | Capture readiness | Coverage |
| --- | ---: | ---: | ---: | ---: |
| 345 degrees azimuth, 45 degrees elevation | 71 | 135.2 MiB | 3.64 seconds | 99.99% |
| 330 degrees azimuth, 42 degrees elevation | 73 | 147.1 MiB | 1.63 seconds | 99.99% |
| 315 degrees azimuth, 42 degrees elevation | 75 | 155.7 MiB | 1.62 seconds | 99.99% |

The complete session made 282 successful requests under a 400-request ceiling:
one root tileset request, 33 JSON records, and 249 GLB payloads. It transferred
15.39 MiB. No request failed or was blocked. A preceding 100-request experiment
completed all 100 responses but blocked 26 more, proving that a 100-request
ceiling cannot load this guarded view reliably.

The recommended Stanford baseline is 330 degrees azimuth and 42 degrees
elevation. It gives Hoover Tower two balanced visible faces, keeps the dome and
shaft readable, makes Stanford's dominant building axes approach useful pixel
art diagonals, and reveals slightly more facade than the 45-degree view. The
345-degree Isometric NYC value remains a useful comparator. The 315-degree view
is geometrically clean but makes important campus axes and foreground masses
less balanced around the tower.

Orthographic camera distance does not control apparent scale. The
orthographic span does. The fixed 2,000-meter distance is retained only to keep
the camera safely outside the terrain while remaining inside the 1 to 5,000
meter clipping interval. The 250-millimeter source scale remains the efficient
baseline. The camera remains fixed for the masking pilot. The maximum-detail
review master uses the separately qualified 125-millimeter output grid
described below.

## Maximum-detail source probe

The quality probe held the 330 degree azimuth, 42 degree elevation camera and
320 by 320 meter guarded footprint fixed. It varied two independent controls:

- Screen-space error, or SSE, controls which levels of Google's 3D hierarchy
  the renderer requests. A smaller number asks for finer source geometry and
  textures.
- Millimeters per pixel controls only output framebuffer sampling on the fixed
  physical footprint. A smaller number produces more output pixels.

The pinned `3d-tiles-renderer` Google plugin normally applies SSE 20 through
its recommended settings. Isometric NYC's pinned source also accepts that
plugin default. This project disables the opaque preset and pins every quality
control in the reference manifest.

The private 2026-08-30 run measured five candidates in one Google session:

| Candidate | Scale | SSE | Requests added | Visible tiles | Triangles | Deepest tile | Cache |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Baseline | 250 mm/px | 20 | 266 | 73 | 237,150 | 23 | 147.1 MiB |
| Finer source LOD | 250 mm/px | 8 | 518 | 224 | 1,370,554 | 25 | 389.1 MiB |
| Aggressive source LOD | 250 mm/px | 4 | 0 | 224 | 1,370,554 | 25 | 389.1 MiB |
| Two-times raster sampling | 125 mm/px | 8 | 0 | 224 | 1,370,554 | 25 | 389.1 MiB |
| Maximum bounded detail | 125 mm/px | 4 | 0 | 224 | 1,370,554 | 25 | 389.1 MiB |

SSE 8 materially improves roofs, the Hoover shaft and crown, tree texture, and
ordinary building detail compared with SSE 20. SSE 4 adds no requests and
produces byte-identical PNG output at both framebuffer scales. This proves an
available source LOD ceiling for this Stanford view, not a permanent guarantee
about every Google session or campus location.

The 125-millimeter grid doubles each saved image dimension from 1,024 to 2,048
pixels and is visibly cleaner at native inspection. It does not add source
mesh or texture hierarchy. Faceted tree crowns, construction, melted thin
objects, and photogrammetry holes that remain at SSE 8 are source defects.
They must be classified, masked, omitted, or reconstructed by later semantic
and deterministic art stages. Generic edge smoothing would also blur valid
architectural boundaries.

The selected maximum-detail profile is therefore 330 degrees azimuth, 42
degrees elevation, 125 millimeters per output pixel, and SSE 8. The efficient
profile is the same camera at 250 millimeters per pixel and SSE 8. Camera
distance remains 2,000 meters for clipping only.

The Photorealistic 3D Tiles root API documents no capture date or historical
imagery parameter. Google updates the dataset and returns attribution per
visible tile, but the client cannot request a pre-construction epoch. Street
View's separate metadata date is not a 3D Tiles selector. Construction must be
classified from the registered Google layers, removed when an underlying
surface can be recovered, or retained as explicit unknown. The active pipeline
cannot consult a second geographic source to invent the hidden state.

## Bounded capture execution

The production path does not retain a Playwright protocol session. It uses a
small Chrome DevTools Protocol connection only to create and navigate the page.
The browser uploads each raw pass directly to a separate credential-free Node
ingest worker. That worker stages raw files, invokes the bounded safe-Rust PNG
encoder, generates exact crops, validates the completed reference bundle, and
promotes it atomically. Neither Node process retains a full encoded image or a
campus-scale raster.

The final 1,280-pixel three-camera measurement used a 96 MiB Google tile-cache
ceiling and Metal-backed ANGLE. Node reached 79 MiB, the ingest worker reached
85 MiB, Chromium reached 675 MiB summed RSS, and the complete tree reached 849
MiB. The one-camera 2,560-pixel pilot reached 79 MiB Node, 90 MiB ingest-worker,
810 MiB Chromium, and 1,014 MiB complete-tree peaks. Both runs retained 99.99
percent core coverage, exact internal joins, and Rust-valid bundles.

The measured worker envelopes are 1 GiB for a 1,280-pixel grid and 1.25 GiB for
a 2,560-pixel grid. Capture concurrency is derived from host memory after
reserving at least 2 GiB and 25 percent for the operating system and other
work, with a hard limit of four workers. A host that cannot fit one envelope
must not schedule capture.

The high-LOD quality session completed 784 of 784 requests, retained
408,005,115 bytes in the renderer cache, and reached a 1,819,934,720-byte
complete-tree peak. The quality spec pins a 2 GiB worker envelope. Scheduling
uses the largest candidate grid plus any larger measured minimum, so a cheap
baseline candidate cannot understate the memory required by later refinement.

The direct browser can observe response status and any CORS-visible declared
content length, but it does not read response bodies solely for telemetry.
`responseBodyBytes` in direct reports is therefore a declared lower bound, not
an exact transfer total. Request counts, formats, statuses, memory, layer
hashes, coverage, and bundle validity remain exact evidence.

The independent fixed-camera overlap run used a 128 MiB cache-retention target
and a 256 MiB hard ceiling. It completed 428 of 428 requests with zero blocked
or failed responses: 395 GLB and 33 JSON. The monolithic capture retained
226,785,670 bytes in the renderer cache. Node reached 85,606,400 bytes, the
ingest worker reached 98,533,376 bytes, Chromium reached 1,073,037,312 bytes,
and the complete tree reached 1,254,883,328 bytes. The run stayed within the
1.25 GiB envelope by approximately 87 MB. The earlier 96 MiB ceiling is not a
safe production setting because the selected working set can exceed a hard
admission cap and starve refinement.

## Formats and evidence

Each accepted camera produces these local private artifacts:

| Artifact | Encoding | Purpose |
| --- | --- | --- |
| `color.png` | RGBA8 PNG | Textured Google reference |
| `whitebox.png` | RGBA8 PNG | Texture-independent geometry and lighting |
| `depth.bin` | `ISOD32V1` little-endian u32 millimeters | Visible-surface depth |
| `normal.png` | RGBA8 encoded view normals | Surface orientation and structural boundaries |
| `fixed-shadow.png` | Gray8 PNG | One project lighting direction |
| `coverage.png` | Gray8 PNG | Valid visible source coverage |
| `reference.manifest.json` | JSON plus SHA-256 | Camera, source, attribution, and hash chain |

The optional private raw archive keeps exact `rgba8`, `gray8`, and
`ISOD32V1` u32 little-endian depth bytes. It exists so Rust comparison can run
without decoding PNGs or contacting Google again. Neither raw archives nor
Google imagery are committed.

The local `/review` workbench verifies one bundle before displaying any layer.
The `/review/overlap` workbench verifies a one-session report and seven hashed
comparison images. It supports synchronized split and wipe comparison, fit and
1:1 navigation, source versus monolithic cores, independent guards, mismatch
heatmaps, scoped gates, camera and geographic-grid facts, request formats,
memory, coverage, and failure classes. Raw Google layers remain local and
cannot enter a public release.

```bash
OVERLAP_EVIDENCE_DIRECTORY=/absolute/private/experiment \
  npm --prefix web run dev -- --host 127.0.0.1
```

Open `http://127.0.0.1:5173/isometric-stanford/review/overlap`.

The `/review/quality` workbench verifies the one-session quality report and all
five candidate image hashes. It compares candidates on the same physical
footprint, provides native-pixel and named problem-area views, and reports LOD,
sampling, requests, selected geometry, coverage, timing, and memory.

```bash
QUALITY_EVIDENCE_DIRECTORY=/absolute/private/quality-experiment \
  npm --prefix web run dev -- --host 127.0.0.1
```

Open `http://127.0.0.1:5173/isometric-stanford/review/quality`.

## Stitching boundary

The original probe proved only that two cells cropped from one guarded core
reassemble exactly. The 2026-08-30 overlap experiment tested the harder
boundary with two geographically adjacent captures and a larger monolithic
oracle from the same root session.

The first control reconstructed the camera at each geographic center. That
approach failed despite a 0.023233-pixel geographic-grid error and a best
registration offset of 0, 0 pixels. Independent-overlap failures reached
147,698 ppm for depth and 171,539 ppm for normals. Reconstructing the camera
changed screen-space LOD selection.

The accepted mechanism constructs the camera once at the Hoover anchor. The
exact world matrix stays fixed while each neighbor receives an off-axis
orthographic frustum. The left, monolithic, and right views retain the same 4
pixels/meter scale and use projection-center X values of approximately 0.8, 0,
and -0.8. The saved 64-pixel seam corridor measured:

| Source layer | Above tolerance | Gate | Result |
| --- | ---: | ---: | --- |
| Color | 30 ppm | 5,000 ppm | Pass |
| Coverage | 0 ppm | 0 ppm | Pass |
| Linear depth | 61 ppm | 100 ppm | Pass |
| View normals | 91 ppm | 100 ppm | Pass |

The corridor crosses 5,944 structural depth edges, including Hoover Tower, so
the result is not a featureless-boundary pass. Visual 1:1 review shows no
chopped tower or ordinary building at the saved join.

The raw comparison remains diagnostic. A separately traversed monolithic view chose
slightly different source LOD: joined-seam depth measured 854 ppm and normals
2,822 ppm. Captured fixed shadow measured 62,057 ppm and the old shadowed
whitebox measured 100,143 ppm in the independent seam. The source-only
independent seam therefore passes, while the monolithic source comparison and
captured-lighting comparisons record adaptive traversal drift.

The implementation now renders whitebox without shadows and anchors the
diagnostic shadow grid to the initial macroblock. Synthetic tests cover that
remediation, but it has not received another live session. Final artwork uses
deterministic Rust lighting rather than captured Google brightness.

## Canonical ReferenceAtlas

Issue #167 now establishes exactness after frozen registered capture rather
than requiring a fresh monolithic Google traversal to select identical LOD.
The `isometric-reference` atlas compiler validates every source bundle, requires
one provider, camera, renderer, source epoch, sampling scale, core, guard, and
session identity, then sorts inputs by stable grid and bundle identity.

Every saved atlas pixel receives exactly one source owner. Selection prefers:

1. valid coverage;
2. greater distance from the capture edge;
3. valid depth and normal structure;
4. finer source sampling;
5. stable source order as the final tie-break.

The compiler streams PNG decode by row into private temporary raw layers. It
materializes one canonical core tile for color, whitebox, depth, normals,
captured shadow diagnostics, and coverage, plus one dense `u16` ownership tile.
It never allocates a campus-sized image. Output is promoted atomically only
after the complete atlas manifest, file hashes, layer headers, rectangular grid,
and every ownership pixel pass validation.

```bash
cargo run --locked -- reference atlas-compile \
  /absolute/private/atlas-request.json \
  /absolute/private/hoover-atlas

cargo run --locked -- reference atlas-inspect \
  /absolute/private/hoover-atlas
```

Public CI compiles an original synthetic 2 by 2 fixture and proves that source
permutations produce the same manifest hash. Private Google bundles and atlas
tiles stay outside Git. Downstream masks and art consume only the canonical
atlas, so a raw provider overlap can remain visible evidence without becoming a
saved-art seam.

### One-session atlas acquisition

The capture workspace now derives the four Hoover cells from one accepted
geographic center. Cell `r0c0` becomes the fixed camera anchor. The other three
targets are solved on its local ground tangent so their centers land at exact
2,048-pixel horizontal and vertical offsets. The browser does not reconstruct
or rotate the camera between cells. It changes only the off-axis orthographic
frustum while retaining the 330 degree azimuth, 42 degree elevation, SSE 8,
125 millimeter source grid, 256-pixel guard, and 2,560-pixel processing window.

The browser hashes the successful `/v1/3dtiles/root.json` response through Web
Crypto before discarding the temporary body clone. Accepted telemetry contains
exactly one root tileset request and its 64-character digest. The compiler
request stores a derived non-secret session identifier, that digest, and the
three-hour acquisition interval. It never stores the root body, key, request
URL, or provider session URL.

Public browser assurance renders four original synthetic cells, promotes them
through the same registered bundle writer, and compiles their Google-shaped
container contract through the real Rust atlas CLI. Unit tests separately
prove row-major identities, all 16,777,216 saved pixel-center translations,
the fixed world matrix, projection scale and centers, root hashing, secret
redaction, and rejection of profile drift.

The first live private run stopped safely at the former 1,000-request ceiling.
It attempted 1,000 requests, completed 987 successful responses, blocked 10,
and retained no bundle or atlas output. This demonstrates that one maximum-detail
cell's request budget cannot be multiplied linearly or inferred from the earlier
single-view probe. The qualified profile now permits at most 4,000 attempted
requests, keeps one root session, and preserves every quality control. A live
private rerun remains required before `REF-ATLAS-QUAL-001` can pass.

## Adversarial findings

- Two live captures with the same request differed in 0.0003 to 0.0007 percent
  of color pixels. The browser GPU and live upstream are therefore reference
  acquisition, not a byte-deterministic renderer. Accepted layer hashes freeze
  the input from which Rust becomes deterministic.
- Repeated Playwright-controlled probes reached 740 to 865 MiB Node peak RSS.
  Direct Chromium, process-isolated ingest, raw streaming, and Rust PNG encoding
  reduced Node to 79 MiB. The original 768 MiB complete-tree estimate was still
  below the measured 849 MiB quality-preserving process tree, so ADR 0007
  replaced it with a 1 GiB envelope rather than reducing reference level of
  detail or process isolation.
- Google geometry preserves the tower and ordinary buildings strongly, but
  tree crowns, construction, thin objects, and some terrain edges are visibly
  rough. These pixels are evidence for masks and structure. They are not a
  finished art layer and should not be repaired by a generic smoothing filter.
- SSE 20 was a renderer recommendation, not a Stanford quality ceiling. SSE 8
  increased selected triangles by 5.78 times. SSE 4 then plateaued exactly.
- Doubling framebuffer sampling makes the selected source easier to inspect
  but cannot recover geometry or texture detail Google did not serve.
- The current 3D Tiles API has no documented historical imagery selector, so
  it cannot remove construction by choosing an older epoch.
- The source capture intentionally disables browser antialiasing so depth,
  normal, and coverage edges remain unambiguous. A later review-only
  supersampled color preview may improve human inspection, but canonical masks
  and final pixel art must continue to use the registered hard-edge layers.
- A full monolithic view and two smaller off-axis views can ask the dynamic
  renderer for different refinement even when their camera matrix and pixel
  scale match. The monolithic image is evidence, not automatically an exact
  pixel oracle for every independently traversed source cell.

Google's standard [usage and billing guide](https://developers.google.com/maps/documentation/tile/usage-and-billing)
separates the root quota from renderer requests. Root tileset requests count
toward the default 10,000-per-day quota. Child renderer requests do not consume
that daily root quota and one root session can serve requests for at least three
hours. Google's [price sheet](https://developers.google.com/maps/billing-and-pricing/pricing)
defines each returned Photorealistic 3D tile as a billable event, lists a
1,000-event monthly free usage cap, and lists USD 6 per 1,000 events in the
first paid tier. The project had previously mislabeled only root calls as
billable. Telemetry now calls them `rootTilesetRequests`, while `completed`
tracks successful returned tile events. The repository still treats owner
authorization and private handling as explicit project constraints rather than
inferring derivative rights from billing behavior. Account-wide usage and
negotiated pricing can differ from the public sheet.

The complete source archaeology and raw measurements are retained in the
[registered capture research package](../research/registered-capture/index.md).

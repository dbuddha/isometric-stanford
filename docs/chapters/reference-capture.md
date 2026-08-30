# Google reference capture

Google Photorealistic 3D Tiles are streamed 3D scenes, not precomposed image
tiles. The root request returns an OGC 3D Tiles hierarchy and a session. Child
JSON records select geometry and textured glTF binary payloads. A renderer must
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
server. The credential enters only an isolated Playwright browser context.
The request interceptor applies a configured ceiling before navigation and
records counts, response status, payload format, and transfer bytes without
retaining URLs, keys, or session values. One probe reuses one Google root
session while changing only the camera orientation. Ordinary CI uses the
synthetic provider and never contacts Google.

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
one billable root request, 33 JSON records, and 249 GLB payloads. It transferred
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
meter clipping interval. The 250-millimeter source scale should remain fixed
for the masking pilot. Later pixel-art reduction may combine source samples,
but it must not change the registered capture grid.

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

The local review workbench verifies that manifest before displaying any layer.
It supports synchronized split and wipe comparison, fit and 1:1 navigation,
layer selection, immutable hashes, camera facts, coverage, and attribution.
Raw Google layers remain local and cannot enter a public release.

## Stitching boundary

The current probe proves that two 512-pixel cells cropped from one guarded
1,024-pixel core reassemble exactly. Both cells have zero mismatched pixels
against the same monolithic source crop. This is the correct internal cell
boundary because masks and local filters will operate on the guarded
supertile before it is sliced.

This result does not yet prove the harder boundary between two independently
captured supertiles. That later gate must render neighboring registered
supertile cameras, compare their geographic overlap, and fail on coverage,
camera, level-of-detail, or subpixel disagreement before Stanford-scale
collection begins. Art seams remain a separate Rust gate: guarded deterministic
stylization of adjacent cells must equal a monolithic stylization oracle.

## Adversarial findings

- Two live captures with the same request differed in 0.0003 to 0.0007 percent
  of color pixels. The browser GPU and live upstream are therefore reference
  acquisition, not a byte-deterministic renderer. Accepted layer hashes freeze
  the input from which Rust becomes deterministic.
- Repeated live static-server probes reached 740 to 865 MiB Node peak RSS.
  Browser JavaScript used about 97 MiB and each final three-bundle evidence set
  occupied about 61 MiB.
  The high Node peak is not acceptable as the projected campus acquisition
  baseline. Buffering and PNG encoding require a dedicated memory profile and
  streaming correction before parallel capture.
- Google geometry preserves the tower and ordinary buildings strongly, but
  tree crowns, construction, thin objects, and some terrain edges are visibly
  rough. These pixels are evidence for masks and structure. They are not a
  finished art layer and should not be repaired by a generic smoothing filter.
- The source capture intentionally disables browser antialiasing so depth,
  normal, and coverage edges remain unambiguous. A later review-only
  supersampled color preview may improve human inspection, but canonical masks
  and final pixel art must continue to use the registered hard-edge layers.

Google's standard [usage and billing guide](https://developers.google.com/maps/documentation/tile/usage-and-billing)
distinguishes billable root sessions from child tile requests. The repository
still treats owner authorization and private handling as explicit project
constraints rather than inferring derivative rights from billing behavior.

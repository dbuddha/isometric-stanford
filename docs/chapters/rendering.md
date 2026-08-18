# Rendering pipeline

The canonical final-art boundary is safe Rust on a pinned Linux CPU. Registered
reference layers share one orthographic camera and pixel grid. The stylizer
uses checked integer or fixed-point kernels, an indexed palette, and no
anti-aliasing at native art resolution. The existing geometric renderer
remains the deterministic comparison baseline and procedural-overlay engine.

The pass order is terrain, water, hardscape, buildings, vegetation, hard
shadows, world-anchored dithering, outlines, and landmark detail. Stable object
IDs select approved procedural variations. A guarded supertile renders beyond
every saved tile boundary, then crops to the canonical tile. Patterns, shadows,
and random-looking detail are anchored in world coordinates so they do not
restart at tile edges.

The implemented raster core uses 1/256-pixel vertices, an integer depth buffer,
pixel-center sampling, and a half-open shared-edge rule. It canonicalizes each
batch by nonzero stable primitive key before drawing. Larger depth values win;
at equal depth, the lower key retains ownership. The one-batch surface contract
makes that tie rule independent of caller order without allocating an owner ID
per pixel.

Every production surface owns exactly one palette byte and one 32-bit depth
value per pixel. Dimensions are capped at 4,096 pixels per side, and invalid
keys, palette indexes, coordinates, depths, degenerate triangles, duplicate
submission, and checked-arithmetic failures stop rendering. The original
diamond-and-column reference renderer remains only as a regression fixture.

The ordinary-scene compiler projects canonical rings into the camera, slices
each polygon at its vertex rows, pairs crossings using even-odd fill, and emits
trapezoids as shared-edge triangles. This deterministic decomposition supports
concave polygons and holes without a floating-point tessellation dependency.
Ground classes receive explicit palette and depth layers. Buildings receive a
flat roof and directional wall faces at their compiled heights. Vegetation
polygons receive a stable, jittered world grid of original faceted crowns.
Placement is clipped to each polygon and its holes.

Buildings and crowns are rasterized into a separate hard-shadow surface. The
mask is composited only onto eligible ground, hardscape, water, and athletic
pixels, so it cannot darken a closer building or canopy. The main raster target
still owns exactly five bytes per active pixel. During composition the output
image coexists with one five-byte shadow surface, and outline processing uses
one temporary palette byte per pixel. Peak memory therefore remains bounded by
tile dimensions rather than total map dimensions.

Terrain and athletic patterns hash absolute projected coordinates rather than
tile-local coordinates. Cropping or moving a guarded tile does not restart the
pattern. Outlines operate only on building and canopy color families and remain
one logical pixel wide. Neither operation introduces alpha blending,
anti-aliasing, tile-local phase, or colors outside the style palette.
Reference-derived art may retain accepted material hue and structural detail,
but cannot copy an unmodified photographic region into the saved palette
output.

The implemented capture boundary precedes canonical Rust rendering. A pinned
Three.js and 3d-tiles-renderer page positions one orthographic camera over the
registered world grid and renders all six reference passes without changing
camera or canvas dimensions. Playwright waits for a root tileset, an empty
load queue, a minimum visible set, and a stable signature before reading any
pixels. Raw layers travel over a tokenized loopback endpoint, not command-line
arguments or base64 logs. The writer emits deterministic PNG containers and a
millimeter depth payload, hashes every artifact, asks `isometric-reference` to
validate the staged bundle, then atomically promotes it. A failed or partial
capture leaves no usable bundle.

The manifest freezes camera distance as well as target, azimuth, elevation,
near plane, far plane, and orthographic spans. This makes both visible geometry
and linear depth reproducible from the registered request.

The browser GPU render is a reference acquisition step, not the canonical art
renderer. Live upstream sessions are not expected to be byte-identical. Once
accepted, the six-layer bundle is immutable and its hashes become the fixed
input to mask fusion, obstruction repair, and Rust stylization.

The local reference workbench reads these immutable bytes sequentially under
fixed limits and verifies the complete hash chain before visual review. It
retains compressed bytes for the six inputs but decodes only the selected pair.
All comparison transforms are expressed in source pixels, so color, geometry,
depth, normal, shadow, and coverage views remain registered during pan, zoom,
and wipe inspection. The depth grayscale is a dashboard visualization and is
never an input to canonical masking or art.

The renderer derives one immutable full-scene coordinate layout without
allocating its framebuffer. A tile request renders only objects whose
conservative projected bounds intersect its guard, applies all scene passes in
that world-aligned guard, and crops the saved tile. The required guard is
derived from the style's shadow, maximum tree crown, crown height, and outline
extent rather than being a magic constant. Tests reconstruct the complete hero
from independently rendered tiles and require byte equality with the monolithic
oracle. Tile evidence records selected objects, submitted primitives, guarded
dimensions, and a conservative six-byte-per-guarded-pixel peak for palette and
depth buffers.

The fused semantic hero preview is 1,954 by 880 pixels with indexed hash
`de91fe6a35a4d987` and lossless PPM SHA-256
`aad8b6d3689ee339d7fdc5bb2be1c3d8d9bd789e63267c187a1a7b6e17c3e827`.
The 8K-scale Candidate C pyramid is 7,623 by 3,325 pixels and retains the same
bounded guarded-tile contract. On the measured development machine, publishing
its 157 lossless WebP tiles completed in at most 1.06 seconds at 22,429,696
bytes peak RSS. This is engineering evidence, not artistic approval. The
guarded indexed-tile seam oracle and lossless WebP DZI publisher are
implemented. Publication retains indexed canonical tiles, derives lower levels
with top-left nearest-neighbor sampling, and validates decoded WebP colors
against the palette before accepting an artifact. General detailed roof grammar
remains tracked work.

`wgpu` is not a style engine and is not a v1 dependency. It becomes a research
candidate only after profiling shows the CPU renderer misses the accepted
eight-hour full-estate budget.

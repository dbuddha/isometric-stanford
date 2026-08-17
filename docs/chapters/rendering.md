# Rendering pipeline

The canonical renderer is safe Rust on a pinned Linux CPU. It uses a fixed 2:1
orthographic camera, integer world coordinates, fixed-point screen coordinates,
checked arithmetic, an indexed palette, and no anti-aliasing at native art
resolution.

The pass order is terrain, water, hardscape, buildings, vegetation, hard
shadows, world-anchored dithering, and outlines. Landmarks will add a dedicated
pass in P-014. Stable object
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
anti-aliasing, source pixels, or colors outside the style palette.

The resulting vector-only hero preview is 1,950 by 873 pixels with indexed hash
`3fa06d391073b5b6` and lossless PPM SHA-256
`ca2b341e3421e6e86613fad77e9a170e406ff721a4ed2e84553d2e466c3808c7`.
On the measured development machine, release rendering completed in about 0.3
seconds of renderer CPU time at roughly 25 MB peak RSS. This is engineering
evidence, not artistic approval. Guarded supertiles, detailed roof grammar,
landmarks, and the seam oracle remain tracked work.

`wgpu` is not a style engine and is not a v1 dependency. It becomes a research
candidate only after profiling shows the CPU renderer misses the accepted
eight-hour full-estate budget.

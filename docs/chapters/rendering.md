# Rendering pipeline

The canonical renderer is safe Rust on a pinned Linux CPU. It uses a fixed 2:1
orthographic camera, integer world coordinates, fixed-point screen coordinates,
checked arithmetic, an indexed palette, and no anti-aliasing at native art
resolution.

The intended pass order is terrain, water, hardscape, buildings, vegetation,
landmarks, hard shadows, outlines, and world-anchored dithering. Stable object
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
flat roof and directional wall faces at their compiled heights.

The resulting vector-only hero preview is 1,950 by 873 pixels with indexed hash
`26132f9895f0cd70` and lossless PPM SHA-256
`7af31aecd72149fe3cc3618c8bbf310c358b126f37176c5727645742e4363e59`.
On the measured development machine, release rendering completed in 0.31
seconds at roughly 20 MB peak RSS. This is engineering evidence, not artistic
approval. Guarded supertiles, vegetation crowns, detailed roof grammar,
shadows, outlines, dithering, and the seam oracle remain tracked work.

`wgpu` is not a style engine and is not a v1 dependency. It becomes a research
candidate only after profiling shows the CPU renderer misses the accepted
eight-hour full-estate budget.

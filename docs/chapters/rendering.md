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

World-to-triangle pass composition, guarded supertiles, and the seam oracle
remain tracked work.

`wgpu` is not a style engine and is not a v1 dependency. It becomes a research
candidate only after profiling shows the CPU renderer misses the accepted
eight-hour full-estate budget.

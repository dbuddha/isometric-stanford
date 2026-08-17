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

The bootstrap implements projection and a small palette-indexed reference
grammar only. Production triangle rasterization, depth, pass composition,
guarded supertiles, and the seam oracle remain tracked work.

`wgpu` is not a style engine and is not a v1 dependency. It becomes a research
candidate only after profiling shows the CPU renderer misses the accepted
eight-hour full-estate budget.

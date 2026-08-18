# Performance and memory

The offline target is at least 100 accepted 512 by 512 tiles per minute on a
qualified 16-core machine, below 4 GB target peak memory and 8 GB hard maximum.
The complete qualification slice must finish in 90 minutes. Each worker owns
bounded tile, guard, geometry, depth, and scratch buffers independent of total
map size.

Benchmark records include commit, manifest hashes, machine identity, CPU and
memory configuration, worker count, warmup, sample distribution, accepted and
rejected tile counts, peak RSS, and output hashes. A single best run is not
qualification evidence.

If profiling projects more than eight hours for the full estate, first remove
algorithmic waste and validate parallel partitioning. Only then open the
accepted `wgpu` research decision. A GPU backend must preserve the semantic and
CPU oracle and cannot redefine style output for speed.

The raster core fixes each active raster surface at five bytes per pixel: one
indexed color byte and one 32-bit depth value. A 512 by 512 tile therefore owns
1,310,720 pixel-buffer bytes before bounded triangle and encoder scratch space.
The main indexed image coexists with the five-byte shadow surface during
composition, producing a conservative six-byte-per-guarded-pixel peak before
bounded triangle and encoder scratch space. The surface cap prevents accidental
master-frame allocation. Conservative projected bounds prevent off-tile objects
from producing primitives, and viewport bounds limit each selected triangle's
pixel loop to the region it can touch.

Geometry-kernel rasters are capped at 4,096 pixels per side and use 32-bit
stable row-major pixel identities. The pilot supertile is 2,560 by 2,560
including its guard. A retained Scharr field uses five logical bytes per pixel:
one 32-bit magnitude and one one-byte direction. Hysteresis adds one state byte
and a worst-case 32-bit queue identity per pixel, for a conservative 65.6 MB of
logical raster storage at the pilot size. Morphology uses one input, one
horizontal scratch raster, and one output byte per pixel. Pipeline stages drop
scratch storage before the next whole-supertile operation. Actual process RSS,
allocator overhead, and concurrency remain release-measurement evidence rather
than assumptions in this design accounting.

The dependency-free 250 millimeter scale probe produces a 7,623 by 3,325 pixel
layout as 105 independently rendered 512-pixel tiles with an 80-pixel derived
guard. The fused semantic-world probe selected at most 325 of 2,820 world
objects per tile, emitted 393,441 bounded primitives, and reported a
2,709,504-byte maximum guarded pixel-buffer budget. Its aggregate indexed hash
is `8a79308a6218f976`. These numbers prove the bounded raster stage, not source
compilation or fixed-device release performance.

The model-free source compiler separately decoded 1,065,077 in-bounds NAIP
samples and streamed 8,506,505 in-bounds LiDAR points from approximately 450 MB
of locked source data. A release run completed in 38.02 seconds with 36,438,016
bytes maximum RSS on the development machine. Its reusable LAZ buffer is capped
at 250,000 points, independent of total input size. The resulting 372-cell
artifact is 189,076 bytes and byte deterministic.

Candidate B increases ordinary-scene primitive density with facade openings,
convex roof planes, and closer tree spacing. A complete four-scene review pack
still completed in 1.14 seconds with 86,917,120 bytes maximum RSS on the local
development probe. Candidate B guarded tiles reconstruct the monolithic scene
exactly. Review assembly holds one approximately 25 MB indexed hero master;
canonical render workers remain bounded to one guarded tile.

Candidate C adds only deterministic palette treatment and bounded facade
primitives. A complete four-scene review pack completed in 1.35 seconds with
88,539,136 bytes maximum RSS on the local development probe. Two clean packs
were byte-identical, and guarded Candidate C tiles reconstruct the monolithic
scene exactly. This remains development evidence rather than fixed-device
release qualification.

The current end-to-end Candidate C publication harness builds three independent
7,623 by 3,325 pyramids and compares every path and byte. On the 10-logical-core
arm64 development machine, the fused-world runs completed in 1.03 to 1.06
seconds, used at most 22,429,696 bytes maximum RSS, and produced the same
157-tile
artifact and tile-set SHA-256
`1f0261eb5141a4a37bc43f072aa29e839bd5c35724766b4a71b15a5d5752cd41`.
The 105 maximum-resolution tiles sustained at least 5,946 tiles per minute.
Candidate C's complete served WebP set is 4,324,252 bytes. Initial viewport
bytes are measured separately because browsers do not fetch the complete
pyramid at startup. These are development regression results, not fixed-device
qualification evidence.

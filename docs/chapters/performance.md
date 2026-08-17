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

The dependency-free 250 millimeter scale probe produces a 7,623 by 3,325 pixel
layout as 105 independently rendered 512-pixel tiles with an 80-pixel derived
guard. A release run on the development machine rendered the complete indexed
tile set in 0.73 seconds, selected at most 303 of 2,820 world objects per tile,
and reported a 2,709,504-byte maximum guarded pixel-buffer budget. The wrapping
process measured 32,636,928 bytes maximum RSS. Its aggregate indexed hash is
`bf0604f68bc38d2c`. These numbers prove the bounded raster stage, not WebP
encoding, source compilation, or fixed-device release performance.

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
`bf0604f68bc38d2c`. These numbers prove the bounded raster stage, not source
compilation or fixed-device release performance.

The implemented publisher converts that layout into 157 WebP tiles across all
DZI levels. A warm release run on the development machine completed the full
publication in 1.07 seconds with 16,154,624 bytes maximum RSS. The served WebP
set totals 1,759,172 bytes; retained canonical indexed tiles make the complete
local artifact about 35 MiB. Two clean publications were byte-identical with
tile-set SHA-256
`cee6b78366a6b1fed5b49ee663f2e524a82c4a5209f9e67d3b5dd74ea142e9e6`.
These are local regression measurements, not fixed-device qualification.

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

The end-to-end Candidate C publication harness builds three independent
7,623 by 3,325 pyramids and compares every path and byte. On the 10-logical-core
arm64 development machine, the three runs completed in 0.92 to 0.93 seconds,
used at most 16,089,088 bytes maximum RSS, and produced the same 157-tile
artifact and tile-set SHA-256
`d5cba2ce1bb6ca2fd5b909d6f0f341f9bca1b188844acd956eeebd043f981e3b`.
The 105 maximum-resolution tiles sustained at least 6,753 tiles per minute.
Candidate C's complete served WebP set is 3,421,208 bytes. Initial viewport
bytes are measured separately because browsers do not fetch the complete
pyramid at startup. These are development regression results, not fixed-device
qualification evidence.

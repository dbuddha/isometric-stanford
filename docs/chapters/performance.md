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

The raster core fixes active pixel storage at five bytes per pixel: one indexed
color byte and one 32-bit depth value. A 512 by 512 tile therefore owns
1,310,720 pixel-buffer bytes before bounded triangle and encoder scratch space.
The surface cap prevents accidental master-frame allocation. Primitive keys are
sorted once per tile, while viewport bounds limit each triangle's pixel loop to
the region it can touch.

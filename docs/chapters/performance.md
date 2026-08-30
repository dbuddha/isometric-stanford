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

The live Hoover acquisition path is memory-qualified under the revised
measured envelopes in ADR 0007. The three-camera 1,280-pixel probe reached 79
MiB Node, 85 MiB ingest-worker, 675 MiB Chromium, and 849 MiB complete-tree peak
RSS. The one-camera 2,560-pixel pilot reached 79 MiB Node, 90 MiB ingest-worker,
810 MiB Chromium, and 1,014 MiB complete-tree peaks. Both stayed inside the 1
GiB and 1.25 GiB operational worker envelopes, retained 99.99 percent core
coverage, produced exact internal joins, and passed Rust bundle validation.

The later fixed-camera overlap experiment is the highest measured acquisition
load. It rendered one 2,304 by 1,280 monolithic grid and two 1,280 by 1,280
neighbors through one Chromium session. It completed 428 requests, retained up
to 226,785,670 renderer-cache bytes, and reached these peaks:

| Process boundary | Peak RSS |
| --- | ---: |
| Node orchestrator | 85,606,400 bytes |
| Ingest worker | 98,533,376 bytes |
| Chromium process family | 1,073,037,312 bytes |
| Complete process tree | 1,254,883,328 bytes |

The complete tree remained below the 1,342,177,280-byte envelope. Candidate
readiness took 3.132 seconds for the monolithic view, 1.518 seconds for the
left view, and 1.632 seconds for the right view after scene startup. These are
capture readiness times, not complete campus throughput.

Capture scheduling reserves at least 2 GiB and 25 percent of host memory, uses
the smaller resulting capacity, divides by the measured per-grid envelope, and
caps concurrency at four. It returns zero workers when no measured envelope
fits. This prevents the later campus collector from guessing concurrency or
allocating a campus master image. See
[Google reference capture](reference-capture.md) for the measured workload and
[ADR 0007](../adr/0007-bounded-reference-capture.md) for the rejected 768 MiB
tree assumption.

A 24 GiB host can fit four measured envelopes arithmetically, but memory is not
the only admission constraint. Four independent Chromium sessions would also
multiply Google traversal, GPU residency, request rate, cache pressure, and
failure recovery. The current recommendation is one serial session per
registered macroblock during the pilot. Increase acquisition concurrency only
after a long-session experiment measures peak memory, cache starvation,
throughput, and request reuse. Rust comparison and post-capture processing may
parallelize independently under their smaller per-cell budgets.

The renderer cache uses a 128 MiB retention target and a 256 MiB ceiling. A
smaller hard cap is not necessarily more efficient: an upstream renderer issue
documents that a demanded working set larger than the admission cap can stall
refinement and cause repeated parse and eviction work. This project observed
72 failed responses and much lower selected geometry in its earlier 96 MiB
camera-recentered control. The causal attribution is not independently
qualified, but the fixed 256 MiB run completed all requests and retained the
required detail.

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

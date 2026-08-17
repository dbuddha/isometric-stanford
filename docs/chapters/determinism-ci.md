# Determinism and CI

Canonical Linux rendering runs three times and must produce identical output
hashes. Stable object identity, fixed input order, integer projection, checked
arithmetic, indexed color, world-space patterns, and pinned toolchains make the
contract explicit.

Cross-platform semantic and material IDs must match exactly. Cross-platform
images use an accepted palette-index tolerance because encoders and future
platform-specific tooling may differ. Canonical release bytes still come from
one pinned Linux environment.

`ci-pass` aggregates policy, Rust, Python, dependency and license, mdBook, web,
golden, seam, determinism, semantic, no-transients, and release-manifest jobs.
Ordinary pull requests target a fifteen-minute completion budget. Coverage,
mutation, Kani, fuzzing, full-slice rendering, model benchmarks, and browser
traces run when selected by risk or schedule.

CI must not rerun a failed job until the cause is understood, weaken a threshold
to obtain green status, or treat hosted timing as fixed-device qualification.

The hero compiler runs twice in the Rust test suite and compares complete JSON
bytes. The second comparison checks the generated manifest against the
committed `world.manifest.json`, which pins the world SHA-256 and both vector
source SHA-256 values. On the measured development machine, a release build
compiled 2,820 objects using approximately 16 MB maximum resident memory after
the one-time Rust build. Source verification is streamed separately with a 64
KiB copy buffer.

The renderer's ordinary test gate reassembles the hero from independently
guarded tiles and compares every palette index with the monolithic oracle. The
scheduled release-only probe renders the complete 250 millimeter tile set,
checks its aggregate `bf0604f68bc38d2c` hash, and enforces the per-tile pixel
memory ceiling. WebP determinism remains outside this evidence until the
encoder decision is accepted and implemented.

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

For pull request events, the policy job also validates the review contract from
the immutable event payload. Titles must follow the repository Conventional
Commit form, bodies must retain all four evidence sections and link a GitHub
issue, and exactly one `release:*` label must be present. The aggregate gate
cannot pass when review metadata is incomplete.

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
memory ceiling.

Ordinary CI regenerates Candidates A and B from the same locked world and
requires their indexed and encoded contact-sheet hashes to match the frozen
review records. Candidate A therefore cannot drift while later grammar is
added. Scheduled assurance also generates Candidate B twice and requires a
recursive byte-for-byte artifact comparison.

The publisher uses pinned pure-Rust `image-webp` 0.2.4. Ordinary Rust tests
publish the same fixture twice, compare every artifact byte, decode every WebP
back to approved palette colors, and prove corrupt bytes fail validation.
Scheduled assurance compiles the locked hero world, publishes two clean
pyramids, validates both complete hash chains, and requires a recursive
byte-for-byte directory comparison.

The ordinary web gate runs desktop and mobile Playwright checks against a
minimal lossless DZI, including descriptor retry, exhausted tile retry, Canvas
context restoration, keyboard-accessible controls, and painted output. The
scheduled browser gate replaces that fixture with a newly compiled complete
hero pyramid. This separates fast recovery-contract feedback from full-artifact
integration evidence.

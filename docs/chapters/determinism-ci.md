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

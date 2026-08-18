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
cannot pass when review metadata is incomplete. CI runs on pull request open,
commit synchronization, reopen, body edit, and label changes so correcting
review metadata always produces fresh authoritative evidence.

CI must not rerun a failed job until the cause is understood, weaken a threshold
to obtain green status, or treat hosted timing as fixed-device qualification.

Registered reference capture is not assumed byte-stable across live upstream
sessions. A capture becomes canonical only after its six layers and manifest
are frozen and hashed. Ordinary CI validates synthetic or locked bundles for
shared dimensions, camera identity, encoding, depth payload length, coverage,
safe paths, and exact hashes. From a frozen bundle and accepted mask onward,
the Rust stylizer and guarded seam oracle must be byte-identical.

The hero compiler runs twice in the Rust test suite and compares complete JSON
bytes. The second comparison checks the generated manifest against the
committed `world.manifest.json`, which pins the world SHA-256, all seven source
SHA-256 values, and the frozen perception SHA-256. The suite also removes one
evidence cell and proves incomplete coverage fails closed. Source verification is
streamed separately with a 64 KiB copy buffer.

The model-free perception compiler is tested independently for its CRS control
point, artifact ordering, policy metadata, and transient exclusion. Ordinary CI
validates the frozen artifact and does not download 450 MB of raw sources. The
weekly scheduled job synchronizes every exact source, recompiles evidence twice
with a serial 250,000-point LAZ buffer, compares both runs, and requires the
committed SHA-256.

The renderer's ordinary test gate reassembles the hero from independently
guarded tiles and compares every palette index with the monolithic oracle. The
scheduled release-only probe renders the complete 250 millimeter tile set,
checks its aggregate `8a79308a6218f976` hash, and enforces the per-tile pixel
memory ceiling.

Ordinary CI regenerates Candidates A, B, and C from the same locked world and
requires their indexed and encoded contact-sheet hashes to match the frozen
review records. Earlier candidates therefore cannot drift while later grammar
is added. Scheduled assurance also generates Candidate C twice, recursively
compares every artifact byte, and rechecks the encoded Candidate A and B
hashes.

The prototype performance harness separately publishes three complete
Candidate C DZI directories under the pinned release binary. It hashes sorted
paths and bytes, validates the style identity and complete artifact chain, and
fails if any run differs, exceeds 512 MiB RSS or 20 minutes, or falls below 100
maximum-level tiles per minute. The resulting JSON records the commit, machine,
commands, timings, peak RSS, throughput, release dimensions, served bytes, and
tile hashes.

The publisher uses pinned pure-Rust `image-webp` 0.2.4. Ordinary Rust tests
publish the same fixture twice, compare every artifact byte, decode every WebP
back to approved palette colors, and prove corrupt bytes fail validation.
Scheduled assurance compiles the locked hero world, publishes three clean
Candidate C pyramids, validates each complete hash chain, and requires one
directory hash across every run.

The ordinary web gate runs desktop and mobile Playwright checks against a
minimal lossless DZI, including descriptor retry, exhausted tile retry, Canvas
context restoration, keyboard-accessible controls, and painted output. The
scheduled browser gate replaces that fixture with a newly compiled complete
Candidate C hero pyramid and attaches desktop and mobile regression metrics.
This separates fast recovery-contract feedback from full-artifact
integration evidence.

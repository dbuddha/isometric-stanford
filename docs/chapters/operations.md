# Operations and updates

An update begins with `isometric-stanford source sync`. The command validates
the complete lock before transfer, downloads through a fixed 64 KiB buffer,
verifies the declared length and SHA-256 hash, and atomically promotes each
artifact into `artifacts/source-cache/sha256`. Existing cache entries are
rehashed before reuse. A hash or length mismatch leaves no accepted artifact.

A changed hash invalidates perception artifacts, world partitions, shadow dependencies,
guarded render tiles, DZI ancestors, and the release candidate through explicit
dirty bounds.

Operators inspect source changes, unknown growth, model disagreement, hero
geometry, transient masks, and visual comparisons before promoting artifacts.
Every stage emits an immutable manifest and keeps the previous qualified chain
available for rollback. Git stores manifests, fixtures, code, style assets, and
small goldens. Large source and DZI artifacts live outside Git in the
content-addressed cache until versioned object storage is introduced.

`isometric-stanford publish dzi` stages a new candidate in a sibling
`.partial` directory and renames it only after every level, tile, descriptor,
and manifest succeeds. It refuses existing final and staging paths. The
maximum level is rendered as independently guarded indexed tiles; lower levels
are derived from at most four parent tiles at a time. `validate release`
rechecks pyramid completeness, all SHA-256 values, indexed dimensions and
palette membership, and lossless WebP decoding before an artifact can proceed.

No automated workflow publishes a GitHub release. It may build a dry run and
qualification report. The owner reviews provenance, style, fixed-device web
evidence, and the immutable release manifest before publication.

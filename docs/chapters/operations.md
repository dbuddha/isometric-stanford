# Operations and updates

An update begins by syncing approved source metadata and hashes. Changed hashes
invalidate perception artifacts, world partitions, shadow dependencies,
guarded render tiles, DZI ancestors, and the release candidate through explicit
dirty bounds.

Operators inspect source changes, unknown growth, model disagreement, hero
geometry, transient masks, and visual comparisons before promoting artifacts.
Every stage emits an immutable manifest and keeps the previous qualified chain
available for rollback. Git stores manifests, fixtures, code, style assets, and
small goldens. Large source and DZI artifacts live in versioned object storage.

No automated workflow publishes a GitHub release. It may build a dry run and
qualification report. The owner reviews provenance, style, fixed-device web
evidence, and the immutable release manifest before publication.

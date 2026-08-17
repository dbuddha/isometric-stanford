# Operations and updates

An update begins with `isometric-stanford source sync`. The command validates
the complete lock before transfer, downloads through a fixed 64 KiB buffer,
verifies the declared length and SHA-256 hash, and atomically promotes each
artifact into `artifacts/source-cache/sha256`. Existing cache entries are
rehashed before reuse. A hash or length mismatch leaves no accepted artifact.

HTTPS acquisition applies separate 30 second connection, 60 second response
header, and 300 second response-body deadlines. It makes at most
three attempts and retries only connect, timeout, stalled-body, interrupted
stream, and explicitly transient HTTP status failures. Permanent HTTP status,
locked-length, digest, local I/O, and partial-file failures are not retried.
Every failed attempt removes its partial file, and every operator-facing line
names the stable source ID and acquisition stage without printing its URL.
Byte-range resume is deliberately deferred until an upstream-specific range
contract can prove that resumed bytes are immutable and equivalent to a clean
download.

Scheduled assurance caches `artifacts/source-cache` under a key containing the
entire `source.lock.json` digest. No prefix restore is allowed. A workflow
dispatch namespace makes a deliberate first run cold and a second run warm;
both still rehash all restored artifacts before compilation. The uploaded
`source-sync.log` records whether each source was downloaded, the attempt
count, or a verified cache hit.

The 7.2 MB NAIP hero crop is a committed licensed source fixture. A cold run
imports and rehashes it locally because the USDA FPAC export endpoint rejects
connections from GitHub-hosted runners. Its original item, date, dimensions,
license, attribution, length, and digest remain locked. The approximately 440
MB USGS LiDAR bundle remains remote and enters the Actions cache only after the
same exact verification boundary.

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

# Data, asset, and attribution policy

Every external source must be approved and recorded before its content enters a
fixture, compiled artifact, style asset, golden image, or public release.

Each record must include:

- Stable source name and URL
- Publisher and retrieval date
- Geographic and temporal coverage
- License or written permission
- Required attribution and notice text
- Original content hash
- Permitted transformations and redistribution boundary
- Generated artifact hashes and dependency relationship
- Reviewer and approval reference

The prototype baseline uses the exact OpenStreetMap, Overture Buildings, USDA
NAIP, and USGS 3DEP LiDAR records approved in `source.lock.json`. The lock
records retrieval and metadata URLs, dates, geographic bounds, licenses,
required attribution, byte lengths, and SHA-256 hashes. Adding another source
requires an explicit provenance review and lock update.

Google content is not an approved production source. It may enter the pipeline
only after written permission expressly authorizes the intended retrieval,
derivative production, storage, and public redistribution. A URL, API key, paid
account, or ability to view content is not permission.

Isometric NYC is a research reference, not an asset source. Do not copy or
redistribute its imagery, trained weights, private datasets, or unlicensed
assets. Record observable techniques and independently implement original
Stanford style assets.

Final releases must include a machine-readable source lock, release manifest,
human-readable notices, and evidence that no unapproved source contributed to
the released pixels.

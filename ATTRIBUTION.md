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

The production baseline may use properly attributed open data such as USDA
NAIP, Santa Clara County LiDAR, OpenStreetMap, Overture Buildings, and Microsoft
US Building Footprints only after the exact release and terms are recorded in
`source.lock.json`.

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

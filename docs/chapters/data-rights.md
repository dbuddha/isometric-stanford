# Data rights and provenance

The production baseline is open data. The prototype source families are 2024
USDA NAIP imagery, approved 2020 Santa Clara County or USGS LiDAR,
OpenStreetMap, Overture 2026-06-17.0 Buildings, and original owner-approved
Stanford overrides. Each exact artifact requires a source-lock record before
use. Overture already incorporates complementary open building sources, so the
prototype does not ingest a separate Microsoft footprint layer.

Source precedence is:

1. Approved authoritative Stanford data
2. OpenStreetMap and Overture
3. LiDAR-derived geometry
4. NAIP semantic predictions
5. Deterministic heuristics
6. Explicit unknown

Google content is not approved. Written permission must explicitly authorize
retrieval, derivative production, storage, and public redistribution before a
Google-derived record can be enabled. General API access is insufficient.

`source.lock.json` records URLs, bounds, dates, licenses, hashes, permissions,
and required notices. Derived manifests retain input hashes. Publication fails
closed if any released output cannot trace to approved sources and original
style assets.

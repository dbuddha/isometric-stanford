# Data rights and provenance

The production baseline is open data. Candidate sources are 2024 USDA NAIP
imagery, 2020 Santa Clara County LiDAR, OpenStreetMap, Overture Buildings,
Microsoft US Building Footprints, and owner-approved Stanford references. Each
exact release requires a Research issue and a source-lock record before use.

Source precedence is:

1. Approved authoritative Stanford data
2. OpenStreetMap and Overture
3. Microsoft building footprints
4. LiDAR-derived geometry
5. NAIP semantic predictions
6. Heuristics
7. Explicit unknown

Google content is not approved. Written permission must explicitly authorize
retrieval, derivative production, storage, and public redistribution before a
Google-derived record can be enabled. General API access is insufficient.

`source.lock.json` records URLs, bounds, dates, licenses, hashes, permissions,
and required notices. Derived manifests retain input hashes. Publication fails
closed if any released output cannot trace to approved sources and original
style assets.

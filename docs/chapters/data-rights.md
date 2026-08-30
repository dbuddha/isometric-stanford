# Data rights and provenance

The semantic and geographic baseline is open data. The prototype source
families are 2024
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

Google Photorealistic 3D Tiles are the owner-authorized registered visual
reference. Dynamic capture is isolated from the generic source synchronizer.
Every retained reference bundle records provider, epoch, camera, renderer,
layer hashes, coverage, and downstream experiment identity.

That owner authorization does not by itself establish a Google contractual
exception. Google's current [Map Tiles API policy](https://developers.google.com/maps/documentation/tile/policies)
limits the default product to visualization and prohibits unauthorized caching,
offline use, image analysis, machine interpretation, object detection, and
derived geodata. The bounded Hoover experiments therefore remain private
technical evidence under the owner's asserted authorization. Campus collection,
computer-vision processing, derived-art publication, and public display remain
permission-gated until a written Google agreement or exception is retained in
the provenance record and reviewed against the exact workflow. An API key and
a Google employee's informal permission are not substitutes for that evidence.

If that permission is not obtained, the production pipeline must replace the
Google reference with licensed open imagery, LiDAR, and vector-derived renders.
No Google-derived geometry, texture, mask, or measurement may silently enter
the open-data world model.

`source.lock.json` records immutable semantic-source URLs, bounds, dates,
licenses, hashes, decisions, and required notices. Reference manifests own
dynamic capture identity. Derived manifests retain both chains. Publication
fails closed if any released output cannot trace to accepted sources, masks,
and original style assets.

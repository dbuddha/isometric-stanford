# Prototype source acquisition record

- Date: 2026-08-17
- Region: `stanford-hero-v1` with the accepted 50 meter guard
- Parent task: P-003, issue #92
- Status: exact artifacts locked pending CI provenance review

## Acquired records

| Record | Exact release or source item | Locked payload |
| --- | --- | ---: |
| OpenStreetMap | Historical query at 2026-07-15T00:00:00Z | 1,386 elements, normalized JSON |
| Overture Buildings | 2026-06-17.0 | 85 clipped features, GeoJSON |
| USDA NAIP | 2024 California, tile `m_3712239_se_10_060_20240520` | 1,326 by 1,168 four-band TIFF |
| USGS 3DEP | CA Santa Clara County 2020 A20 | Four source LAZ tiles |

Every byte length, SHA-256, retrieval or metadata URL, license, acquisition
date, and required attribution is recorded in `source.lock.json`.

## Reproducibility decisions

The OSM historical Overpass response contains server-generated metadata and a
live endpoint may time out. The normalized 681 KB extract is therefore kept in
Git under ODbL rather than treating the query URL as an immutable artifact. The
same rule applies to the clipped 78 KB Overture extract because the official
2026-06-17.0 catalog lookup was unavailable during acquisition. Both files are
small, reviewable source inputs and retain their upstream attribution.

NAIP and LiDAR remain outside Git because their combined size is approximately
448 MB. NAIP is locked to the exact export request and image dimensions. Each
LiDAR record is an exact USGS staged object with a ScienceBase metadata item.
The source synchronizer fails if an upstream object changes length or content.

No Google content was accessed or retained. Raw source bytes are prohibited
from final render output. Only validated semantic geometry and evidence may
cross into the canonical world compiler.

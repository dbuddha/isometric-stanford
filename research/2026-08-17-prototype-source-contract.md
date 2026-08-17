# Stanford hero prototype source contract

- Date: 2026-08-17
- Region: `stanford-hero-v1` plus a 50 meter acquisition guard
- Epoch: 2026-08-17
- Parent task: P-002, issue #91
- Status: accepted acquisition boundary; exact artifact hashes remain P-003 work

## Allowed source roles

| Source | Prototype role | Rights boundary | Final pixels |
| --- | --- | --- | --- |
| OpenStreetMap | Roads, paths, names, land use, and complementary buildings | ODbL with contributor attribution | No |
| Overture 2026-06-17.0 Buildings | Primary building and building-part geometry | ODbL with Overture and upstream attribution | No |
| USDA NAIP 2024 | Land-cover, material, and disagreement evidence | Public-domain federal imagery; exact item metadata must be locked | No |
| Santa Clara County or USGS 2020 LiDAR | Terrain, height, roof, and canopy evidence | Exact item must pass redistribution and attribution review | No |
| Original reviewed overrides | Landmark and conflict correction | CC BY 4.0 project art and data | Procedural geometry only |

No source is approved merely by its family name. P-003 must record the exact
download URL, release or item identifier, acquisition date, geographic bounds,
license, required notice, byte size, and SHA-256 before the compiler accepts it.

## Explicit exclusions

Google Maps, Google satellite imagery, Google Street View, and Google
Photorealistic 3D Tiles cannot supply geometry, textures, measurements, masks,
training data, or validation fixtures. API access and visible browser content do
not authorize extraction, storage, offline processing, or derivative output.

Source imagery and LiDAR may compile semantic evidence but never become final
pixels. People, vehicles, buses, cranes, and temporary equipment are masked
from material evidence and cannot enter the final-world schema.

## Precedence and disagreement

1. Original owner-approved Stanford reference or override
2. OpenStreetMap and Overture geometry
3. LiDAR-derived geometry and height evidence
4. NAIP semantic evidence
5. Deterministic heuristics
6. Explicit unknown

Confidence and contributing source identifiers remain attached to every fused
object. A lower-priority source may fill missing information but cannot silently
replace accepted higher-priority geometry. Temporal or geometric conflicts that
cannot be resolved reproducibly remain unknown.

## Sources

- [OpenStreetMap copyright](https://www.openstreetmap.org/copyright)
- [Overture buildings guide](https://docs.overturemaps.org/guides/buildings/)
- [Overture attribution and licensing](https://docs.overturemaps.org/attribution/)
- [USDA NAIP access](https://naip-usdaonline.hub.arcgis.com/)
- [USGS 3DEP](https://www.usgs.gov/3d-elevation-program)
- [Google Map Tiles policies](https://developers.google.com/maps/documentation/tile/policies)

# Stanford data and perception baseline

- Date: 2026-08-16
- Scope: production rights boundary, candidate open sources, perception aids,
  and determinism limits
- Status: research baseline, not a source approval record

## Question

Can the accepted vertical slice be compiled and published from sources whose
rights allow derivative production and repeatable local processing?

## Findings

Open data provides a credible baseline: USDA NAIP supplies aerial imagery,
Santa Clara County LiDAR supplies elevation and canopy evidence, and OSM,
Overture, and Microsoft footprints supply complementary vector geometry. Exact
releases, dates, hashes, terms, and attribution still require R-002 approval
before bytes enter the pipeline.

Current Google Maps Platform terms and Map Tiles policies impose restrictions
on extraction, storage, rehosting, and content-derived use. The repository
therefore keeps Google content disabled unless written permission expressly
covers the intended workflow.

SatlasPretrain and OpenEarthMap are candidate land-cover baselines. RT-DETR is a
candidate aerial transient detector. Model outputs are not canonical render
inputs until benchmarked, reviewed, frozen, and content-addressed. PyTorch
documents that complete reproducibility is not guaranteed across releases and
platforms, so exact render CI validates accepted compiled artifacts rather than
rerunning perception on every change.

## Sources

- [USDA NAIP access](https://naip-usdaonline.hub.arcgis.com/), imagery source
  and program metadata
- [OpenStreetMap copyright and license](https://www.openstreetmap.org/copyright),
  ODbL and attribution boundary
- [Overture building schema](https://docs.overturemaps.org/schema/reference/buildings/building/),
  building and part attributes
- [Microsoft US Building Footprints](https://github.com/microsoft/USBuildingFootprints),
  California coverage and license record
- [Google Map Tiles policies](https://developers.google.com/maps/documentation/tile/policies),
  current use restrictions
- [Google Maps service terms](https://cloud.google.com/maps-platform/terms/maps-service-terms),
  governing product terms
- [Satlas repository](https://github.com/allenai/satlas), aerial representation
  and task baseline
- [OpenEarthMap repository](https://github.com/bao18/open_earth_map), land-cover
  benchmark and code
- [RT-DETR repository](https://github.com/lyuwenyu/RT-DETR), detector baseline
- [PyTorch reproducibility note](https://docs.pytorch.org/docs/stable/notes/randomness.html),
  limits of cross-release and cross-platform repeatability

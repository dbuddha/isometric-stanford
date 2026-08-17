# World contract fixtures

`representative.json` is an original synthetic CC BY 4.0 contract fixture. It
contains integer polygon geometry with a hole, a parent building and elevated
building part, a road surface, multipolygon canopy, confidence values, source
references, a fixed EPSG:26910 origin, reviewed override notes, and an explicit
unknown conflict. It contains no captured source pixels or transient classes.

Files under `invalid/` are original negative fixtures. Each declares the exact
validation error expected from the fixture checker. They ensure provenance
failures remain fail-closed before the production world reader is implemented.

The fixture source identifiers and attribution strings mirror approved records
in `source.lock.json`; the geometry itself is synthetic and is not a derived
copy of any Stanford or third-party feature.

# Canonical world model

The canonical world is an immutable semantic artifact partitioned by spatial
bounds and version. Coordinates use integer millimeters relative to a declared
local origin. Object identity is stable across ingestion order and localized
rebuilds.

Every object carries permanent class, geometry, height, material evidence,
confidence, source references, review status, and dependency bounds. Conflicts
remain explicit. An unknown region is not silently filled by a plausible
building, tree, or terrain rule.

The implemented Rust model accepts bounded integer polygon and multipolygon
geometry with holes. It rejects open, zero-area, self-intersecting, overlapping,
or excessively large topology before indexing. Objects retain typed roof form,
direction, floors, material, basis-point confidence, sorted source IDs, and
parent relations. Overrides and unresolved conflicts require review notes.
Objects become immutable after validation.

The prototype origin is fixed at easting 573,200,000 mm and northing
4,142,200,000 mm in EPSG:26910. A deterministic 128 meter grid indexes every
object by conservative world bounds. Canonical 2:1 screen bounds are also
derived with integer arithmetic so future tile scheduling does not need to scan
all geometry.

The v1 renderable class set includes terrain, water, roads, paths, athletic
surfaces, empty parking surfaces, buildings, vegetation, and unknown. It has no
person or vehicle type. Qualification rejects unknown objects in hero bounds
and enforces accepted aggregate unknown budgets elsewhere.

Dirty-region propagation begins at changed source hashes and expands through
perception, fused geometry, shadows, guarded supertiles, and affected DZI
ancestors. Total map size must not determine one worker's memory use.

The portable contract fixture in `fixtures/world/representative.json` freezes
the first polygon, hole, multipolygon, building-part, confidence, source, and
unknown examples and is parsed directly by the Rust world model. Its companion
negative fixtures prove that undeclared provenance fails closed.

## Hero vector compilation

`isometric-stanford world compile` validates the complete lock, verifies the
consumed OSM and Overture artifacts, and compiles those vectors. It uses Overture as the
primary building-footprint and height source, enriches matching features with
OSM names, floors, and roof tags, and uses nonduplicated OSM buildings as a
fallback. OSM ways become deterministic roads, paths, parking, water, athletic
surfaces, vegetation, and mapped terrain. Construction-tagged ways are omitted.

WGS84 coordinates are projected into UTM zone 10N and rounded once into local
integer millimeters. Rasterization never receives longitude, latitude, or
floating-point world coordinates. Object IDs use a stable source-identity hash,
not collection position, and compilation order cannot change canonical output.

The current vector-only hero world has 2,820 objects in 72 partitions. A 20
meter review grid marks cells without accepted vector surface evidence as
unknown, producing 387,096 ppm unknown coverage. This is a progress metric, not
a qualification claim. The committed manifest explicitly defers NAIP and four
LiDAR tiles to the later perception and terrain compiler.

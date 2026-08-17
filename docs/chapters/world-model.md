# Canonical world model

The canonical world is an immutable semantic artifact partitioned by spatial
bounds and version. Coordinates use integer millimeters relative to a declared
local origin. Object identity is stable across ingestion order and localized
rebuilds.

Every object carries permanent class, geometry, height, material evidence,
confidence, source references, review status, and dependency bounds. Conflicts
remain explicit. An unknown region is not silently filled by a plausible
building, tree, or terrain rule.

The v1 renderable class set includes terrain, water, roads, paths, athletic
surfaces, empty parking surfaces, buildings, vegetation, and unknown. It has no
person or vehicle type. Qualification rejects unknown objects in hero bounds
and enforces accepted aggregate unknown budgets elsewhere.

Dirty-region propagation begins at changed source hashes and expands through
perception, fused geometry, shadows, guarded supertiles, and affected DZI
ancestors. Total map size must not determine one worker's memory use.

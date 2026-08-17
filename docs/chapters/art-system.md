# Procedural art system

The software tool that converts the world into art is the combination of
`isometric-style` and `isometric-render`. The style pack specifies the camera,
logical pixel scale, indexed palette, face ramps, shadow direction, outline
rules, material patterns, window grammar, tree and terrain grammar, landmark
assets, and stable variation rules. The renderer executes those rules against
semantic geometry.

For a building, the compiler supplies a reviewed footprint, height, roof class,
material evidence, stable ID, and confidence. The style chooses a simplified
massing grammar, roof and facade palette ramps, window cadence, outline width,
and allowed detail density. The renderer projects vertices in fixed point,
clips them, resolves depth, paints indexed faces, adds hard world-space shadows,
and applies world-anchored patterns. The saved tile contains only approved
palette indexes before WebP encoding.

Ordinary structures use reusable grammar. Hoover Tower, Main Quad, Memorial
Church, Green Library, Stanford Stadium, and other hero landmarks require
original silhouette and component definitions. They are code or vector-like
pixel primitives, not copied textures and not manually painted output tiles.

The prototype implements the first three as independently authored parameters
inside `stanford_v1`. Hoover Tower uses a footprint base, narrow shaft, stable
window rows and dark bands, overhanging crown, lantern, and pyramidal cap.
Memorial Church uses its source footprint for the lower shell and adds a
campus-aligned gable, repeated side openings, a portal, and a dark rose-window
mark. The Main Quad renderer recognizes its stable canonical
object, corrects its implausible 22.5 meter source extrusion to a reviewed 12
meter visual mass, and repeats pointed openings along sufficiently large inner
courtyard rings. These are procedural interpretations for visual review, not
claims of survey-grade architectural detail.

Prototype style candidates A, B, and C use the fixed four-scene set: Hoover
Tower, Memorial Church and the Main Quad, roads and empty parking, and dense
canopy with mixed ordinary buildings. The later qualification slice expands to
twelve scenes. Automated metrics constrain drift, but the owner decides whether
the art is compelling enough to continue.

# Procedural art system

The active software that converts the registered Google view into art is the
implemented `isometric-stylize` crate combined with `isometric-style`, accepted
Google-only masks, and bounded procedural overlays from `isometric-render`.
The style pack specifies the logical pixel scale, indexed palette, material
ramps, fixed lighting, outline rules, patterns, detail thresholds, and stable
variation rules. The stylizer simplifies canonical ReferenceAtlas color using
the registered depth and normal layers, semantic and obstruction masks,
repaired surfaces, and original non-geographic assets.

The first implemented `reference-repair-rust/v1` experiment deliberately stops
short of that complete design. Candidate A establishes the RGB-only filter
ceiling. Candidate B adds registered depth and normal guidance, fixed
relighting, and structural outlines. Candidate C keeps Candidate A's
architectural abstraction while replacing only high-confidence canopy with a
controlled seven-band treatment. This progression isolates what each added
source layer contributes. It does not disguise unsupported construction, roof,
window, or facade repair.

For a building, the Google surface graph supplies visible planes, repaired
boundaries, material evidence, stable identity, and confidence. The style
chooses simplified roof and facade palette ramps, window cadence, outline
width, and allowed detail density. The renderer relights repaired surfaces,
paints indexed materials, adds hard globally anchored shadows, and applies
world-anchored patterns. The saved tile contains only approved palette indexes
before WebP encoding.

Ordinary structures use reusable material grammar. Named landmarks receive a
higher detail budget while retaining their registered silhouettes. Procedural
components may correct unreadable details, markings, or masked regions, but
they do not replace the reference geometry or permit manually painted output
tiles.

The historical procedural comparison implemented three independently authored
landmark interpretations inside `stanford_v1`. Hoover Tower uses a footprint
base, narrow shaft, stable
window rows and dark bands, overhanging crown, lantern, and pyramidal cap.
Memorial Church uses its source footprint for the lower shell and adds a
campus-aligned gable, repeated side openings, a portal, and a dark rose-window
mark. The Main Quad renderer recognizes its stable canonical
object, corrects its implausible 22.5 meter source extrusion to a reviewed 12
meter visual mass, and repeats pointed openings along sufficiently large inner
courtyard rings. These are procedural interpretations for visual review, not
claims of survey-grade architectural detail.

Historical style candidates A, B, and C use the fixed four-scene set: Hoover
Tower, Memorial Church and the Main Quad, roads and empty parking, and dense
canopy with mixed ordinary buildings. The later qualification slice expands to
twelve scenes. Automated metrics constrain drift, but the owner decides whether
the art is compelling enough to continue.

Candidate A is implemented as a reproducible review artifact, not an approved
style. It demonstrates the complete deterministic crop, mask, metric, and
contact-sheet workflow. Its measured detail density and tonal variation remain
well below the live reference, while ordinary roofs, facade cadence, canopy
variation, and parking legibility remain visibly incomplete. The versioned
[review record](https://github.com/dbuddha/isometric-stanford/blob/main/research/2026-08-17-style-candidate-a.md)
recommends using
Candidate B to correct those art-system deficiencies without changing the
world, renderer, DZI, or viewer boundaries.

Candidate B implements that bounded correction. Its ordinary grammar adds
stable facade bays, doors, convex hip roofs, four-tone canopy faces, distinct
parking, and independently authored material ramps. Unsupported complex roof
footprints retain the safe flat fallback. Per-object facade and roof caps bound
pathological source complexity. Candidate A remains byte-identical,
which makes the [Candidate B review
record](https://github.com/dbuddha/isometric-stanford/blob/main/research/2026-08-17-style-candidate-b.md)
a controlled comparison rather than a moving baseline.

Candidate C is the final bounded procedural pass. It applies a restrained tile
cadence to every roof material, including the complex planar fallback, varies
ordinary openings from stable object identity, gives hero openings their own
glazing and door materials, and distinguishes roads and paths with sparse
world-anchored accents. Candidates A and B remain byte-identical. The
[Candidate C review
record](https://github.com/dbuddha/isometric-stanford/blob/main/research/2026-08-17-style-candidate-c.md)
contains the final deterministic evidence and the remaining visual risks.

# Style Candidate C review record

Date: 2026-08-17

Status: engineering evidence implemented, owner decision required

## Scope

Candidate C is the final bounded procedural iteration. It preserves the locked
hero world, fixed-point CPU rasterizer, guarded tiles, lossless WebP
publication, OpenSeadragon viewer, no-transients policy, and prohibition on
generated final pixels or manual tile painting.

The independently authored procedural changes are:

- World-anchored diagonal roof-tile cadence on hip and complex planar roofs
- Stable window omission, glazing variation, lintel accents, and door position
  derived from content IDs
- Candidate-specific glazing and door materials for Hoover Tower, Memorial
  Church, and Main Quad openings
- Sparse deterministic road and path accents instead of random texture noise
- A monotonic style detail level that prevents invalid feature combinations
- A 33-color original palette with roof, facade, circulation, and opening ramps

## Candidate B to C comparison

Candidates A and B remained byte-identical after the final renderer extension.
Their encoded contact-sheet SHA-256 values remain
`3c6f880e282778588bace57e48a3a91cbc7ad154569d8ec2933e87808bb58970`
and
`1fb51fa2ed7af9d99da386d75e195414b4e748337c8b411ddaa40fa2cfac8dc6`.

| Scene | B edge transitions | C edge transitions | Change | B used colors | C used colors |
| --- | ---: | ---: | ---: | ---: | ---: |
| Hoover Tower | 45,281 ppm | 103,595 ppm | +128.8% | 10 | 15 |
| Church and Main Quad | 50,311 ppm | 99,199 ppm | +97.2% | 19 | 23 |
| Roads and parking | 40,848 ppm | 91,102 ppm | +123.0% | 13 | 18 |
| Canopy and buildings | 110,660 ppm | 113,994 ppm | +3.0% | 14 | 17 |

Foreground coverage is unchanged from Candidate B. The increase comes from
roof cadence, facade variation, and restrained circulation accents rather than
crop or background changes. Candidate C has indexed contact-sheet hash
`fa044db833563e4b` and encoded contact-sheet SHA-256
`61bd04b672f0df5fe4a3eda7e85c6edebd71fb9aa7c1c7e2da16e95695227a62`.

## Determinism and resource evidence

Two clean release-mode generations had no differing files. On the local probe,
one complete Candidate C review pack completed in 1.35 seconds with 88,539,136
bytes maximum RSS. Guarded Candidate C tiles reconstruct the monolithic scene
exactly, and the style remains within the 128-color contract.

The canonical command is:

```sh
cargo run --release --locked -- style candidate-c artifacts/style/candidate-c
```

The first current-head hosted evidence run is
[GitHub Actions run 32054771613](https://github.com/dbuddha/isometric-stanford/actions/runs/32054771613).
Its immutable
[deterministic render artifact](https://github.com/dbuddha/isometric-stanford/actions/runs/32054771613/artifacts/9296012776)
matches the local encoded hash and preserves the complete Candidate A and B
directories byte for byte.

## Adversarial verdict

Candidate C is the strongest procedural result. Stanford roof character,
landmark openings, ordinary facade rhythm, empty parking, and canopy hierarchy
are clearer than in Candidates A and B. The first circulation experiment was
rejected before freezing because hashed highlights looked like noise; the
accepted pack uses sparse fixed cadence instead.

The result is still not a surveyed digital twin and should not be represented
as one. Large complex roofs remain planar beneath their tile treatment,
ordinary facade variation is grammatical rather than building-specific, and
the vector-only world still reports 387,096 ppm unknown coverage. It is also
less intricate than the live Isometric NYC artwork in close landmark views.

The engineering evidence is fit to merge as the third and final procedural
candidate. Project policy now requires a human decision to approve this
analogue, relax the target, authorize a new sprite-assisted or controlled asset
architecture, or stop. A fourth unapproved procedural iteration would violate
the accepted experiment boundary.

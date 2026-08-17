# Style Candidate B review record

Date: 2026-08-17

Status: engineering evidence implemented, visual review pending

## Scope

Candidate B changes only the recorded Candidate A deficiencies. It preserves
the locked hero world, fixed-point CPU rasterizer, bounded guarded tiles,
lossless WebP publication, OpenSeadragon viewer, no-transients policy, and
prohibition on generated final pixels or manual tile painting.

The independently authored procedural changes are:

- Stable ordinary facade windows and one door per eligible building
- Convex hip-roof planes with flat fallback for complex footprints
- A 27-color palette with additional roof, stucco, glazing, parking, and canopy
  ramps
- Four canopy face tones, closer stable tree spacing, and bounded crown sizes
- Distinct parking material with world-anchored line grammar
- More frequent but still world-anchored terrain variation

## Candidate A to B comparison

The exact Candidate A artifact remained byte-identical after the renderer
extension. Its contact-sheet SHA-256 is
`3c6f880e282778588bace57e48a3a91cbc7ad154569d8ec2933e87808bb58970`.

| Scene | A edge transitions | B edge transitions | Change | A used colors | B used colors |
| --- | ---: | ---: | ---: | ---: | ---: |
| Hoover Tower | 39,186 ppm | 45,281 ppm | +15.6% | 8 | 10 |
| Church and Main Quad | 42,178 ppm | 50,311 ppm | +19.3% | 11 | 19 |
| Roads and parking | 32,156 ppm | 40,848 ppm | +27.0% | 8 | 13 |
| Canopy and buildings | 73,956 ppm | 110,660 ppm | +49.6% | 10 | 14 |

Candidate B preserves essentially the same foreground coverage while adding
detail, which shows that the gain is procedural structure rather than a crop or
background change. Its indexed contact-sheet hash is `1613892db6f12493` and
encoded contact-sheet SHA-256 is
`1fb51fa2ed7af9d99da386d75e195414b4e748337c8b411ddaa40fa2cfac8dc6`.

## Determinism and resource evidence

Two clean release-mode generations had no differing files. On the local probe,
one complete Candidate B review pack completed in 1.14 seconds with 86,917,120
bytes maximum RSS. Guarded Candidate B tiles also reconstruct the monolithic
scene exactly in the test harness.

The canonical command is:

```sh
cargo run --release --locked -- style candidate-b artifacts/style/candidate-b
```

## Adversarial verdict

Candidate B is a substantial and clearly visible improvement over Candidate A.
It is still materially less dense and less architecturally expressive than the
live Isometric NYC analogue. The Main Quad and other complex footprints retain
large flat roof fields, ordinary openings remain repetitive, parking marks are
grammar rather than mapped stalls, and unresolved semantic ground still limits
the scene.

The engineering result is fit to merge as a preserved second candidate. The
recommended visual decision is to reject Candidate B as the final approved
style and reserve Candidate C for roof ridge and tile treatment, more varied
facade composition, finer landmark treatment, and semantic-ground integration.
The browser and publication architecture do not need to change.

# Style Candidate A review record

Date: 2026-08-17

Status: owner review required

## Scope and method

Candidate A is independently authored deterministic output from the locked
vector hero world and `stanford_v1.landmarks.1`. The review pack contains four
native-resolution scenes, a contact sheet, and three isolated landmark masks.
It contains no copied reference image, manually painted tile, photographic
source pixel, or generated final pixel.

The live comparison target was <https://isometric.nyc/>. The reference was
observed in a browser at 1,269 by 534 pixels on 2026-08-17. Its screenshot was
used only for transient visual inspection and derived measurements. It is not
redistributed by this repository. A Stanford viewer observation at 1,228 by
523 pixels used the same measurement script. Browser interpolation means these
screen measurements are comparative signals, not canonical palette metrics.

## Comparative observations

| Screen observation | Isometric NYC | Stanford Candidate A |
| --- | ---: | ---: |
| Exact displayed colors | 140,014 | 191 |
| Displayed color entropy | 15.901 bits | 1.713 bits |
| Edge pixels above threshold 10 | 669,885 ppm | 70,314 ppm |
| Edge pixels above threshold 25 | 488,761 ppm | 69,765 ppm |
| Edge pixels above threshold 50 | 351,885 ppm | 66,976 ppm |
| Dominant displayed color | 2,656 ppm | 717,503 ppm |

The reference contains substantially more tonal variation, architectural
surface treatment, vegetation variation, and small-scale edges. Candidate A
is recognizably Stanford at map scale, but it remains sparse and diagrammatic.
The comparison does not set an exact-copy requirement and does not authorize
reuse of reference assets.

## Canonical Candidate A evidence

- Full indexed master: 7,623 by 3,325 logical pixels
- Full master indexed hash: `0174fa809bacd9cb`
- Approved palette entries available: 16
- Hoover Tower scene: 8 used colors, 39,186 edge transitions per million
- Church and Main Quad scene: 11 used colors, 42,178 edge transitions per million
- Roads and parking scene: 8 used colors, 32,156 edge transitions per million
- Canopy and buildings scene: 10 used colors, 73,956 edge transitions per million
- Contact-sheet indexed hash: `215b8da3d2197672`

The canonical command is:

```sh
cargo run --release --locked -- style candidate-a artifacts/style/candidate-a
```

## Adversarial verdict

Candidate A proves the deterministic review workflow and landmark-mask
boundary. It does not yet meet the intended stylistic analogue. The largest
deficiencies are flat ordinary roofs, missing ordinary facade windows and
doors, a 16-color palette with too little material separation, homogeneous
mapped canopy, unresolved ground semantics, and a roads-and-parking scene whose
parking surface is not visually legible.

The recommended decision is to reject Candidate A as the final style and use
Candidate B for ordinary-building grammar, roof forms, denser canopy, expanded
material ramps, and reduced blank-ground dominance. The source-to-browser
architecture should remain unchanged.

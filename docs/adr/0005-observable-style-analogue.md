# ADR 0005: Observable style analogue contract

- Status: Accepted
- Date: 2026-08-17
- Decision owners: repository owner for visual approval, project architecture
  for implementation boundaries

## Context

Isometric NYC is the preferred visual reference, but its final appearance came
from generated imagery, learned weights, generation order, postprocessing, and
manual review. Those assets and transformations are not a deterministic style
engine that this project can copy.

## Decision

`stanford_v1` targets a close independently authored analogue of observable
properties: a fixed 2:1 orthographic view, crisp logical pixels, a restrained
indexed palette, hard directional light, dense readable detail, simplified
ordinary massing, distinctive landmark silhouettes, chunky vegetation, clean
hardscape, and coherent world-space texture.

The implementation may measure reference screenshots for camera, color-count,
edge-density, and detail-density research. It may not commit or redistribute
those screenshots, compare Stanford output by pixel identity, trace reference
assets, train on Isometric NYC imagery, or copy its tiles, weights, prompts, or
datasets.

The prototype review set is fixed to four native-resolution scenes:

1. Hoover Tower
2. Memorial Church and the Main Quad
3. Roads and empty parking
4. Dense canopy and mixed ordinary buildings

Every candidate includes its palette report, native images, contact sheet,
known deviations, and the same camera. The owner may accept candidate A, B, or
C. A third rejection stops expansion and requires a new owner-approved ADR.
Manual painting of saved tiles and generated final artwork are disqualifying.

## Consequences

Automated checks constrain repeatability and drift, not beauty. Style approval,
hero-landmark acceptance, global camera or palette changes, and any architecture
pivot remain human-owned decisions.

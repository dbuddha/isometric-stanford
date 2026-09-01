---
title: "Deterministic Google reference repair and stylization"
slug: "reference-repair"
status: "active"
research_kind: "algorithm survey and controlled differential experiment"
question: "How far can deterministic Rust image processing move one registered Google view toward intentional pixel art without Qwen, and what must follow?"
decision_owner: "dbuddha"
last_reviewed: "2026-09-01"
review_due: "Before semantic repair implementation or another paid Google capture"
evidence_level: "E3"
upstream_revisions:
  - "cannoneyed/isometric-nyc@008446357ec67512c4329d25edefb6c508c7b24d"
  - "Guided Image Filtering, ECCV 2010"
  - "Image Smoothing via L0 Gradient Minimization, SIGGRAPH Asia 2011"
  - "Rolling Guidance Filter, ECCV 2014"
  - "SNIC, CVPR 2017"
  - "Mask2Former, CVPR 2022"
  - "Segment Anything, ICCV 2023"
related_issues: [178]
related_requirements: [142]
supersedes: []
---

# Deterministic Google reference repair and stylization

## Decision answer

Do not spend on another Google capture and do not introduce Qwen yet. The
existing frozen SSE 8, 125 millimeter Hoover bundle is sufficient to test the
post-capture hypothesis.

Keep safe Rust as the canonical pixel-production boundary. An RGB-only filter
is a useful control but is not a viable final art engine. Registered depth and
normals materially improve structural-edge preservation. A narrow semantic
canopy rule materially reduces Google tree fragmentation. The combination is a
strong deterministic baseline, not a complete Isometric NYC analogue.

The next work should add accepted semantic masks and surface-specific repair in
this order: construction and source artifacts, roof planes and eaves, facade
openings, road and path regions, then markings and small persistent details.
Learned models may propose masks, but accepted mask bytes must be reviewed,
frozen, and hashed before Rust produces final pixels. Passenger cars remain.

## Why the current result stops

The best candidate still knows too little about the scene. It can preserve
geometry edges and replace high-confidence canopy, but it cannot know that a
construction area should be repaired, that a roof edge should be straightened,
or that a facade pattern represents windows without a semantic surface model.
More generic filtering would mainly polish those defects rather than correct
them.

Isometric NYC used Qwen after Google capture to translate raw registered views
into pixel art and used neighboring finished art in its infill templates to
control seams. That approach can invent clean architectural detail but also
creates nondeterministic geometry, palette, and correction work. Stanford's
experiment instead proves exact post-capture hashes and records every
unsupported repair as a blocker.

## Scope boundary

Included:

- One frozen registered Google Hoover core.
- RGB, depth, and normal inputs at one fixed camera.
- Deterministic palette abstraction, relighting, outlines, and canopy repair.
- Objective A/B/C metrics, three-run determinism, memory, and review UX.
- Survey of edge-aware smoothing, structure extraction, superpixels, universal
  segmentation, and prompt-refined masks.

Excluded:

- New Google requests, campus capture, or atlas qualification.
- Qwen or another generative final-pixel model.
- Claims that unreviewed color heuristics are accepted semantic truth.
- Construction, roof, facade, road, marking, or person repair in v1.
- Public redistribution of Google-derived experiment images.

## Navigation

- [Source map](./source-map.md)
- [Findings](./findings.md)
- [Experiments](./experiments.md)
- [Decisions](./decisions.md)
- [References](./references.bib)

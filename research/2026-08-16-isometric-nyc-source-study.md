# Isometric NYC source study

- Date: 2026-08-16
- Scope: architecture, art inputs, production workflow, public delivery, and
  reusable lessons
- Evidence: public project article, public repository at commit
  `008446357ec67512c4329d25edefb6c508c7b24d`, and live-site inspection
- Influence mode: behavioral observation and independent reimplementation only

## Question

Which parts of Isometric NYC created its style and delivery quality, and which
parts can Isometric Stanford reuse without copying imagery or assuming rights?

## Findings

The author rendered Google Photorealistic 3D Tiles with a fixed orthographic
Three.js camera, used generated target examples, fine-tuned Qwen Image-Edit on
roughly forty source and target pairs, then generated contextual quadrants.
Generation state lived in SQLite. Manual workflows addressed seams, color
drift, water, vegetation, terrain, and generation-order failures.

The style was not an OpenCV filter. It was distributed across target images,
prompts, paired training examples, fine-tuned weights, generation order,
preprocessing, postprocessing, and manual acceptance. OpenCV-like operations
conditioned images but did not supply art direction.

The public site is a static DZI/WebP pyramid in OpenSeadragon, not a live 3D
world. This cleanly separates expensive offline art production from a mature,
responsive browser viewer.

## Adopted lessons

- Keep static DZI delivery and OpenSeadragon for version 1.
- Give neighboring context to every saved tile through guarded supertiles.
- Make state, provenance, review, and rejection visible in a dashboard.
- Treat water, trees, terrain, and seams as first-class qualification scenes.

## Rejected patterns

- Generated final artwork and nondeterministic tile regeneration
- Copied Isometric NYC imagery, weights, datasets, or unlicensed assets
- Google-derived production data without express written permission
- Manual painting of saved output tiles

## Sources

- [Cannon Eyed project article](https://cannoneyed.com/projects/isometric-nyc),
  author description of the production process
- [Public source repository](https://github.com/cannoneyed/isometric-nyc),
  implementation evidence and code topology
- [Live Isometric NYC viewer](https://isometric.nyc), delivery inspection

# ADR 0006: Registered reference-derived stylization

## Status

Accepted for the Reference Masking and Stylization Pilot on 2026-08-18.

## Context

Candidates A through C proved deterministic world compilation, bounded guarded
rendering, DZI publication, and browser delivery. They did not prove the visual
hypothesis. Rebuilding Stanford from coarse semantic geometry discarded the
roof, facade, vegetation, and landmark evidence present in the orthographic
Google 3D render used by Isometric NYC.

The source project used Qwen Image-Edit to transform registered renders and
used previously generated neighbors as context for model infill. This project
requires deterministic final art, bounded memory, exact seam evidence, and no
manual output-tile painting.

## Decision

Google Photorealistic 3D Tiles become the primary registered visual reference.
A capture bundle must contain color, whitebox, linear depth, view normals,
fixed shadows, coverage, one orthographic camera, and complete content hashes.
Dynamic capture remains isolated from the generic immutable source
synchronizer.

Pinned perception models may propose semantic and obstruction masks. Accepted
masks become immutable hashed inputs. Safe Rust then performs deterministic
mask fusion, obstruction repair, illumination normalization, material mapping,
relighting, outlines, quantization, and world-anchored patterns. Guarded cells
must match a monolithic oracle at every saved pixel.

Candidate C remains an immutable comparison baseline and procedural-overlay
library. OpenSeadragon and static DZI/WebP remain the browser boundary.

## Consequences

- Architectural fidelity comes from registered source geometry instead of
  hand-authored reconstruction.
- Source capture and model inference are evidence-producing compilation stages,
  not canonical final-art renderers.
- The final output is deterministic only from a frozen reference bundle, mask
  bundle, and style pack.
- Ordinary CI uses synthetic or locked reference fixtures. Live capture is a
  protected smoke and scheduled assurance path.
- Weak filter output blocks expansion. It cannot be hidden with manual tile
  painting or another unreviewed procedural candidate.

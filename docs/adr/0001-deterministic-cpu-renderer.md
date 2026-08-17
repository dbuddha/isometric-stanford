# ADR 0001: Deterministic CPU renderer

- Status: Accepted by the repository bootstrap plan
- Date: 2026-08-16

## Context

Final art must be rerenderable, seam-safe, inspectable, and free from generated
pixel drift. The project needs one canonical byte-producing environment.

## Decision

Use a safe Rust fixed-point CPU renderer for canonical output. Use world-space
patterns, stable object IDs, indexed color, guarded supertiles, and a pinned
Linux environment. Investigate `wgpu` only if profiling shows the qualified CPU
implementation misses the eight-hour full-estate budget.

## Consequences

The project accepts higher up-front rasterizer and art-grammar work in exchange
for reproducibility and targeted updates. GPU acceleration cannot become a
style-specific fork and must remain comparable to CPU semantic oracles.

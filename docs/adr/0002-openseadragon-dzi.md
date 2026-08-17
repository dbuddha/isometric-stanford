# ADR 0002: OpenSeadragon and DZI

- Status: Accepted by the repository bootstrap plan
- Date: 2026-08-16

## Context

The public product is a large static artwork requiring mature pan, zoom, pinch,
tile scheduling, caching, and browser recovery. A custom viewer would own all of
those behaviors before it could improve rendering smoothness.

## Decision

Use a static DZI/WebP pyramid and OpenSeadragon for version 1. Measure initial
bytes, decoded cache, frame rate, INP, LCP, blank frames, tile recovery, and
context recovery on fixed devices.

## Consequences

Rust and WebAssembly are not required in the public viewer. A custom viewer
needs a later accepted decision backed by a specific, reproducible
OpenSeadragon limitation.

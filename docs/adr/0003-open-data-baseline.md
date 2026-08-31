# ADR 0003: Open-data production baseline

- Status: Superseded for the active pipeline by ADR 0008
- Date: 2026-08-16

## Context

The project must publish derivative artwork and retain intermediate artifacts.
Access to a map or API does not automatically permit those uses.

## Decision

Use approved open data as the production baseline and record exact source
terms, hashes, attribution, and transformations. Keep Google content disabled
unless written permission explicitly covers retrieval, derivative production,
storage, and public redistribution.

## Consequences

Open sources may require more fusion and correction work. Provenance failure
blocks publication. A source-rights exception remains human-owned.

## Supersession

On 2026-08-30 the owner selected Google Photorealistic 3D Tiles as the sole
geographic source for the active masking and stylization pipeline. This record
remains the rationale for the historical procedural baseline. See ADR 0008.

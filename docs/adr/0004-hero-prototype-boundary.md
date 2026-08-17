# ADR 0004: Stanford hero prototype boundary

- Status: Accepted
- Date: 2026-08-17
- Decision owners: repository owner and project architecture

## Context

The original 2.8 by 2.0 kilometer qualification slice put source, perception,
renderer, style, landmark, publication, and browser work on one long critical
path. The bootstrap proved repository contracts but did not test whether real
Stanford geometry could become convincing deterministic artwork.

## Decision

The first product gate is `stanford-hero-v1`, a continuous approximately 600 by
600 meter area with a 50 meter source guard:

| Edge | Coordinate |
| --- | ---: |
| West | -122.1722 |
| East | -122.1653 |
| South | 37.4245 |
| North | 37.4299 |

The prototype epoch is 2026-08-17. Every input retains its actual acquisition
date. Source disagreement is represented as confidence or unknown data rather
than silently rewritten to match the epoch.

The area must contain Hoover Tower, Memorial Church, the Main Quad, ordinary
buildings, roads, paths, empty parking, and vegetation. It must produce a
navigable static DZI before the qualification slice resumes.

## Consequences

Prototype fixtures, manifests, visual scenes, performance targets, and Project
work use this region. The original qualification bounds remain a downstream
milestone. Passing the prototype does not qualify the larger slice.

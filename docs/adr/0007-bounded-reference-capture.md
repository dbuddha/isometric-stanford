# ADR 0007: Bounded reference capture processes

## Status

Accepted as an engineering baseline on 2026-08-30.

## Context

The first live Hoover camera probe kept the complete Google scene, browser
protocol session, raw layer bodies, and PNG work in one Node process. Three
1,280-pixel cameras reached 740 to 865 MiB Node peak RSS. The original follow-up
gate assumed that the complete process tree could stay below 768 MiB, but it
did not measure Chromium, its renderer, its GPU process, or the encoder
separately.

Successive bounded measurements isolated the costs:

| Configuration | Node peak | Chromium summed peak | Complete summed peak |
| --- | ---: | ---: | ---: |
| Direct full Chrome with SwiftShader | 188 MiB | 2,378 MiB | 2,653 MiB |
| Direct headless shell with SwiftShader | 180 MiB | 1,046 MiB | 1,343 MiB |
| Direct headless shell with Metal | 177 MiB | 780 MiB | 1,064 MiB |
| Direct shell, Metal, no Playwright runtime, 96 MiB tile cache | 79 MiB | 675 MiB | 849 MiB |
| One 2,560-pixel pilot supertile using the same bounded path | 79 MiB | 810 MiB | 1,014 MiB |

Every final measurement retained 99.99 percent core coverage, exact internal
cell joins, six valid registered layers, and Rust bundle acceptance. The
private source images and reports remain outside Git.

The final 1,280-pixel browser alone reached 675 MiB summed RSS. Coordinator and
credential-free ingest processes added the required isolation and validation
boundary. Reducing the Google tile cache below 96 MiB visibly reduced source
level of detail, so the original 768 MiB tree target was not a quality-neutral
limit.

## Decision

Live capture launches the pinned Playwright Chromium headless shell directly
and uses only small raw Chrome DevTools Protocol WebSockets for target creation
and navigation. It does not retain a Playwright browser, context, page, or
network-routing session.

One token-authenticated loopback coordinator transfers the Google credential
into page memory after navigation. The key never enters a command line,
artifact, child environment, manifest, URL, or diagnostic. A browser-side
request budget is installed before the first Google request.

Raw registered passes stream to a separate credential-free ingest worker. The
worker owns private temporary files, invokes the bounded safe-Rust PNG encoder,
generates crops, validates the complete bundle, and promotes it atomically.
Node never holds a full 2,560-pixel source raster or encoded PNG.

The Google geometry cache uses a 64 MiB retention target and a 96 MiB ceiling.
macOS uses Metal-backed ANGLE. Other supported capture hosts use pinned
SwiftShader unless separately measured. The approved acquisition envelopes
are 1 GiB per 1,280-pixel worker and 1.25 GiB per 2,560-pixel worker. A host
keeps at least 2 GiB and 25 percent of memory outside capture, caps concurrency
at four, and refuses parallel work when no measured worker fits.

## Consequences

- The original 768 MiB complete-tree assumption is replaced by a measured 1
  GiB 1,280-pixel envelope. The Node limit remains 384 MiB and passed with more
  than four times headroom.
- The original 2,560-pixel limits remain unchanged and passed.
- Browser capture remains noncanonical. Exact determinism begins with the
  immutable Rust-validated reference bundle.
- Ordinary CI proves the protocol, WebGL availability, streaming encoders,
  credential isolation, memory policy, and fail-closed behavior without making
  a Google request.
- Future acquisition scheduling must use the versioned memory policy rather
  than a fixed worker count.

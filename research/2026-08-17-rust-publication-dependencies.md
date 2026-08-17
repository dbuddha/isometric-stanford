# Rust publication dependency review

Date: 2026-08-17

## Decision

Pin `image-webp` 0.2.4 for canonical lossless WebP publication. Defer `tiff`
and `las` until their source-ingestion implementations and fixtures are added.

## Accepted dependency

`image-webp` 0.2.4 is a pure-Rust WebP codec licensed MIT OR Apache-2.0 with a
Rust 1.80.1 minimum. The publisher uses only its lossless VP8L path. The lock
also adds `byteorder-lite` 0.1.0 and `quick-error` 2.0.1. Repository
`cargo-deny` policy audits all three transitive records.

Acceptance evidence covers byte-identical repeated encoding, exact decoded RGB
equality with canonical indexed palette colors, corrupt artifact rejection,
and two complete hero publications compared recursively on pinned Linux. No
native library or system encoder affects canonical bytes.

## Deferred dependencies

`tiff` 0.11.3 and `las` 0.11.0 were reviewed as likely NAIP and LiDAR readers.
Adding unused readers now would widen the supply chain without exercising their
feature sets, bounds checks, or malformed-input behavior. Each will receive a
separate dependency review with the ingestion PR that first uses it.

## Consequences

- Canonical and served pyramid bytes are controlled by `Cargo.lock`.
- The publisher does not require libvips at runtime.
- DZI dimensions, 512-pixel tile size, zero overlap, and nearest-neighbor
  indexed reductions are part of the validated prototype contract.
- A codec change requires new deterministic, decoding, size, and browser
  compatibility evidence.

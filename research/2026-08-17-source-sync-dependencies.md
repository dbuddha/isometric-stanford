# Source synchronization dependency review

- Date: 2026-08-17
- Parent task: P-003, issue #92
- Scope: runtime dependencies added by `isometric-source`

## Decision

The source synchronizer uses a small synchronous stack because acquisition is
offline, sequential, and dominated by upstream transfer time. It does not add
an async runtime or a geospatial parser to the renderer.

| Crate | Version | License | Purpose |
| --- | ---: | --- | --- |
| serde | 1.0.229 | MIT OR Apache-2.0 | Typed source-lock decoding |
| serde_json | 1.0.151 | MIT OR Apache-2.0 | JSON source-lock format |
| sha2 | 0.11.0 | MIT OR Apache-2.0 | Streaming SHA-256 verification |
| ureq | 3.4.0 | MIT OR Apache-2.0 | Bounded synchronous HTTPS retrieval |
| webpki-roots data | 1.0.9 | CDLA-Permissive-2.0 | Mozilla-derived TLS trust anchors used by Rustls |

`ureq` disables default gzip behavior so cached hashes describe the exact HTTP
response body. Its Rustls feature supplies TLS without a platform OpenSSL
dependency. Downloads use a 64 KiB heap buffer, enforce the locked length while
streaming, verify SHA-256 before rename, and never allocate in proportion to
artifact size. Connection and response-header waits are bounded. Response-body
time is not globally capped because the locked LiDAR objects are each larger
than 100 MB and valid transfer time varies with upstream throughput.

The TLS root dataset uses the permissive CDLA 2.0. Its license text ships in
the dependency source and permits unrestricted computational results. The
license is added to the repository allowlist because the application uses the
trust anchors for HTTPS validation and does not redistribute a modified root
dataset independently.

The cache is content-addressed by SHA-256. Partial files are process-scoped and
removed on errors. Existing entries are rehashed before reuse. Source retrieval
is not linked into the renderer, and render commands do not access the network.

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
artifact size. Connection, response-header, and response-body waits are bounded
at 30, 60, and 300 seconds. The synchronizer makes at most three
attempts for classified transient conditions. Permanent status responses,
length mismatches, and digest mismatches fail without retry.

The TLS root dataset uses the permissive CDLA 2.0. Its license text ships in
the dependency source and permits unrestricted computational results. The
license is added to the repository allowlist because the application uses the
trust anchors for HTTPS validation and does not redistribute a modified root
dataset independently.

The cache is content-addressed by SHA-256. Partial files are process-scoped and
removed before retries and on every error. Each attempt starts from byte zero;
range resume remains excluded until an immutable upstream range contract is
implemented. Existing entries are rehashed before reuse. Scheduled assurance
uses the official pinned `actions/cache` action with the entire source-lock
digest in its key and no prefix fallback. Source retrieval is not linked into
the renderer, and render commands do not access the network.

The first namespaced cold assurance run on 2026-08-17 exhausted three 30 second
connections to `naip-2024-hero` before any response headers arrived. The stable
source ID and stage were preserved without exposing the export URL. Because the
exact crop is 7.2 MB and redistributable public federal imagery, it is now a
committed licensed fixture. This keeps the retry policy honest and avoids
turning a permanently blocked runner-to-host route into a longer retry loop.

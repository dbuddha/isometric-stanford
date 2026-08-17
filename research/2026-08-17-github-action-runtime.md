# GitHub Action runtime review

- Date: 2026-08-17
- Scope: actions executed by CI, Pages, scheduled assurance, and release dry run
- Parent task: P-001, issue #90

## Finding

The bootstrap workflows referenced major-version aliases whose JavaScript
actions used the deprecated Node.js 20 runtime. GitHub's compatibility bridge
allowed the workflows to pass but did not provide a durable runtime contract.

The accepted replacements use Node.js 24 or composite and Docker entrypoints.
Every action reference is pinned to the reviewed commit rather than a mutable
major-version alias. `peaceiris/actions-mdbook` remains on Node.js 20, so it is
replaced by the composite `taiki-e/install-action` and its checksum-verified
mdBook installer.

## Accepted references

| Action | Accepted release | Runtime |
| --- | --- | --- |
| actions/checkout | v7.0.1 | Node.js 24 |
| actions/setup-python | v7.0.0 | Node.js 24 |
| actions/setup-node | v7.0.0 | Node.js 24 |
| actions/upload-artifact | v7.0.1 | Node.js 24 |
| actions/upload-pages-artifact | v5.0.0 | Composite |
| actions/deploy-pages | v5.0.0 | Node.js 24 |
| taiki-e/install-action | v2.86.1 | Composite |
| Swatinem/rust-cache | v2.9.2 | Node.js 24 |
| EmbarkStudios/cargo-deny-action | v2.1.1 | Docker |
| dtolnay/rust-toolchain | stable commit 4360b52 | Composite |

The complete immutable commit references live in the workflow files and are
checked by repository policy.

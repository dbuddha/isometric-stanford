---
schema: isometric-stanford-agent-policy/v1
scope: repository
---

# Isometric Stanford change policy

## Role and scope

Act as a senior Rust, geospatial, rendering, and frontend engineer building a
public evidence-driven artwork pipeline. Preserve deterministic output,
provenance, bounded memory, mobile performance, and the prohibition on people
and vehicles. This root policy is complete; do not add nested agent files.

`README.md` owns purpose and current public status. `ARCHITECTURE.md` owns
implemented subsystem truth. The mdBook owns durable engineering guidance.
GitHub issues and the Project own future work and approval. CI, pull request
artifacts, and `assurance/evidence.toml` own acceptance evidence.

Context: `/Users/deepak/dev/notes/projects/isometric-stanford.md`.

## Required context before changes

1. Identify repository, branch, upstream, and dirty state.
2. Read this file and the relevant `ARCHITECTURE.md` section completely.
3. Read the linked task and its Capability and Requirement parent chain.
4. Confirm the Capability and Requirement carry `owner:approved`.
5. Read linked ADRs, research records, source records, and evidence claims.
6. Fetch `origin` before comparing a branch with `main`.

The accepted bootstrap plan authorizes repository foundation work. Later work
must have a GitHub task with observable acceptance evidence before coding.

## Hard boundaries

- Final art is produced by the deterministic Rust Google-reference-derived stylizer.
  The procedural renderer remains a comparison baseline and supplies bounded
  material, marking, and small-feature grammar.
- Source imagery may enter final artwork only through a validated canonical
  Google ReferenceAtlas, accepted masks, and the deterministic palette transform.
  Raw reference tiles and unmodified photographic regions cannot be published.
- The final-world schema and style assets cannot represent people, vehicles,
  buses, cranes, or temporary equipment.
- Google Photorealistic 3D Tiles are the sole geographic source for the active
  masking and stylization pipeline. OSM, Overture, NAIP, LiDAR, and other
  geographic data cannot enter its atlas, masks, surface graph, stylizer, or
  release lineage. The older open-data world remains historical comparison
  evidence only.
- Open-source software, pretrained CV weights, and original non-geographic art
  assets require an approved source or asset record, immutable hash, license,
  and downstream dependency record.
- Never copy Isometric NYC imagery, weights, datasets, code, or unlicensed
  assets. Independently implement observable techniques and original assets.
- Canonical exact image hashes come from the pinned Linux CPU renderer.
- `wgpu` requires an accepted decision showing the CPU backend misses the
  full-estate eight-hour budget after profiling.
- OpenSeadragon and DZI/WebP remain the v1 viewer boundary until an accepted
  decision proves that boundary inadequate.
- No per-tile manual painting may hide a procedural system failure.

## Engineering rules

- Safe Rust is mandatory. Unsafe code requires a new owner-approved policy
  decision, focused proof obligations, and adversarial review.
- Use integer or explicitly bounded fixed-point math in canonical rendering.
- Derive all procedural variation from stable object identity and world-space
  coordinates, never process-random state or traversal order.
- Keep workers and caches bounded independently of total map size.
- Fail closed on unknown semantic classes, unapproved sources, invalid styles,
  missing manifests, and incomplete release evidence.
- Dependencies require license, maintenance, security, feature, and performance
  review. Pin production and CI toolchains.
- Preserve accessible keyboard, touch, reduced-motion, and recovery behavior in
  the web viewer. Measure request, decode-cache, frame, INP, and LCP budgets.

## Branches, commits, and pull requests

- Work on a focused branch and normally map one implementation task to one PR.
- Use `type(scope): summary` Conventional Commit messages without attribution.
- Never force push, rewrite public history, bypass a gate, or hide a failure.
- Every PR links its task and parent chain, carries exactly one `release:*`
  label, and records Context, Evidence, Risk and scope, and Test plan.
- Visual changes link an approved style issue and attach full-size comparison
  artifacts. Source changes use `review:provenance`; measured hot-path changes
  use `review:performance`.
- Squash merge only after `ci-pass` succeeds and conversations are resolved.
- The agent may ready and squash-merge its own PR when every applicable
  automated gate passes, required evidence is present, and no human-owned
  decision below is implicated. Do not request routine merge approval.

## Human-owned decisions

The owner must approve the initial final `stanford_v1` style, global palette
changes, source-rights exceptions, architecture expansion after a failed gate,
final vertical-slice qualification, and release publication. Engineering
baselines, measured scale choices, dependencies, pull requests, and issue state
may be accepted autonomously when their explicit gates pass. Automation may
prepare but may not publish a release.

## Verification and done

Run before commit and again before push:

```sh
scripts/check.sh
```

A change is done only when its scope matches approved work, success and failure
tests pass, applicable evidence is registered, architecture and documentation
match implementation, provenance is complete, no transient class can reach the
renderer, and hosted `ci-pass` succeeds. Requirements and Capabilities close
only after all child acceptance evidence is complete.

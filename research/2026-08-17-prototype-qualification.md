# Prototype qualification evidence

- Date: 2026-08-17
- Parent task: P-018, issue #107
- Region: `stanford-hero-v1`
- Status: engineering preview assembled, prototype not qualified

## Verdict

The repository can now produce an inspectable source-to-browser Candidate C
preview from the current semantic world. The deterministic compilation,
bounded CPU rendering, lossless DZI publication, artifact-chain validation,
and OpenSeadragon delivery hypotheses are supported by evidence. The visual
hypothesis is not yet accepted. Hoover Tower, Memorial Church, the Main Quad,
and the overall similarity target still require the human-owned decisions in
issues #103 and #18. No public map release has been published.

The correct next decision is visual, not architectural. If Candidate C is
accepted, record the style and landmark approvals and perform fixed-device
qualification before publication. If it is rejected, preserve this working
engineering path and open the required ADR to choose a sprite-assisted or
otherwise revised art architecture. Do not add a fourth unbounded procedural
iteration and do not hide the gap with manual tile painting.

## Reproduced artifact

The verified local preview contains:

| Measure | Result |
| --- | ---: |
| Semantic objects | 2,820 |
| Spatial partitions | 72 |
| Unknown coverage | 5,202 ppm, or 0.5202 percent |
| DZI dimensions | 7,623 by 3,325 pixels |
| WebP tiles | 157 |
| Encoded tile bytes | 4,324,252 |
| Tile-set SHA-256 | `1f0261eb5141a4a37bc43f072aa29e839bd5c35724766b4a71b15a5d5752cd41` |
| Style | `stanford_v1.candidate_c.1` |
| Renderer peak RSS | 22.4 MiB |
| DZI publication time | at most 1.06 seconds |
| Accepted maximum-level throughput | 5,946 tiles per minute |

The assembler verifies the current world hash, exact unqualified style,
descriptor hash, tile count, byte count, and every WebP hash. It rejects a
viewer build that already contains art, which closes the stale ignored-preview
failure discovered during this audit. The earlier stale directory was moved to
recoverable temporary storage and was not published.

## Browser evidence

The assembled preview was exercised in Chromium at desktop and 390 by 844
mobile viewports. Pan and zoom controls, the full-map Home view, Canvas output,
release metadata, source link, and unqualified disclosure were present. The
desktop run measured 264 millisecond FCP and LCP and about 9.5 MiB used
JavaScript heap. The mobile run measured 152 millisecond FCP, 168 millisecond
LCP, and about 11.1 MiB used heap on localhost. Automated WCAG A and AA
inspection reported no violations and one inconclusive glyph contrast check
for the zoom-out button.

These browser values are integration evidence only. They do not replace the
required iPhone 12-class and Pixel 7-class physical-device measurements. The
first mobile run exposed a 0.13 CLS caused by asynchronous release-banner
insertion. The implementation now reserves that layout row before metadata
loads and includes regression coverage.
The repeated mobile run reported 0.00 cumulative layout shift.

## Gate matrix

| Gate | State | Evidence or gap |
| --- | --- | --- |
| Current source-to-browser preview | Pass | Verified portable preview bundle |
| Three canonical render hashes | Pass | Scheduled assurance and performance harness |
| Exact guarded-tile seams | Pass | Monolithic reconstruction oracle |
| Palette-only output | Pass | Release decoder and palette validator |
| No people or vehicles | Pass structurally | Schema, source-mask, and asset policy |
| Unknown coverage below 2 percent | Pass | 0.5202 percent |
| Peak RSS at most 512 MiB | Pass | 22.4 MiB |
| Hero render within 20 minutes | Pass | DZI publication at most 1.06 seconds |
| At least 100 accepted tiles per minute | Pass | 5,946 per minute |
| Mobile initial imagery at most 2.5 MiB | Pass hosted | 1,258,408 bytes in Pixel 7 profile |
| Cache budgets | Pass by policy | 48 mobile and 128 desktop decoded tiles |
| Recognizable hero landmarks | Pending | Human issue #103 |
| Near-copy style approval | Pending | Human issue #18 |
| Physical mobile frame and memory gates | Not run | Fixed devices unavailable in hosted evidence |
| Representative 4G LCP | Not run | Requires controlled network and device run |
| Public release | Not authorized | Human-owned after all qualification gates |

## Difference from Isometric NYC

Both projects flatten a large artwork into 512 pixel WebP tiles and use
OpenSeadragon for delivery. Isometric NYC's source at commit
`008446357ec67512c4329d25edefb6c508c7b24d` uses a custom tile-level mapping,
object storage, a Cloudflare cache worker, and optional WebGL water treatment.
Its artwork came from Google 3D white boxes, trained Qwen editing, contextual
infill, postprocessing, and substantial manual review and repair.

Stanford uses standard DZI levels, same-origin prototype hosting, explicit
cache limits, a semantic vector world, and deterministic procedural final
pixels. This is more reproducible and easier to validate, but Candidate C is
visually flatter and less individually detailed. Isometric NYC supplies no
procedural art implementation that can be ported to solve that difference.
OpenSeadragon remains the right v1 viewer because no measured viewer bottleneck
exists. Object storage and immutable caching should wait until a versioned URL
scheme is introduced for campus-scale artifacts.

# Findings

| ID | Bounded finding | Class and level | Support and contradiction | Project impact | Confidence |
| --- | --- | --- | --- | --- | --- |
| F-001 | Google returns a root OGC 3D Tiles JSON hierarchy and visible child JSON plus GLB payloads. It does not return a ready isometric image. | Fact, E1 | S-001 to S-003 | A renderer and explicit camera are mandatory. | High |
| F-002 | Among the three measured Hoover cameras, 330 degree azimuth and 42 degree elevation gave the best balanced landmark and campus-axis view at comparable readiness and coverage. | Result, E3 | Prior camera probe, PR #168; S-006 uses 345/45 as a useful comparator | Freeze 330/42 for the masking pilot. A later owner visual decision may change it globally. | Medium-high |
| F-003 | Orthographic apparent scale is controlled by frustum span, not the 2,000 meter camera distance. | Fact plus reproduced implementation, E2 | Three camera mathematics, S-006, project camera tests | Keep 250 mm/source pixel and use distance only for near/far safety. | High |
| F-004 | Translating or reconstructing the camera for each neighbor changed dynamic LOD enough to make registered source layers unusable: depth 147,698 ppm, normal 171,539 ppm, and whitebox 173,007 ppm exceeded tolerance in the independent overlap. | Result, E3 | E-001; screen-space LOD in S-007 explains the direction of effect | Never recenter the camera inside one registered macroblock. | High |
| F-005 | A fixed world matrix plus off-axis neighboring frusta reduced the independent 64-pixel seam corridor to color 30 ppm, coverage 0 ppm, depth 61 ppm, and normals 91 ppm, all within their bounded source gates. | Result, E3 | E-002 and E-003 | Two independently owned source cores can join without visible chopped Hoover geometry. Local deterministic processing with at most a 32-pixel dependency radius can use the measured corridor. | High for this scene |
| F-006 | A separately traversed monolithic view is not pixel-equivalent to the joined views: its seam comparison measured depth 854 ppm and normals 2,822 ppm. | Result, E3 | E-003 | Do not call the complete monolithic gate qualified. Prefer bounded macroblock capture and explicit core ownership while investigating fixed reusable tile selection. | High |
| F-007 | The captured shadow and old shadowed whitebox were not registered across view sizes. The independent seam measured 62,057 ppm for fixed shadow and 100,143 ppm for whitebox. | Result, E3 | E-002 and E-003 | Structural whitebox must be unshadowed. Final art lighting belongs to deterministic Rust. A block-anchored capture shadow remains diagnostic until retested. | High |
| F-008 | The fixed run completed 428 of 428 requests under a 450 ceiling, with 395 GLB and 33 JSON responses, 99.99 percent core coverage, and one billable root session. | Result, E3 | E-002 | One session is sufficient for this three-view test. Keep fail-closed ceilings and reuse the root session. | High |
| F-009 | The fixed run peaked at 1,254,883,328 bytes complete-tree RSS and 226,785,670 bytes renderer cache, inside the 1.25 GiB worker envelope only by about 87 MB. | Result, E3 | E-002; S-008 contradicts the former small hard-cache assumption | Use 128 MiB retention and 256 MiB ceiling. Admit conservatively and do not run four such workers merely because the 24 GiB host can fit them arithmetically. | High |
| F-010 | TypeScript plus Three.js is the lowest-risk Google streaming boundary. Rust is the right canonical post-capture boundary. | Inference, E2 | S-001, S-006 to S-010, E-001 and E-002 | Do not rebuild Google 3D Tiles traversal in Rust. Do not move canonical masks or art transforms into browser GPU code. | High |
| F-011 | deck.gl is strongest for interactive data overlays and adaptive memory. CesiumJS is strongest for a mature globe product. `3d-tiles-renderer` is the smallest direct fit for an exact offscreen multipass exporter. | Inference, E2 | S-007 to S-010 | Retain `3d-tiles-renderer`; borrow overlay and adaptive-memory ideas, not their whole frameworks. | Medium-high |
| F-012 | Isometric NYC performs infill after Google capture, as part of Qwen generation using neighboring finished art and raw Google regions. It does not use infill to repair or stitch the Google source raster. | Fact, E2 | S-005 and S-006 | Stanford source registration must be solved before art. Qwen's ordered neighbor dependency is unnecessary for deterministic source capture. | High |
| F-013 | This experiment consumed two root sessions, 0.02 percent of the official 10,000/day default and 0.2 percent of the current 1,000-event monthly free cap. | Result over official limits, E2 | S-003, S-004, E-001, E-002 | Stop live experimentation after two sessions and preserve remaining quota. Account-wide usage may be higher. | High |
| F-014 | Direct-browser `responseBodyBytes` only sums CORS-visible `Content-Length` headers. The fixed report's 27,988 bytes are a lower bound, not transfer volume. | Fact and result, E3 | Browser instrumentation plus E-002 | Do not use it for bandwidth or cost conclusions. Use request counts, cache residency, and process memory until streamed byte accounting exists. | High |

## Threats to validity

- The seam experiment covers one sunny Stanford scene and one horizontal
  boundary. It does not qualify vertical boundaries, hills, water, or a long
  macroblock chain.
- E-002 was captured before unshadowed whitebox and block-anchored shadow code
  existed. Those remediations are implemented and synthetic-tested, not live
  reproduced.
- Google content and LOD can change between sessions. Frozen input hashes make
  later Rust output reproducible, but cannot make live acquisition immutable.
- Browser RSS is sampled every 250 milliseconds and can miss shorter peaks.
- The camera choice is an engineering baseline from three samples, not final
  art-style approval.
- Visual inspection found no chopped tower, but the full issue contract still
  requires the monolithic and lighting relations that failed.

# Findings

| ID | Bounded finding | Class and level | Support | Project impact | Confidence |
| --- | --- | --- | --- | --- | --- |
| F-001 | Isometric NYC applies Qwen after Google capture. Its infill templates mix raw Google regions with neighboring finished art so generation sees the visual context it must match. | Source fact, E2 | S-001, S-002 | Do not mistake its infill for source-raster repair or Google seam assembly. | High |
| F-002 | An RGB-only deterministic filter preserves recognizable layout but also preserves source defects and reaches only 83.33 percent accepted structural-edge recall. | Result, E3 | E-001, E-002 | Retain Candidate A only as the filter ceiling. | High for this scene |
| F-003 | Depth and normal guidance raise accepted structural-edge recall from 83.33 to 92.38 percent, but the geometry-only candidate raises canopy fragmentation from 164,645 to 192,837 ppm. | Result, E3 | E-002 | Geometry guidance is necessary but insufficient. Apply it by semantic surface class. | High for this scene |
| F-004 | Narrow high-confidence canopy replacement reaches 97.30 percent structural-edge recall and lowers canopy interior edge density to 104,147 ppm, a 36.7 percent reduction from Candidate A. | Result, E3 | E-002 | Selective semantic repair materially changes the trajectory. | High for this scene |
| F-005 | Three clean runs reproduce every PNG and report hash exactly. | Result, E3 | E-002 | Deterministic Rust post-capture output is proven for this bounded algorithm. | High |
| F-006 | Actual complete-process RSS is about 94 MiB and the transform completes in about 1.2 seconds on the measured host. | Result, E3 | E-002 | The current CPU path is comfortably fast enough for further algorithm work. | High for this host |
| F-007 | Candidate C remains visibly below the target because construction, roofs, windows, facade cadence, paths, and markings lack accepted semantic repair. | Result plus visual review, E3 | E-001, E-002, S-001 | Do not expand capture or claim style qualification. | High |
| F-008 | Guided and rolling-guidance filters can simplify appearance while preserving major edges, but neither supplies material or object identity. | Literature fact, E1 | S-003, S-005 | Use these ideas only inside a known semantic material region. | High |
| F-009 | L0 gradient smoothing and superpixels are useful controlled baselines for structure and regions, but they cannot decide what a roof, tree, or construction defect is. | Literature fact and inference, E1 | S-004, S-006 | Benchmark them on accepted masks, not as a replacement for masks. | High |
| F-010 | Mask2Former and SAM-style tooling can propose dense or prompted masks, but accepted Stanford labels and domain evaluation are required before they become production evidence. | Literature fact and inference, E1 | S-007 to S-009 | Freeze reviewed mask bytes before deterministic Rust styling. | High |
| F-011 | Passenger cars do not need obstruction repair under the clarified objective and can remain registered persistent detail. | Owner decision plus implementation, E3 | Issue #178, E-002 | Remove cars from transient rejection and assert their preservation. | High |
| F-012 | The review cockpit itself is part of correctness. Browser dogfooding found reversed wipe semantics and overlapping mobile evidence labels that compile-time tests had missed. | Result, E3 | Browser evidence under the private run | Keep screenshot, interaction, bounding-box, and corrupt-hash checks in CI. | High |

## Threats to validity

- The experiment covers one Hoover view, not the Main Quad, roads, long seams,
  water, or varied source epochs.
- The canopy mask is a conservative deterministic color, normal, and geometry
  heuristic. It is not an accepted Stanford segmentation benchmark.
- Structural-edge recall measures agreement with depth and normal evidence. It
  is not a direct measure of beauty or architectural truth.
- Lower canopy edge density is useful only within an accepted canopy mask. It
  can reward oversmoothing if used without visual review.
- Exact output hashes begin after the frozen Google bundle. Live Google content
  can change between sessions.
- The owner has not approved Candidate C as the target art style.

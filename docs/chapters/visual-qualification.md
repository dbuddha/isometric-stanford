# Visual qualification

The prototype scene set covers Hoover Tower, Memorial Church and the Main Quad,
roads and empty parking, and dense canopy with mixed ordinary buildings. The
later qualification set adds Green Library, Stanford Stadium, athletic
surfaces, sparse landscaped trees, Lake Lagunita shoreline, and dry terrain.

Every candidate produces native-resolution scene images, a full contact sheet,
palette report, silhouette masks, edge-frequency and detail-density metrics,
seam evidence, and a list of known deviations. Automated tests enforce the
style contract but cannot certify beauty or similarity.

The owner approves the initial style, hero silhouettes, global palette or
camera changes, and the final qualification. Three deliberate style iterations
are allowed. A failed third review triggers a continue, pivot, or stop decision.
Manual painting of individual saved tiles is disqualifying.

Candidate A is the first implementation of this contract. CI regenerates its
four scenes, contact sheet, landmark masks, metrics, and deviations from locked
inputs and uploads them as review evidence. Its engineering evidence is
implemented, while style and landmark approval remain explicitly pending. The
current project recommendation is to reject A as the final style and proceed to
the bounded Candidate B iteration.

Candidate B improves fixed-scene edge density by 15 to 50 percent, expands the
declared palette from 16 to 27 colors, and remains byte-deterministic under
clean reruns. The review pack completed under 87 MB peak RSS in the local
probe. These metrics establish a real improvement but do not establish final
artistic acceptance. The current recommendation is to preserve B as evidence
and use the final bounded Candidate C iteration for the remaining density and
architectural-expression gap.

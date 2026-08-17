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
project rejected A as the final style and proceeded to the bounded Candidate B
iteration.

Candidate B improves fixed-scene edge density by 15 to 50 percent, expands the
declared palette from 16 to 27 colors, and remains byte-deterministic under
clean reruns. The review pack completed under 87 MB peak RSS in the local
probe. These metrics establish a real improvement but do not establish final
artistic acceptance. The project preserved B as rejected final-style evidence
and used the bounded Candidate C iteration for the remaining density and
architectural-expression gap.

Candidate C raises edge-transition density to 103,595 ppm for Hoover Tower,
99,199 ppm for the Church and Main Quad, 91,102 ppm for roads and parking, and
113,994 ppm for canopy and ordinary buildings. Its 33-color pack, exact rerun,
unchanged earlier candidates, and bounded resource result satisfy the
engineering side of the third-iteration contract. These measurements do not
approve the art. The next action is the required human approve, relax, pivot,
or stop decision, not a fourth silent procedural iteration.

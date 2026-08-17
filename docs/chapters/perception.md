# Perception system

Perception is an offline compilation stage. Its accepted outputs are frozen,
hashed, reviewed semantic artifacts. Ordinary render CI consumes the small
artifact and never downloads raw imagery, point clouds, or model weights.

The implemented prototype path is model-free safe Rust. It decodes the exact
four-band NAIP GeoTIFF, streams four exact USGS LAZ files in reusable
250,000-point chunks, transforms EPSG:2227 into EPSG:26910 with an audited
control point, and compiles only cells not already owned by persistent vector
semantics. Source pixels and point records do not enter the artifact.

NAIP NDVI supports grass and dry-ground decisions. ASPRS class 4 and 5 returns
support bounded canopy height. A cell dominated by building returns remains
unknown rather than receiving invented geometry. Unclassified low elevated
returns are counted and excluded as conservative transient candidates. The
frozen result resolves 367 of 372 vector-unknown cells and reduces whole-grid
unknown coverage to 5,202 ppm.

The later full-estate research baseline compares SatlasPretrain or OpenEarthMap
for land cover,
RT-DETR adapted for aerial vehicles and construction equipment, LiDAR-derived
terrain and canopy, and temporal or source disagreement for construction.
SAM-style segmentation may help review only after its license and operating
cost are accepted.

A future 150-patch Stanford benchmark covers buildings, roads, paths, water,
canopy,
fields, parking, dry terrain, construction, and transient objects. Model and
threshold changes require the full benchmark, artifact hashes, runtime and GPU
records, error slices, and reviewer approval.

H100 execution is reserved for a learned-model experiment that first proves a
measured accuracy gap in the model-free compiler. The current complete source
recompile takes approximately 38 seconds and 36.4 MB maximum RSS on the
development machine, so a GPU is neither necessary nor part of canonical
output. Weekly assurance recompiles the locked sources twice and requires exact
artifact bytes.

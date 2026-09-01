# Perception system

Active perception is an offline Google-reference compilation stage. Its
accepted outputs are frozen,
hashed, reviewed semantic artifacts. Ordinary render CI consumes small locked
artifacts and never downloads live reference layers, raw point clouds, or model
weights.

The implemented open-data semantic-world path is retained only as historical
comparison evidence. It decodes the exact
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

The active reference-mask pilot uses the canonical Google ReferenceAtlas.
SegFormer proposes dense surfaces, RT-DETRv2 proposes transient and thin-object
instances, and SAM refines accepted boxes into masks. Models do not determine
final pixels. Rust fuses their frozen outputs with Google depth and normals,
deterministic edges, and reviewed corrections. It does not consult geographic
priors from OSM, Overture, NAIP, LiDAR, or another map source.

Boundary extraction does not depend on one object detector. Integer Scharr
responses locate depth discontinuities, and squared encoded-normal differences
locate orientation changes without trigonometry. Stable Canny-style
hysteresis retains weak edges connected to strong seeds. Model interiors and
Google-derived planar regions provide watershed markers, while morphology
removes bounded noise and connected components assign stable row-major
identities. Chamfer
distance supplies deterministic proximity evidence. Quantized horizontal,
vertical, and 45-degree line runs provide evidence for road markings and thin
architectural divisions. These kernels propose structural evidence; later
fusion still decides the semantic class and records its confidence and source
flags.

Local kernels declare a finite pixel radius. They may run on a guarded cell
only when the guard is at least the complete composed radius. Hysteresis,
connected components, and watershed can propagate across an arbitrary number
of pixels, so they run once on the complete registered supertile before mask
cells are cut. This is the semantic equivalent of the render seam contract.

The implemented `isometric-mask` boundary defines 24 stable classes, including
persistent surfaces, scale-gated infrastructure, removable obstructions,
unknown regions, and broken-source evidence. Each eight-byte pixel record
contains a class, quantized confidence, evidence flags, and an optional positive
instance identity. Every artifact pins the exact registered reference-manifest
digest and grid digest. Its manifest records all class counts, unresolved and
transient totals, instance bounds, producer identity, encoding, byte length,
and content hash.

There are three explicit artifact roles. `evidence` and `repair-input` may
contain people, bicycles, buses, trucks, construction equipment, and source
artifacts because downstream repair must know where they are. Passenger cars
are persistent-compatible and are preserved by default. `persistent` output
rejects every removable transient and source-artifact class. Unknown pixels remain
representable so later qualification can fail honestly rather than inventing a
surface. The validator streams records through a 64 KiB reader and uses a
fixed-size instance table, so memory does not scale with mask raster bytes.

Model floating-point output does not need byte identity. Once accepted model
output is encoded as a frozen mask artifact, its bytes and manifest are hashed.
All later Rust fusion, repair, styling, and stitching stages consume that exact
immutable input.

OpenCV 5.0 is a pinned research oracle for small Scharr, square-morphology, and
connected-component fixtures. It is not a production dependency. Ordinary CI
compares the safe-Rust output with the committed oracle bytes and lints the
generator. Updating the oracle requires an explicit review of its inputs,
versions, and the Rust contract.

A future Google-capture Stanford benchmark covers buildings, roads, paths, water,
canopy,
fields, parking, dry terrain, construction, and transient objects. Model and
threshold changes require the full benchmark, artifact hashes, runtime and GPU
records, error slices, and reviewer approval.

H100 execution is reserved for a learned-model experiment that first proves a
measured accuracy gap on accepted Google-capture fixtures. A GPU is neither
necessary nor part of canonical final-pixel generation. Weekly assurance
replays frozen masks through the Rust boundary and requires exact artifact
bytes.

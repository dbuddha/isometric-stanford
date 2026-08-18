# Perception system

Perception is an offline compilation stage. Its accepted outputs are frozen,
hashed, reviewed semantic artifacts. Ordinary render CI consumes small locked
artifacts and never downloads live reference layers, raw point clouds, or model
weights.

The implemented semantic-world path is model-free safe Rust. It decodes the exact
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

The active reference-mask pilot adds a separate pixel-space perception path.
SegFormer proposes dense surfaces, RT-DETRv2 proposes transient and thin-object
instances, and SAM refines accepted boxes into masks. Models do not determine
final pixels. Rust fuses their frozen outputs with depth, normals, projected
vectors, deterministic edges, and reviewed corrections.

The implemented `isometric-mask` boundary defines 24 stable classes, including
persistent surfaces, scale-gated infrastructure, removable obstructions,
unknown regions, and broken-source evidence. Each eight-byte pixel record
contains a class, quantized confidence, evidence flags, and an optional positive
instance identity. Every artifact pins the exact registered reference-manifest
digest and grid digest. Its manifest records all class counts, unresolved and
transient totals, instance bounds, producer identity, encoding, byte length,
and content hash.

There are three explicit artifact roles. `evidence` and `repair-input` may
contain cars, people, bicycles, buses, trucks, construction equipment, and
source artifacts because downstream repair must know where they are.
`persistent` output rejects all of those classes. Unknown pixels remain
representable so later qualification can fail honestly rather than inventing a
surface. The validator streams records through a 64 KiB reader and uses a
fixed-size instance table, so memory does not scale with mask raster bytes.

Model floating-point output does not need byte identity. Once accepted model
output is encoded as a frozen mask artifact, its bytes and manifest are hashed.
All later Rust fusion, repair, styling, and stitching stages consume that exact
immutable input.

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

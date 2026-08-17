# Perception system

Perception is an offline compilation stage. Its accepted outputs are frozen,
hashed, reviewed semantic artifacts. Ordinary render CI never reruns a model.

The research baseline compares SatlasPretrain or OpenEarthMap for land cover,
RT-DETR adapted for aerial vehicles and construction equipment, LiDAR-derived
terrain and canopy, and temporal or source disagreement for construction.
SAM-style segmentation may help review only after its license and operating
cost are accepted.

A 150-patch Stanford benchmark covers buildings, roads, paths, water, canopy,
fields, parking, dry terrain, construction, and transient objects. Model and
threshold changes require the full benchmark, artifact hashes, runtime and GPU
records, error slices, and reviewer approval.

H100 execution is reserved for perception experiments or a changed perception
artifact. It does not block an ordinary renderer or web pull request.

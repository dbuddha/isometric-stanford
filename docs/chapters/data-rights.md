# Data rights and provenance

Google Photorealistic 3D Tiles are the sole geographic source for the active
masking and stylization pipeline. Registered Google captures supply visible
geometry, texture, object placement, terrain, buildings, roads, and vegetation.
OSM, Overture, NAIP, LiDAR, and other geographic datasets cannot enter the
ReferenceAtlas, semantic masks, surface graph, stylizer, or release lineage.

Open-source libraries, pretrained CV weights, and original non-geographic art
assets may process the registered atlas. Each dependency, model weight, and art
asset requires an immutable identity, license, provenance record, and downstream
dependency record. Qwen and other image generators do not produce final pixels.

The repository records this boundary in `reference-policy.json`. CI validates
that active production crates do not depend on the historical open-data source,
perception, or world compilers. `source.lock.json` and its derived artifacts are
retained only to reproduce the rejected procedural comparison baseline. They
are not active geographic inputs.

Every registered Google bundle records:

- provider and source epoch;
- fixed camera and world-grid cell;
- renderer and version;
- attribution;
- coverage;
- exact layer sizes and hashes;
- acquisition experiment identity.

The canonical ReferenceAtlas additionally records the provider session, root
tileset hash, validated source manifests, deterministic per-pixel ownership,
canonical layer-tile hashes, and the complete ownership-map hash. Downstream
artifacts retain that atlas digest rather than independently selecting raw
capture pixels.

The owner has asserted permission for the private internal processing workflow.
Google's published [Map Tiles API policy](https://developers.google.com/maps/documentation/tile/policies)
otherwise limits the default product to visualization and restricts caching,
offline use, image analysis, machine interpretation, object detection, and
derived geodata. Public transformed-art publication therefore remains blocked
until permission for the exact publication workflow is retained in the project
provenance record. An API key is a credential, not the rights record.

Raw Google captures, atlas tiles, masks derived from them, and private review
screenshots remain outside Git and public CI. Public CI uses original synthetic
registered fixtures. Publication fails closed if a released pixel cannot trace
through an accepted atlas, masks, deterministic style, and original assets.

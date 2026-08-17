# No-transients policy

Final art depicts the persistent campus, not a captured moment. People, cars,
buses, cranes, temporary equipment, and construction clutter are absent.

This is enforced structurally:

- The final semantic enum cannot represent transient classes.
- Style validation rejects transient sprite or asset names.
- Perception masks every vector-owned cell before raster material inference.
- Unclassified LiDAR returns 0.5 to 4 meters above cell ground are discarded
  from persistent-class evidence and retained only as an aggregate QA counter.
- Parking and road surfaces render as intentional empty hardscape.
- Release review includes road, parking, and construction fixtures.

An output detector may warn about a stylized shape but is not authoritative.
The authoritative evidence is the unrepresentable world contract, asset policy,
source sampling masks, and human audit. A violation blocks publication and is
fixed in compilation or grammar, never painted away in final pixels.

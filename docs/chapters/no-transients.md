# No-transients policy

Final art depicts the persistent campus, not a captured moment. People, cars,
buses, cranes, temporary equipment, and construction clutter are absent.

This is enforced structurally:

- The final semantic enum cannot represent transient classes.
- Intermediate reference masks may identify transient classes only so the
  obstruction-repair stage can remove them. They cannot become a material,
  procedural asset, or final semantic object.
- Style validation rejects transient sprite or asset names.
- Reference perception computes obstruction masks on complete supertiles
  before art-cell slicing so an object crossing a boundary receives one mask.
- Unclassified LiDAR returns 0.5 to 4 meters above cell ground are discarded
  from persistent-class evidence and retained only as an aggregate QA counter.
- Parking and road surfaces render as intentional empty hardscape.
- Release review includes road, parking, and construction fixtures.

An output detector may warn about a stylized shape but is not authoritative.
The authoritative evidence is the unrepresentable world contract, asset policy,
source sampling masks, and human audit. A violation blocks publication and is
fixed in compilation or grammar, never painted away in final pixels.

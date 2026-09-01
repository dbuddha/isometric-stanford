# No-transients policy

Final art removes people, bicycles, buses, trucks, cranes, temporary equipment,
and construction clutter. Passenger cars in an accepted registered reference
may remain. They are persistent-compatible evidence, not repair targets.

This is enforced structurally:

- The final semantic enum cannot represent transient classes.
- Intermediate reference masks may identify removable transient classes so the
  obstruction-repair stage can remove them. They cannot become a material,
  procedural asset, or final semantic object.
- The mask codec distinguishes `evidence`, `repair-input`, and `persistent`
  roles. Its writer and streaming validator both reject every transient or
  broken-source pixel in a persistent artifact.
- Nonzero instance identities are bounded and class-consistent. A passenger car
  instance cannot silently change into road pixels inside the same artifact.
- Style validation rejects transient sprite or asset names.
- Reference perception computes obstruction masks on complete supertiles
  before art-cell slicing so an object crossing a boundary receives one mask.
- Unclassified LiDAR returns 0.5 to 4 meters above cell ground are discarded
  from persistent-class evidence and retained only as an aggregate QA counter.
- Parking and road surfaces may retain registered passenger cars but never gain
  invented traffic.
- Release review includes road, parking, and construction fixtures.

An output detector may warn about a stylized shape but is not authoritative.
The authoritative evidence is the unrepresentable world contract, asset policy,
source sampling masks, and human audit. A violation blocks publication and is
fixed in compilation or grammar, never painted away in final pixels.

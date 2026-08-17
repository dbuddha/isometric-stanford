//! Immutable semantic-world contracts with no transient object classes.

use isometric_core::{ObjectId, WorldPoint};

/// The complete set of renderable v1 semantic classes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticClass {
    /// Terrain or landscaped ground.
    Terrain,
    /// Open water.
    Water,
    /// A permanent road surface.
    Road,
    /// A pedestrian or bicycle path surface.
    Path,
    /// A marked athletic field or court.
    AthleticSurface,
    /// A permanent parking surface, rendered empty.
    Parking,
    /// A permanent building or building part.
    Building,
    /// A tree or stable canopy object.
    Vegetation,
    /// A source conflict that must remain visibly unresolved.
    Unknown,
}

/// A minimal immutable object accepted by the renderer foundation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldObject {
    /// Stable source-derived identity.
    pub id: ObjectId,
    /// Permanent renderable semantic class.
    pub class: SemanticClass,
    /// World-space anchor.
    pub anchor: WorldPoint,
    /// Horizontal half extent in millimeters.
    pub radius_mm: u32,
    /// Height in millimeters.
    pub height_mm: u32,
}

/// Immutable canonical world input.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct World {
    objects: Vec<WorldObject>,
}

impl World {
    /// Builds a world sorted by stable object identity.
    #[must_use]
    pub fn from_objects(mut objects: Vec<WorldObject>) -> Self {
        objects.sort_by_key(|object| object.id);
        Self { objects }
    }

    /// Returns objects in deterministic stable-ID order.
    #[must_use]
    pub fn objects(&self) -> &[WorldObject] {
        &self.objects
    }

    /// Returns a small original fixture for renderer and web bootstrap tests.
    ///
    /// # Panics
    ///
    /// Panics only if a source-level fixture ID is changed to the reserved zero
    /// value. Fixture IDs are constants reviewed with the test source.
    #[must_use]
    pub fn reference_fixture() -> Self {
        let object = |id, class, x_mm, y_mm, z_mm, radius_mm, height_mm| WorldObject {
            id: ObjectId::new(id).expect("fixture IDs are non-zero"),
            class,
            anchor: WorldPoint::new(x_mm, y_mm, z_mm),
            radius_mm,
            height_mm,
        };
        Self::from_objects(vec![
            object(1, SemanticClass::Terrain, 0, 0, 0, 48_000, 0),
            object(2, SemanticClass::Road, -12_000, 8_000, 10, 7_000, 0),
            object(3, SemanticClass::Water, 26_000, 18_000, 0, 13_000, 0),
            object(4, SemanticClass::Building, 4_000, -5_000, 0, 9_000, 23_000),
            object(
                5,
                SemanticClass::Vegetation,
                -17_000,
                -12_000,
                0,
                6_000,
                12_000,
            ),
            object(6, SemanticClass::Path, 16_000, -17_000, 5, 3_000, 0),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::World;

    #[test]
    fn canonicalizes_object_order() {
        let world = World::reference_fixture();
        assert!(
            world
                .objects()
                .windows(2)
                .all(|pair| pair[0].id < pair[1].id)
        );
    }
}

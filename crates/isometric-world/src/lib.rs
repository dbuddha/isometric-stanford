//! Immutable polygonal semantic-world contracts with no transient classes.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt::{Display, Formatter},
};

use isometric_core::{ObjectId, WorldPoint};
use serde::Deserialize;

const FIXTURE_SCHEMA: &str = "isometric-world-fixture/v1";
const PARTITION_SIZE_MM: i64 = 128_000;
const MAX_PARTITIONS_PER_OBJECT: usize = 4_096;
const MAX_RING_POINTS: usize = 2_049;
const MAX_LOCAL_COORDINATE_MM: u64 = 10_000_000_000;

/// The complete set of renderable v1 semantic classes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
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

/// A nonempty stable source identifier.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceId(String);

impl SourceId {
    /// Creates a validated source identifier.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty identifier or unsupported characters.
    pub fn new(value: impl Into<String>) -> Result<Self, WorldError> {
        let value = value.into();
        if value.is_empty()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(WorldError::Invalid("source ID is empty or unsafe"));
        }
        Ok(Self(value))
    }

    /// Returns the stable identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Rights metadata retained with a canonical world source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceProvenance {
    /// Stable source identity.
    pub id: SourceId,
    /// License or public-domain statement.
    pub license: String,
    /// Required attribution text.
    pub attribution: String,
}

/// A validated semantic material identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterialId(String);

impl MaterialId {
    /// Creates a lower-case material identifier.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty identifier or unsupported characters.
    pub fn new(value: impl Into<String>) -> Result<Self, WorldError> {
        let value = value.into();
        if value.is_empty()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(WorldError::Invalid(
                "material ID must be lower-case kebab-case",
            ));
        }
        Ok(Self(value))
    }

    /// Returns the stable material identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Integer confidence in basis points from 0 through 10,000.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Confidence(u16);

impl Confidence {
    /// Creates a bounded confidence value.
    ///
    /// # Errors
    ///
    /// Returns an error when the value exceeds 10,000 basis points.
    pub const fn new(basis_points: u16) -> Result<Self, WorldError> {
        if basis_points > 10_000 {
            Err(WorldError::Invalid(
                "confidence exceeds 10,000 basis points",
            ))
        } else {
            Ok(Self(basis_points))
        }
    }

    /// Returns confidence in basis points.
    #[must_use]
    pub const fn basis_points(self) -> u16 {
        self.0
    }
}

/// Supported deterministic roof forms.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RoofKind {
    /// Flat roof.
    Flat,
    /// Two-slope gabled roof.
    Gabled,
    /// Four-slope hipped roof.
    Hipped,
    /// Four-face pyramidal roof.
    Pyramidal,
    /// Single-slope shed roof.
    Shed,
    /// Reviewed compound roof assembled from deterministic parts.
    Complex,
    /// Unresolved roof evidence.
    Unknown,
}

/// Validated roof metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Roof {
    kind: RoofKind,
    direction_millidegrees: u32,
}

impl Roof {
    /// Creates roof metadata with a normalized direction.
    ///
    /// # Errors
    ///
    /// Returns an error when direction is at least 360 degrees.
    pub const fn new(kind: RoofKind, direction_millidegrees: u32) -> Result<Self, WorldError> {
        if direction_millidegrees >= 360_000 {
            Err(WorldError::Invalid(
                "roof direction must be below 360 degrees",
            ))
        } else {
            Ok(Self {
                kind,
                direction_millidegrees,
            })
        }
    }

    /// Returns the roof form.
    #[must_use]
    pub const fn kind(self) -> RoofKind {
        self.kind
    }

    /// Returns clockwise direction from grid north in thousandths of a degree.
    #[must_use]
    pub const fn direction_millidegrees(self) -> u32 {
        self.direction_millidegrees
    }
}

/// Review disposition attached to fused semantic data.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewStatus {
    /// Accepted directly from source fusion.
    Accepted,
    /// Accepted with an explicit reviewed override.
    AcceptedOverride,
    /// Unresolved disagreement that must remain unknown.
    UnreviewedConflict,
}

/// A closed, nonzero-area polygon ring.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ring {
    points: Vec<WorldPoint>,
}

impl Ring {
    /// Validates and constructs a closed ring.
    ///
    /// # Errors
    ///
    /// Returns an error for fewer than four points, an open ring, repeated
    /// adjacent points, or zero projected area.
    pub fn try_new(points: Vec<WorldPoint>) -> Result<Self, WorldError> {
        if !(4..=MAX_RING_POINTS).contains(&points.len()) {
            return Err(WorldError::Invalid(
                "ring point count is outside the accepted range",
            ));
        }
        if points.first() != points.last() {
            return Err(WorldError::Invalid("ring must be closed"));
        }
        if points.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(WorldError::Invalid("ring has repeated adjacent points"));
        }
        if points.iter().any(|point| {
            point.x_mm.unsigned_abs() > MAX_LOCAL_COORDINATE_MM
                || point.y_mm.unsigned_abs() > MAX_LOCAL_COORDINATE_MM
                || point.z_mm.unsigned_abs() > MAX_LOCAL_COORDINATE_MM
        }) {
            return Err(WorldError::Invalid(
                "ring coordinate exceeds local-world limits",
            ));
        }
        let mut area_twice = 0_i128;
        for pair in points.windows(2) {
            let forward = i128::from(pair[0].x_mm)
                .checked_mul(i128::from(pair[1].y_mm))
                .ok_or(WorldError::Invalid("ring area overflowed"))?;
            let backward = i128::from(pair[1].x_mm)
                .checked_mul(i128::from(pair[0].y_mm))
                .ok_or(WorldError::Invalid("ring area overflowed"))?;
            area_twice = area_twice
                .checked_add(
                    forward
                        .checked_sub(backward)
                        .ok_or(WorldError::Invalid("ring area overflowed"))?,
                )
                .ok_or(WorldError::Invalid("ring area overflowed"))?;
        }
        if area_twice == 0 {
            return Err(WorldError::Invalid("ring has zero projected area"));
        }
        if ring_self_intersects(&points) {
            return Err(WorldError::Invalid("ring self-intersects"));
        }
        Ok(Self { points })
    }

    /// Returns ring points including the repeated closing point.
    #[must_use]
    pub fn points(&self) -> &[WorldPoint] {
        &self.points
    }
}

/// A polygon whose first ring is the shell and remaining rings are holes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Polygon {
    rings: Vec<Ring>,
}

impl Polygon {
    /// Creates a polygon from one shell and optional holes.
    ///
    /// # Errors
    ///
    /// Returns an error when no shell ring is supplied.
    pub fn try_new(rings: Vec<Ring>) -> Result<Self, WorldError> {
        if rings.is_empty() {
            return Err(WorldError::Invalid("polygon requires a shell ring"));
        }
        let shell = &rings[0];
        for (index, hole) in rings.iter().enumerate().skip(1) {
            if rings_intersect(shell, hole) || !point_in_ring(hole.points[0], shell) {
                return Err(WorldError::Invalid(
                    "polygon hole is not strictly inside its shell",
                ));
            }
            for other in &rings[1..index] {
                if rings_intersect(hole, other)
                    || point_in_ring(hole.points[0], other)
                    || point_in_ring(other.points[0], hole)
                {
                    return Err(WorldError::Invalid(
                        "polygon holes overlap or contain one another",
                    ));
                }
            }
        }
        Ok(Self { rings })
    }

    /// Returns the shell followed by hole rings.
    #[must_use]
    pub fn rings(&self) -> &[Ring] {
        &self.rings
    }
}

/// Polygonal geometry accepted by the v1 canonical world.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Geometry {
    /// One polygon with optional holes.
    Polygon(Polygon),
    /// Multiple polygons, each with optional holes.
    MultiPolygon(Vec<Polygon>),
}

impl Geometry {
    /// Validates and constructs a multipolygon.
    ///
    /// # Errors
    ///
    /// Returns an error when no polygon is supplied.
    pub fn try_multipolygon(polygons: Vec<Polygon>) -> Result<Self, WorldError> {
        if polygons.is_empty() {
            return Err(WorldError::Invalid("multipolygon requires polygons"));
        }
        for (index, polygon) in polygons.iter().enumerate() {
            for other in &polygons[..index] {
                let shell = &polygon.rings[0];
                let other_shell = &other.rings[0];
                if rings_intersect(shell, other_shell)
                    || point_in_polygon(shell.points[0], other)
                    || point_in_polygon(other_shell.points[0], polygon)
                {
                    return Err(WorldError::Invalid("multipolygon components overlap"));
                }
            }
        }
        Ok(Self::MultiPolygon(polygons))
    }

    /// Visits every point, including repeated ring-closing points.
    fn points(&self) -> impl Iterator<Item = &WorldPoint> {
        let polygons: &[Polygon] = match self {
            Self::Polygon(polygon) => std::slice::from_ref(polygon),
            Self::MultiPolygon(polygons) => polygons,
        };
        polygons
            .iter()
            .flat_map(|polygon| polygon.rings.iter())
            .flat_map(|ring| ring.points.iter())
    }
}

/// Conservative three-dimensional bounds in local millimeters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldBounds {
    /// Minimum easting.
    pub min_x_mm: i64,
    /// Minimum northing.
    pub min_y_mm: i64,
    /// Minimum elevation.
    pub min_z_mm: i64,
    /// Maximum easting.
    pub max_x_mm: i64,
    /// Maximum northing.
    pub max_y_mm: i64,
    /// Maximum elevation including object height.
    pub max_z_mm: i64,
}

/// Conservative canonical 2:1 screen bounds before style scaling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScreenBounds {
    /// Minimum value of `x - y` in millimeters.
    pub min_x_mm: i64,
    /// Maximum value of `x - y` in millimeters.
    pub max_x_mm: i64,
    /// Minimum doubled vertical value `x + y - 2z`.
    pub min_y_twice_mm: i64,
    /// Maximum doubled vertical value `x + y - 2z`.
    pub max_y_twice_mm: i64,
}

/// A deterministic 128 meter spatial partition key.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PartitionKey {
    /// Easting cell index.
    pub x: i32,
    /// Northing cell index.
    pub y: i32,
}

/// Fixed projected origin used to interpret local world coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldOrigin {
    epsg: u32,
    easting_mm: i64,
    northing_mm: i64,
    elevation_mm: i64,
}

impl WorldOrigin {
    /// Constructs the v1 local origin.
    ///
    /// # Errors
    ///
    /// Returns an error unless the EPSG code is 26910 and the projected values
    /// fall within conservative UTM and terrestrial elevation limits.
    pub const fn new(
        epsg: u32,
        easting_mm: i64,
        northing_mm: i64,
        elevation_mm: i64,
    ) -> Result<Self, WorldError> {
        if epsg == 26_910
            && easting_mm >= 100_000_000
            && easting_mm <= 900_000_000
            && northing_mm >= 0
            && northing_mm <= 10_000_000_000
            && elevation_mm >= -1_000_000
            && elevation_mm <= 10_000_000
        {
            Ok(Self {
                epsg,
                easting_mm,
                northing_mm,
                elevation_mm,
            })
        } else {
            Err(WorldError::Invalid(
                "world origin is outside EPSG:26910 limits",
            ))
        }
    }

    /// Returns the EPSG code.
    #[must_use]
    pub const fn epsg(self) -> u32 {
        self.epsg
    }

    /// Returns absolute projected easting in millimeters.
    #[must_use]
    pub const fn easting_mm(self) -> i64 {
        self.easting_mm
    }

    /// Returns absolute projected northing in millimeters.
    #[must_use]
    pub const fn northing_mm(self) -> i64 {
        self.northing_mm
    }

    /// Returns the elevation datum offset in millimeters.
    #[must_use]
    pub const fn elevation_mm(self) -> i64 {
        self.elevation_mm
    }
}

/// All validated inputs required to create one world object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldObjectInput {
    /// Stable source-derived identity.
    pub id: ObjectId,
    /// Permanent renderable semantic class.
    pub class: SemanticClass,
    /// Polygonal local-millimeter geometry.
    pub geometry: Geometry,
    /// Extrusion or canopy height.
    pub height_mm: u32,
    /// Optional floor count.
    pub floor_count: Option<u16>,
    /// Optional roof metadata.
    pub roof: Option<Roof>,
    /// Optional semantic material.
    pub material: Option<MaterialId>,
    /// Confidence in fused geometry and classification.
    pub confidence: Confidence,
    /// Sorted contributing source identities.
    pub source_ids: Vec<SourceId>,
    /// Review disposition.
    pub review: ReviewStatus,
    /// Required explanation for overrides and unresolved conflicts.
    pub review_note: Option<String>,
    /// Optional parent building for a building part.
    pub parent_id: Option<ObjectId>,
}

/// One immutable validated canonical-world object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldObject {
    id: ObjectId,
    class: SemanticClass,
    geometry: Geometry,
    height_mm: u32,
    floor_count: Option<u16>,
    roof: Option<Roof>,
    material: Option<MaterialId>,
    confidence: Confidence,
    source_ids: Vec<SourceId>,
    review: ReviewStatus,
    review_note: Option<String>,
    parent_id: Option<ObjectId>,
    bounds: WorldBounds,
    screen_bounds: ScreenBounds,
    radius_mm: u32,
}

impl WorldObject {
    /// Validates an object and derives conservative bounds.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid metadata, source ordering, extents, or
    /// incompatible class-specific fields.
    pub fn try_new(mut input: WorldObjectInput) -> Result<Self, WorldError> {
        if input.source_ids.is_empty() {
            return Err(WorldError::Invalid("world object requires provenance"));
        }
        let original_sources = input.source_ids.clone();
        input.source_ids.sort();
        input.source_ids.dedup();
        if input.source_ids != original_sources {
            return Err(WorldError::Invalid("source IDs must be uniquely sorted"));
        }
        if input.parent_id.is_some() && input.class != SemanticClass::Building {
            return Err(WorldError::Invalid("only buildings can be building parts"));
        }
        if input.class != SemanticClass::Building
            && (input.floor_count.is_some() || input.roof.is_some())
        {
            return Err(WorldError::Invalid(
                "only buildings may have floors or roofs",
            ));
        }
        if input.floor_count == Some(0) {
            return Err(WorldError::Invalid("floor count must be positive"));
        }
        if input.class == SemanticClass::Unknown && input.review != ReviewStatus::UnreviewedConflict
        {
            return Err(WorldError::Invalid(
                "unknown objects must remain unreviewed conflicts",
            ));
        }
        let note_required = matches!(
            input.review,
            ReviewStatus::AcceptedOverride | ReviewStatus::UnreviewedConflict
        );
        if note_required
            && input
                .review_note
                .as_deref()
                .is_none_or(|note| note.trim().is_empty())
        {
            return Err(WorldError::Invalid("review disposition requires a note"));
        }
        let bounds = derive_world_bounds(&input.geometry, input.height_mm)?;
        let screen_bounds = derive_screen_bounds(bounds)?;
        let radius_mm = derive_radius_mm(bounds)?;
        Ok(Self {
            id: input.id,
            class: input.class,
            geometry: input.geometry,
            height_mm: input.height_mm,
            floor_count: input.floor_count,
            roof: input.roof,
            material: input.material,
            confidence: input.confidence,
            source_ids: input.source_ids,
            review: input.review,
            review_note: input.review_note,
            parent_id: input.parent_id,
            bounds,
            screen_bounds,
            radius_mm,
        })
    }

    /// Returns conservative world bounds including height.
    #[must_use]
    pub const fn bounds(&self) -> WorldBounds {
        self.bounds
    }

    /// Returns the stable source-derived identity.
    #[must_use]
    pub const fn id(&self) -> ObjectId {
        self.id
    }

    /// Returns the permanent semantic class.
    #[must_use]
    pub const fn class(&self) -> SemanticClass {
        self.class
    }

    /// Returns immutable polygonal geometry.
    #[must_use]
    pub const fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    /// Returns extrusion or canopy height in millimeters.
    #[must_use]
    pub const fn height_mm(&self) -> u32 {
        self.height_mm
    }

    /// Returns the optional floor count.
    #[must_use]
    pub const fn floor_count(&self) -> Option<u16> {
        self.floor_count
    }

    /// Returns optional roof metadata.
    #[must_use]
    pub const fn roof(&self) -> Option<Roof> {
        self.roof
    }

    /// Returns the optional semantic material.
    #[must_use]
    pub const fn material(&self) -> Option<&MaterialId> {
        self.material.as_ref()
    }

    /// Returns fused confidence.
    #[must_use]
    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }

    /// Returns contributing source IDs in stable order.
    #[must_use]
    pub fn source_ids(&self) -> &[SourceId] {
        &self.source_ids
    }

    /// Returns review disposition.
    #[must_use]
    pub const fn review(&self) -> ReviewStatus {
        self.review
    }

    /// Returns the review explanation when required.
    #[must_use]
    pub fn review_note(&self) -> Option<&str> {
        self.review_note.as_deref()
    }

    /// Returns the parent building ID for a building part.
    #[must_use]
    pub const fn parent_id(&self) -> Option<ObjectId> {
        self.parent_id
    }

    /// Returns conservative canonical screen bounds.
    #[must_use]
    pub const fn screen_bounds(&self) -> ScreenBounds {
        self.screen_bounds
    }

    /// Returns the center of the horizontal bounds at base elevation.
    #[must_use]
    pub fn anchor(&self) -> WorldPoint {
        WorldPoint::new(
            midpoint(self.bounds.min_x_mm, self.bounds.max_x_mm),
            midpoint(self.bounds.min_y_mm, self.bounds.max_y_mm),
            self.bounds.min_z_mm,
        )
    }

    /// Returns a conservative horizontal half extent for bootstrap rendering.
    #[must_use]
    pub const fn radius_mm(&self) -> u32 {
        self.radius_mm
    }
}

/// Immutable canonical world input and deterministic spatial index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct World {
    origin: WorldOrigin,
    sources: Vec<SourceProvenance>,
    objects: Vec<WorldObject>,
    partitions: BTreeMap<PartitionKey, Vec<ObjectId>>,
}

impl World {
    /// Builds and validates a canonical world in stable identity order.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate identities, unknown source references,
    /// invalid parent relations, or objects spanning an unreasonable number of
    /// partitions.
    pub fn try_new(
        origin: WorldOrigin,
        mut sources: Vec<SourceProvenance>,
        mut objects: Vec<WorldObject>,
    ) -> Result<Self, WorldError> {
        sources.sort_by(|left, right| left.id.cmp(&right.id));
        if sources.is_empty()
            || sources.windows(2).any(|pair| pair[0].id == pair[1].id)
            || sources
                .iter()
                .any(|source| source.license.is_empty() || source.attribution.is_empty())
        {
            return Err(WorldError::Invalid(
                "world sources are empty, duplicated, or incomplete",
            ));
        }
        objects.sort_by_key(|object| object.id);
        if objects.is_empty() || objects.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(WorldError::Invalid("world objects are empty or duplicated"));
        }

        let source_ids = sources
            .iter()
            .map(|source| source.id.clone())
            .collect::<BTreeSet<_>>();
        let by_id = objects
            .iter()
            .map(|object| (object.id, object))
            .collect::<BTreeMap<_, _>>();
        for object in &objects {
            if object
                .source_ids
                .iter()
                .any(|source| !source_ids.contains(source))
            {
                return Err(WorldError::Invalid("object references an unknown source"));
            }
            validate_parent_chain(object, &by_id)?;
        }

        let mut partitions = BTreeMap::<PartitionKey, Vec<ObjectId>>::new();
        for object in &objects {
            let keys = partition_keys(object.bounds)?;
            for key in keys {
                partitions.entry(key).or_default().push(object.id);
            }
        }
        Ok(Self {
            origin,
            sources,
            objects,
            partitions,
        })
    }

    /// Decodes and validates the portable representative fixture contract.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed JSON or any invalid world invariant.
    pub fn from_fixture_json(json: &str) -> Result<Self, WorldError> {
        let fixture: RawFixture = serde_json::from_str(json)?;
        if fixture.schema != FIXTURE_SCHEMA
            || fixture.license != "CC-BY-4.0"
            || fixture.crs != "local integer millimeters derived from EPSG:26910"
            || fixture.contains_source_pixels
            || fixture.contains_transients
        {
            return Err(WorldError::Invalid("fixture policy metadata is invalid"));
        }
        let origin = WorldOrigin::new(
            fixture.origin.epsg,
            fixture.origin.easting_mm,
            fixture.origin.northing_mm,
            fixture.origin.elevation_mm,
        )?;
        let sources = fixture
            .sources
            .into_iter()
            .map(|source| {
                Ok(SourceProvenance {
                    id: SourceId::new(source.id)?,
                    license: source.license,
                    attribution: source.attribution,
                })
            })
            .collect::<Result<Vec<_>, WorldError>>()?;
        let objects = fixture
            .features
            .into_iter()
            .map(WorldObject::try_from)
            .collect::<Result<Vec<_>, WorldError>>()?;
        Self::try_new(origin, sources, objects)
    }

    /// Returns objects in deterministic stable-ID order.
    #[must_use]
    pub fn objects(&self) -> &[WorldObject] {
        &self.objects
    }

    /// Returns approved source provenance in stable-ID order.
    #[must_use]
    pub fn sources(&self) -> &[SourceProvenance] {
        &self.sources
    }

    /// Returns the fixed projected origin.
    #[must_use]
    pub const fn origin(&self) -> WorldOrigin {
        self.origin
    }

    /// Returns the deterministic spatial partition index.
    #[must_use]
    pub const fn partitions(&self) -> &BTreeMap<PartitionKey, Vec<ObjectId>> {
        &self.partitions
    }

    /// Returns a small original fixture for renderer bootstrap tests.
    ///
    /// # Panics
    ///
    /// Panics only if reviewed constant fixture data violates world invariants.
    #[must_use]
    pub fn reference_fixture() -> Self {
        let source = SourceId::new("original-bootstrap").expect("constant source ID is valid");
        let provenance = SourceProvenance {
            id: source.clone(),
            license: "CC-BY-4.0".into(),
            attribution: "Isometric Stanford contributors".into(),
        };
        let object = |id, class, x_mm, y_mm, z_mm, radius_mm, height_mm| {
            let review = if class == SemanticClass::Unknown {
                ReviewStatus::UnreviewedConflict
            } else {
                ReviewStatus::Accepted
            };
            WorldObject::try_new(WorldObjectInput {
                id: ObjectId::new(id).expect("fixture IDs are nonzero"),
                class,
                geometry: rectangle(x_mm, y_mm, z_mm, radius_mm)
                    .expect("fixture rectangles are valid"),
                height_mm,
                floor_count: (class == SemanticClass::Building).then_some(2),
                roof: None,
                material: None,
                confidence: Confidence::new(10_000).expect("confidence is valid"),
                source_ids: vec![source.clone()],
                review,
                review_note: None,
                parent_id: None,
            })
            .expect("fixture object is valid")
        };
        Self::try_new(
            WorldOrigin::new(26_910, 573_200_000, 4_142_200_000, 0)
                .expect("fixture origin is valid"),
            vec![provenance],
            vec![
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
            ],
        )
        .expect("reference world is valid")
    }
}

/// Fail-closed canonical-world error.
#[derive(Debug)]
pub enum WorldError {
    /// An invariant was violated.
    Invalid(&'static str),
    /// Portable JSON could not be decoded.
    Json(serde_json::Error),
}

impl Display for WorldError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::Json(error) => write!(formatter, "world JSON failed: {error}"),
        }
    }
}

impl Error for WorldError {}

impl From<serde_json::Error> for WorldError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Deserialize)]
struct RawFixture {
    schema: String,
    license: String,
    crs: String,
    origin: RawOrigin,
    contains_source_pixels: bool,
    contains_transients: bool,
    sources: Vec<RawSource>,
    features: Vec<RawFeature>,
}

#[derive(Deserialize)]
struct RawOrigin {
    epsg: u32,
    easting_mm: i64,
    northing_mm: i64,
    elevation_mm: i64,
}

#[derive(Deserialize)]
struct RawSource {
    id: String,
    license: String,
    attribution: String,
}

#[derive(Deserialize)]
struct RawFeature {
    id: u64,
    class: SemanticClass,
    geometry: RawGeometry,
    #[serde(default)]
    height_mm: u32,
    floor_count: Option<u16>,
    roof: Option<RawRoof>,
    material: Option<String>,
    confidence_bp: u16,
    source_ids: Vec<String>,
    review: ReviewStatus,
    review_note: Option<String>,
    parent_id: Option<u64>,
}

#[derive(Deserialize)]
struct RawRoof {
    kind: RoofKind,
    direction_millidegrees: u32,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum RawGeometry {
    Polygon { rings: Vec<Vec<[i64; 3]>> },
    Multipolygon { polygons: Vec<RawPolygon> },
}

#[derive(Deserialize)]
struct RawPolygon {
    rings: Vec<Vec<[i64; 3]>>,
}

impl TryFrom<RawFeature> for WorldObject {
    type Error = WorldError;

    fn try_from(raw: RawFeature) -> Result<Self, Self::Error> {
        Self::try_new(WorldObjectInput {
            id: ObjectId::new(raw.id).map_err(|_| WorldError::Invalid("object ID is reserved"))?,
            class: raw.class,
            geometry: raw.geometry.try_into()?,
            height_mm: raw.height_mm,
            floor_count: raw.floor_count,
            roof: raw
                .roof
                .map(|roof| Roof::new(roof.kind, roof.direction_millidegrees))
                .transpose()?,
            material: raw.material.map(MaterialId::new).transpose()?,
            confidence: Confidence::new(raw.confidence_bp)?,
            source_ids: raw
                .source_ids
                .into_iter()
                .map(SourceId::new)
                .collect::<Result<Vec<_>, _>>()?,
            review: raw.review,
            review_note: raw.review_note,
            parent_id: raw
                .parent_id
                .map(|id| {
                    ObjectId::new(id).map_err(|_| WorldError::Invalid("parent ID is reserved"))
                })
                .transpose()?,
        })
    }
}

impl TryFrom<RawGeometry> for Geometry {
    type Error = WorldError;

    fn try_from(raw: RawGeometry) -> Result<Self, Self::Error> {
        match raw {
            RawGeometry::Polygon { rings } => Ok(Self::Polygon(parse_polygon(rings)?)),
            RawGeometry::Multipolygon { polygons } => Self::try_multipolygon(
                polygons
                    .into_iter()
                    .map(|polygon| parse_polygon(polygon.rings))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        }
    }
}

fn parse_polygon(rings: Vec<Vec<[i64; 3]>>) -> Result<Polygon, WorldError> {
    Polygon::try_new(
        rings
            .into_iter()
            .map(|ring| {
                Ring::try_new(
                    ring.into_iter()
                        .map(|point| WorldPoint::new(point[0], point[1], point[2]))
                        .collect(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn rectangle(
    center_x_mm: i64,
    center_north_mm: i64,
    z_mm: i64,
    radius_mm: u32,
) -> Result<Geometry, WorldError> {
    let radius = i64::from(radius_mm);
    let min_x = center_x_mm
        .checked_sub(radius)
        .ok_or(WorldError::Invalid("fixture rectangle overflowed"))?;
    let max_x = center_x_mm
        .checked_add(radius)
        .ok_or(WorldError::Invalid("fixture rectangle overflowed"))?;
    let min_y = center_north_mm
        .checked_sub(radius)
        .ok_or(WorldError::Invalid("fixture rectangle overflowed"))?;
    let max_y = center_north_mm
        .checked_add(radius)
        .ok_or(WorldError::Invalid("fixture rectangle overflowed"))?;
    Ok(Geometry::Polygon(Polygon::try_new(vec![Ring::try_new(
        vec![
            WorldPoint::new(min_x, min_y, z_mm),
            WorldPoint::new(max_x, min_y, z_mm),
            WorldPoint::new(max_x, max_y, z_mm),
            WorldPoint::new(min_x, max_y, z_mm),
            WorldPoint::new(min_x, min_y, z_mm),
        ],
    )?])?))
}

fn ring_self_intersects(points: &[WorldPoint]) -> bool {
    let edge_count = points.len() - 1;
    for left in 0..edge_count {
        for right in (left + 1)..edge_count {
            let adjacent = right == left + 1 || (left == 0 && right == edge_count - 1);
            if !adjacent
                && segments_intersect(
                    points[left],
                    points[left + 1],
                    points[right],
                    points[right + 1],
                )
            {
                return true;
            }
        }
    }
    false
}

fn rings_intersect(left: &Ring, right: &Ring) -> bool {
    left.points.windows(2).any(|left_edge| {
        right.points.windows(2).any(|right_edge| {
            segments_intersect(left_edge[0], left_edge[1], right_edge[0], right_edge[1])
        })
    })
}

fn segments_intersect(
    left_start: WorldPoint,
    left_end: WorldPoint,
    right_start: WorldPoint,
    right_end: WorldPoint,
) -> bool {
    let first = orientation(left_start, left_end, right_start);
    let second = orientation(left_start, left_end, right_end);
    let third = orientation(right_start, right_end, left_start);
    let fourth = orientation(right_start, right_end, left_end);
    if first == 0 && point_on_segment(right_start, left_start, left_end)
        || second == 0 && point_on_segment(right_end, left_start, left_end)
        || third == 0 && point_on_segment(left_start, right_start, right_end)
        || fourth == 0 && point_on_segment(left_end, right_start, right_end)
    {
        return true;
    }
    opposite_sign(first, second) && opposite_sign(third, fourth)
}

const fn opposite_sign(left: i128, right: i128) -> bool {
    (left < 0 && right > 0) || (left > 0 && right < 0)
}

fn orientation(start: WorldPoint, end: WorldPoint, point: WorldPoint) -> i128 {
    (i128::from(end.x_mm) - i128::from(start.x_mm))
        * (i128::from(point.y_mm) - i128::from(start.y_mm))
        - (i128::from(end.y_mm) - i128::from(start.y_mm))
            * (i128::from(point.x_mm) - i128::from(start.x_mm))
}

fn point_on_segment(point: WorldPoint, start: WorldPoint, end: WorldPoint) -> bool {
    point.x_mm >= start.x_mm.min(end.x_mm)
        && point.x_mm <= start.x_mm.max(end.x_mm)
        && point.y_mm >= start.y_mm.min(end.y_mm)
        && point.y_mm <= start.y_mm.max(end.y_mm)
}

fn point_in_ring(point: WorldPoint, ring: &Ring) -> bool {
    let mut winding = 0_i32;
    for edge in ring.points.windows(2) {
        if edge[0].y_mm <= point.y_mm {
            if edge[1].y_mm > point.y_mm && orientation(edge[0], edge[1], point) > 0 {
                winding += 1;
            }
        } else if edge[1].y_mm <= point.y_mm && orientation(edge[0], edge[1], point) < 0 {
            winding -= 1;
        }
    }
    winding != 0
}

fn point_in_polygon(point: WorldPoint, polygon: &Polygon) -> bool {
    point_in_ring(point, &polygon.rings[0])
        && polygon.rings[1..]
            .iter()
            .all(|hole| !point_in_ring(point, hole))
}

fn derive_world_bounds(geometry: &Geometry, height_mm: u32) -> Result<WorldBounds, WorldError> {
    let mut points = geometry.points();
    let first = points
        .next()
        .ok_or(WorldError::Invalid("geometry has no points"))?;
    let mut bounds = WorldBounds {
        min_x_mm: first.x_mm,
        min_y_mm: first.y_mm,
        min_z_mm: first.z_mm,
        max_x_mm: first.x_mm,
        max_y_mm: first.y_mm,
        max_z_mm: first.z_mm,
    };
    for point in points {
        bounds.min_x_mm = bounds.min_x_mm.min(point.x_mm);
        bounds.min_y_mm = bounds.min_y_mm.min(point.y_mm);
        bounds.min_z_mm = bounds.min_z_mm.min(point.z_mm);
        bounds.max_x_mm = bounds.max_x_mm.max(point.x_mm);
        bounds.max_y_mm = bounds.max_y_mm.max(point.y_mm);
        bounds.max_z_mm = bounds.max_z_mm.max(point.z_mm);
    }
    bounds.max_z_mm = bounds
        .max_z_mm
        .checked_add(i64::from(height_mm))
        .ok_or(WorldError::Invalid("height overflowed world bounds"))?;
    let width = i128::from(bounds.max_x_mm) - i128::from(bounds.min_x_mm);
    let depth = i128::from(bounds.max_y_mm) - i128::from(bounds.min_y_mm);
    if width <= 0 || depth <= 0 || width.max(depth) / 2 > i128::from(u32::MAX) {
        return Err(WorldError::Invalid(
            "geometry extent is invalid or too large",
        ));
    }
    Ok(bounds)
}

fn derive_screen_bounds(bounds: WorldBounds) -> Result<ScreenBounds, WorldError> {
    let mut min_x = i128::MAX;
    let mut max_x = i128::MIN;
    let mut min_y = i128::MAX;
    let mut max_y = i128::MIN;
    for x in [bounds.min_x_mm, bounds.max_x_mm] {
        for y in [bounds.min_y_mm, bounds.max_y_mm] {
            for z in [bounds.min_z_mm, bounds.max_z_mm] {
                let screen_x = i128::from(x) - i128::from(y);
                let screen_y = i128::from(x) + i128::from(y) - 2 * i128::from(z);
                min_x = min_x.min(screen_x);
                max_x = max_x.max(screen_x);
                min_y = min_y.min(screen_y);
                max_y = max_y.max(screen_y);
            }
        }
    }
    Ok(ScreenBounds {
        min_x_mm: i64::try_from(min_x)
            .map_err(|_| WorldError::Invalid("screen bounds overflowed"))?,
        max_x_mm: i64::try_from(max_x)
            .map_err(|_| WorldError::Invalid("screen bounds overflowed"))?,
        min_y_twice_mm: i64::try_from(min_y)
            .map_err(|_| WorldError::Invalid("screen bounds overflowed"))?,
        max_y_twice_mm: i64::try_from(max_y)
            .map_err(|_| WorldError::Invalid("screen bounds overflowed"))?,
    })
}

fn derive_radius_mm(bounds: WorldBounds) -> Result<u32, WorldError> {
    let width = i128::from(bounds.max_x_mm) - i128::from(bounds.min_x_mm);
    let depth = i128::from(bounds.max_y_mm) - i128::from(bounds.min_y_mm);
    u32::try_from(width.max(depth) / 2)
        .map_err(|_| WorldError::Invalid("geometry half extent exceeds u32"))
}

fn partition_keys(bounds: WorldBounds) -> Result<Vec<PartitionKey>, WorldError> {
    let min_x = bounds.min_x_mm.div_euclid(PARTITION_SIZE_MM);
    let max_x = bounds.max_x_mm.div_euclid(PARTITION_SIZE_MM);
    let min_y = bounds.min_y_mm.div_euclid(PARTITION_SIZE_MM);
    let max_y = bounds.max_y_mm.div_euclid(PARTITION_SIZE_MM);
    let count_x = i128::from(max_x) - i128::from(min_x) + 1;
    let count_y = i128::from(max_y) - i128::from(min_y) + 1;
    if count_x * count_y > MAX_PARTITIONS_PER_OBJECT as i128 {
        return Err(WorldError::Invalid("object spans too many partitions"));
    }
    let mut keys =
        Vec::with_capacity(usize::try_from(count_x * count_y).expect("count is bounded"));
    for x in min_x..=max_x {
        for y in min_y..=max_y {
            keys.push(PartitionKey {
                x: i32::try_from(x).map_err(|_| WorldError::Invalid("partition key overflowed"))?,
                y: i32::try_from(y).map_err(|_| WorldError::Invalid("partition key overflowed"))?,
            });
        }
    }
    Ok(keys)
}

fn validate_parent_chain(
    object: &WorldObject,
    by_id: &BTreeMap<ObjectId, &WorldObject>,
) -> Result<(), WorldError> {
    let mut next = object.parent_id;
    let mut seen = BTreeSet::new();
    while let Some(parent_id) = next {
        if !seen.insert(parent_id) || parent_id == object.id {
            return Err(WorldError::Invalid("building parent cycle detected"));
        }
        let parent = by_id
            .get(&parent_id)
            .ok_or(WorldError::Invalid("building part has an unknown parent"))?;
        if parent.class != SemanticClass::Building {
            return Err(WorldError::Invalid(
                "building part parent is not a building",
            ));
        }
        next = parent.parent_id;
    }
    Ok(())
}

fn midpoint(minimum: i64, maximum: i64) -> i64 {
    let value = i128::from(minimum) + (i128::from(maximum) - i128::from(minimum)) / 2;
    i64::try_from(value).expect("midpoint of validated i64 bounds fits i64")
}

#[cfg(test)]
mod tests {
    use super::{
        Confidence, Polygon, Ring, SemanticClass, SourceId, World, WorldOrigin, WorldPoint,
    };
    use isometric_core::ObjectId;

    const REPRESENTATIVE: &str = include_str!("../../../fixtures/world/representative.json");

    #[test]
    fn canonicalizes_object_and_source_order() {
        let world = World::from_fixture_json(REPRESENTATIVE).expect("fixture is valid");
        assert!(
            world
                .objects()
                .windows(2)
                .all(|pair| pair[0].id < pair[1].id)
        );
        assert!(
            world
                .sources()
                .windows(2)
                .all(|pair| pair[0].id < pair[1].id)
        );
    }

    #[test]
    fn representative_fixture_preserves_geometry_and_metadata() {
        let world = World::from_fixture_json(REPRESENTATIVE).expect("fixture is valid");
        let terrain = &world.objects()[0];
        let building_part = &world.objects()[2];
        assert_eq!(terrain.geometry.clone().points().count(), 10);
        assert_eq!(building_part.parent_id.map(ObjectId::get), Some(1002));
        assert_eq!(building_part.floor_count, Some(1));
        assert_eq!(building_part.confidence.basis_points(), 8_800);
        assert_eq!(world.origin().epsg(), 26_910);
        assert!(building_part.review_note().is_some());
        assert!(!world.partitions().is_empty());
        assert!(building_part.screen_bounds().min_x_mm <= building_part.screen_bounds().max_x_mm);
    }

    #[test]
    fn rejects_invalid_ring_and_confidence() {
        assert!(Ring::try_new(vec![WorldPoint::new(0, 0, 0); 4]).is_err());
        assert!(Confidence::new(10_001).is_err());
        assert!(WorldOrigin::new(4_326, 0, 0, 0).is_err());
    }

    #[test]
    fn rejects_self_intersections_and_outside_holes() {
        let self_intersecting = vec![
            WorldPoint::new(0, 0, 0),
            WorldPoint::new(4, 4, 0),
            WorldPoint::new(0, 4, 0),
            WorldPoint::new(3, 0, 0),
            WorldPoint::new(0, 0, 0),
        ];
        assert!(Ring::try_new(self_intersecting).is_err());

        let shell = Ring::try_new(vec![
            WorldPoint::new(0, 0, 0),
            WorldPoint::new(10, 0, 0),
            WorldPoint::new(10, 10, 0),
            WorldPoint::new(0, 10, 0),
            WorldPoint::new(0, 0, 0),
        ])
        .expect("shell is valid");
        let outside_hole = Ring::try_new(vec![
            WorldPoint::new(20, 20, 0),
            WorldPoint::new(24, 20, 0),
            WorldPoint::new(24, 24, 0),
            WorldPoint::new(20, 24, 0),
            WorldPoint::new(20, 20, 0),
        ])
        .expect("ring is structurally valid");
        assert!(Polygon::try_new(vec![shell, outside_hole]).is_err());
    }

    #[test]
    fn semantic_model_has_no_transient_variant() {
        let classes = [
            SemanticClass::Terrain,
            SemanticClass::Water,
            SemanticClass::Road,
            SemanticClass::Path,
            SemanticClass::AthleticSurface,
            SemanticClass::Parking,
            SemanticClass::Building,
            SemanticClass::Vegetation,
            SemanticClass::Unknown,
        ];
        assert_eq!(classes.len(), 9);
    }

    #[test]
    fn rejects_unknown_provenance_and_parent_cycles() {
        let world = World::from_fixture_json(REPRESENTATIVE).expect("fixture is valid");
        let mut unknown_source_objects = world.objects.clone();
        unknown_source_objects[0].source_ids =
            vec![SourceId::new("unlocked-source").expect("source ID is valid")];
        assert!(
            World::try_new(world.origin, world.sources.clone(), unknown_source_objects).is_err()
        );

        let mut cyclic_objects = world.objects.clone();
        cyclic_objects[1].parent_id = Some(cyclic_objects[2].id);
        cyclic_objects[2].parent_id = Some(cyclic_objects[1].id);
        assert!(World::try_new(world.origin, world.sources.clone(), cyclic_objects).is_err());
    }
}

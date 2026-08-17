//! Deterministic compilation of the locked hero-area vector sources.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt::{Display, Formatter, Write as _},
};

use isometric_core::{ObjectId, WorldPoint};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    Confidence, Geometry, MaterialId, Polygon, ReviewStatus, Ring, Roof, RoofKind, SemanticClass,
    SourceId, SourceProvenance, World, WorldError, WorldObject, WorldObjectInput, WorldOrigin,
};

const ARTIFACT_SCHEMA: &str = "isometric-world/v1";
const MANIFEST_SCHEMA: &str = "isometric-world-manifest/v1";
const OSM_SOURCE: &str = "osm-2026-07-15-hero";
const OVERTURE_SOURCE: &str = "overture-2026-06-17-buildings";
const ORIGIN_EASTING_MM: i64 = 573_200_000;
const ORIGIN_NORTHING_MM: i64 = 4_142_200_000;
const UNKNOWN_CELL_MM: i64 = 20_000;
const HERO_LANDMARKS: [&str; 3] = ["Hoover Tower", "Main Quad", "Memorial Church"];

/// Summary suitable for CLI inspection and CI assertions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompileReport {
    /// Total accepted semantic objects, including explicit unknown cells.
    pub object_count: usize,
    /// Accepted objects by stable semantic class name.
    pub objects_by_class: BTreeMap<String, usize>,
    /// Count of source geometries rejected by canonical validation.
    pub rejected_geometry_count: usize,
    /// Count of OSM construction features intentionally excluded.
    pub excluded_construction_count: usize,
    /// Fraction of 20 meter review cells that remain unknown.
    pub unknown_fraction_ppm: u32,
    /// Required landmark names observed in the locked sources.
    pub landmarks: Vec<String>,
    /// Locked sources intentionally deferred to later compilation stages.
    pub deferred_source_ids: Vec<String>,
}

/// A validated hero world plus portable deterministic artifacts.
#[derive(Debug)]
pub struct CompiledHero {
    /// Immutable canonical world used by renderers and validators.
    pub world: World,
    /// Canonical, newline-terminated JSON world artifact.
    pub world_json: String,
    /// Canonical, newline-terminated manifest with a verified hash chain.
    pub manifest_json: String,
    /// Compilation evidence summary.
    pub report: CompileReport,
}

/// Fail-closed hero compilation error.
#[derive(Debug)]
pub struct CompileError(String);

impl Display for CompileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for CompileError {}

impl From<serde_json::Error> for CompileError {
    fn from(value: serde_json::Error) -> Self {
        Self(format!("source JSON failed: {value}"))
    }
}

impl From<WorldError> for CompileError {
    fn from(value: WorldError) -> Self {
        Self(value.to_string())
    }
}

#[derive(Deserialize)]
struct OsmDocument {
    elements: Vec<OsmElement>,
}

#[derive(Deserialize)]
struct OsmElement {
    #[serde(rename = "type")]
    kind: String,
    id: u64,
    #[serde(default)]
    tags: BTreeMap<String, String>,
    geometry: Option<Vec<LonLat>>,
}

#[derive(Clone, Copy, Deserialize)]
struct LonLat {
    lat: f64,
    lon: f64,
}

#[derive(Deserialize)]
struct FeatureCollection {
    features: Vec<GeoFeature>,
}

#[derive(Deserialize)]
struct GeoFeature {
    geometry: GeoGeometry,
    properties: Value,
}

#[derive(Deserialize)]
struct GeoGeometry {
    #[serde(rename = "type")]
    kind: String,
    coordinates: Value,
}

#[derive(Serialize)]
struct Artifact<'a> {
    schema: &'static str,
    region_id: &'static str,
    crs: &'static str,
    origin: ArtifactOrigin,
    contains_source_pixels: bool,
    contains_transients: bool,
    sources: Vec<ArtifactSource<'a>>,
    features: Vec<ArtifactFeature<'a>>,
}

#[derive(Serialize)]
struct ArtifactOrigin {
    epsg: u32,
    easting_mm: i64,
    northing_mm: i64,
    elevation_mm: i64,
}

#[derive(Serialize)]
struct ArtifactSource<'a> {
    id: &'a str,
    license: &'a str,
    attribution: &'a str,
}

#[derive(Serialize)]
struct ArtifactFeature<'a> {
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    class: &'static str,
    geometry: ArtifactGeometry,
    height_mm: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    floor_count: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    roof: Option<ArtifactRoof>,
    #[serde(skip_serializing_if = "Option::is_none")]
    material: Option<&'a str>,
    confidence_bp: u16,
    source_ids: Vec<&'a str>,
    review: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    review_note: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<u64>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ArtifactGeometry {
    Polygon { rings: Vec<Vec<[i64; 3]>> },
    Multipolygon { polygons: Vec<ArtifactPolygon> },
}

#[derive(Serialize)]
struct ArtifactPolygon {
    rings: Vec<Vec<[i64; 3]>>,
}

#[derive(Serialize)]
struct ArtifactRoof {
    kind: &'static str,
    direction_millidegrees: u32,
}

#[derive(Serialize)]
struct Manifest<'a> {
    schema: &'static str,
    status: &'static str,
    semantic_version: &'static str,
    region_id: &'static str,
    world_sha256: String,
    object_count: usize,
    partition_count: usize,
    unknown_fraction_ppm: u32,
    source_sha256: BTreeMap<&'a str, String>,
    deferred_source_ids: &'a [String],
    landmarks: &'a [String],
    dirty_bounds: Vec<String>,
}

struct BuildState {
    objects: Vec<WorldObject>,
    used_osm_buildings: BTreeSet<u64>,
    rejected_geometry_count: usize,
    excluded_construction_count: usize,
}

/// Compiles the exact locked OSM and Overture vector bytes into a hero world.
///
/// # Errors
///
/// Returns an error for malformed source data, missing landmark evidence, an
/// unsupported geographic coordinate, or any canonical-world invariant.
pub fn compile_hero(osm_json: &[u8], overture_json: &[u8]) -> Result<CompiledHero, CompileError> {
    let osm: OsmDocument = serde_json::from_slice(osm_json)?;
    let overture: FeatureCollection = serde_json::from_slice(overture_json)?;
    let osm_way_tags = osm
        .elements
        .iter()
        .filter(|element| element.kind == "way")
        .map(|element| (element.id, element.tags.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut landmarks = osm
        .elements
        .iter()
        .filter_map(|element| element.tags.get("name"))
        .filter(|name| HERO_LANDMARKS.contains(&name.as_str()))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    landmarks.sort();
    if landmarks.len() != HERO_LANDMARKS.len() {
        return Err(CompileError(format!(
            "locked OSM source is missing required landmark evidence: {landmarks:?}"
        )));
    }

    let mut state = BuildState {
        objects: Vec::new(),
        used_osm_buildings: BTreeSet::new(),
        rejected_geometry_count: 0,
        excluded_construction_count: 0,
    };
    compile_overture_buildings(&overture, &osm_way_tags, &mut state)?;
    compile_osm(&osm, &mut state)?;
    let unknown_cells = add_unknown_cells(&mut state)?;

    let sources = vec![
        SourceProvenance {
            id: SourceId::new(OSM_SOURCE)?,
            license: "ODbL-1.0".into(),
            attribution: "OpenStreetMap contributors".into(),
        },
        SourceProvenance {
            id: SourceId::new(OVERTURE_SOURCE)?,
            license: "ODbL-1.0".into(),
            attribution: "OpenStreetMap contributors and Overture Maps Foundation".into(),
        },
    ];
    let world = World::try_new(
        WorldOrigin::new(26_910, ORIGIN_EASTING_MM, ORIGIN_NORTHING_MM, 0)?,
        sources,
        state.objects,
    )?;

    let cell_count = hero_grid_dimensions().0 * hero_grid_dimensions().1;
    let unknown_fraction_ppm = u32::try_from((unknown_cells * 1_000_000) / cell_count)
        .map_err(|_| CompileError("unknown fraction overflowed".into()))?;
    let mut objects_by_class = BTreeMap::new();
    for object in world.objects() {
        *objects_by_class
            .entry(class_name(object.class()).to_owned())
            .or_insert(0) += 1;
    }
    let deferred_source_ids = vec![
        "naip-2024-hero".into(),
        "usgs-lidar-07509800".into(),
        "usgs-lidar-07509825".into(),
        "usgs-lidar-07759800".into(),
        "usgs-lidar-07759825".into(),
    ];
    let report = CompileReport {
        object_count: world.objects().len(),
        objects_by_class,
        rejected_geometry_count: state.rejected_geometry_count,
        excluded_construction_count: state.excluded_construction_count,
        unknown_fraction_ppm,
        landmarks,
        deferred_source_ids,
    };
    let world_json = canonical_json(&world)?;
    let source_sha256 = BTreeMap::from([
        (OSM_SOURCE, sha256(osm_json)),
        (OVERTURE_SOURCE, sha256(overture_json)),
    ]);
    let manifest = Manifest {
        schema: MANIFEST_SCHEMA,
        status: "prototype-vector-world",
        semantic_version: "0.2.0",
        region_id: "stanford-hero-v1",
        world_sha256: sha256(world_json.as_bytes()),
        object_count: report.object_count,
        partition_count: world.partitions().len(),
        unknown_fraction_ppm,
        source_sha256,
        deferred_source_ids: &report.deferred_source_ids,
        landmarks: &report.landmarks,
        dirty_bounds: Vec::new(),
    };
    let manifest_json = pretty_json(&manifest)?;
    Ok(CompiledHero {
        world,
        world_json,
        manifest_json,
        report,
    })
}

fn compile_overture_buildings(
    collection: &FeatureCollection,
    osm_way_tags: &BTreeMap<u64, BTreeMap<String, String>>,
    state: &mut BuildState,
) -> Result<(), CompileError> {
    for feature in &collection.features {
        let record_id = feature
            .properties
            .get("sources")
            .and_then(Value::as_array)
            .and_then(|sources| sources.first())
            .and_then(|source| source.get("record_id"))
            .and_then(Value::as_str)
            .map_or_else(
                || {
                    format!(
                        "geometry-{}",
                        sha256(feature.geometry.coordinates.to_string().as_bytes())
                    )
                },
                ToOwned::to_owned,
            );
        let osm_id = osm_way_id(&record_id);
        let tags = osm_id.and_then(|id| osm_way_tags.get(&id));
        let name = tags.and_then(|tags| tags.get("name")).cloned();
        let height = feature.properties.get("height").and_then(Value::as_f64);
        let floors = feature
            .properties
            .get("num_floors")
            .and_then(Value::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .or_else(|| tags.and_then(|tags| parse_u16(tags.get("building:levels"))));
        let height_mm = height
            .map(meters_to_mm)
            .transpose()?
            .or_else(|| floors.map(|value| u32::from(value) * 3_600))
            .unwrap_or(9_000);
        let Ok(geometry) = geojson_geometry(&feature.geometry) else {
            state.rejected_geometry_count += 1;
            continue;
        };
        let heuristic_height = height.is_none() && floors.is_none();
        let mut source_ids = vec![SourceId::new(OVERTURE_SOURCE)?];
        if let Some(id) = osm_id {
            state.used_osm_buildings.insert(id);
            if tags.is_some() {
                source_ids.insert(0, SourceId::new(OSM_SOURCE)?);
            }
        }
        let roof = roof_from_tags(tags)?;
        state.objects.push(WorldObject::try_new(WorldObjectInput {
            id: stable_id(&format!("overture:{record_id}"))?,
            name,
            class: SemanticClass::Building,
            geometry,
            height_mm,
            floor_count: floors,
            roof,
            material: Some(MaterialId::new("sandstone")?),
            confidence: Confidence::new(if heuristic_height { 7_000 } else { 9_500 })?,
            source_ids,
            review: if heuristic_height {
                ReviewStatus::AcceptedOverride
            } else {
                ReviewStatus::Accepted
            },
            review_note: heuristic_height.then(|| {
                "No height evidence in the locked vector sources; prototype default is 9 m".into()
            }),
            parent_id: None,
        })?);
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn compile_osm(document: &OsmDocument, state: &mut BuildState) -> Result<(), CompileError> {
    for element in &document.elements {
        if element.kind != "way" {
            continue;
        }
        if element
            .tags
            .get("highway")
            .is_some_and(|value| value == "construction")
            || element.tags.contains_key("construction")
        {
            state.excluded_construction_count += 1;
            continue;
        }
        let Some(points) = element.geometry.as_deref() else {
            continue;
        };
        if element.tags.contains_key("building") || element.tags.contains_key("building:part") {
            if state.used_osm_buildings.contains(&element.id) {
                continue;
            }
            if let Ok(geometry) = lon_lat_polygon(points) {
                let floors = parse_u16(element.tags.get("building:levels"));
                let explicit_height = parse_f64(element.tags.get("height"));
                let height_mm = explicit_height
                    .map(meters_to_mm)
                    .transpose()?
                    .or_else(|| floors.map(|value| u32::from(value) * 3_600))
                    .unwrap_or(9_000);
                let heuristic = explicit_height.is_none() && floors.is_none();
                state.objects.push(WorldObject::try_new(WorldObjectInput {
                    id: stable_id(&format!("osm:way:{}:building", element.id))?,
                    name: element.tags.get("name").cloned(),
                    class: SemanticClass::Building,
                    geometry,
                    height_mm,
                    floor_count: floors,
                    roof: roof_from_tags(Some(&element.tags))?,
                    material: Some(MaterialId::new("sandstone")?),
                    confidence: Confidence::new(if heuristic { 6_500 } else { 8_500 })?,
                    source_ids: vec![SourceId::new(OSM_SOURCE)?],
                    review: if heuristic {
                        ReviewStatus::AcceptedOverride
                    } else {
                        ReviewStatus::Accepted
                    },
                    review_note: heuristic.then(|| {
                        "OSM-only building has no locked height; prototype default is 9 m".into()
                    }),
                    parent_id: None,
                })?);
            } else {
                state.rejected_geometry_count += 1;
            }
            continue;
        }
        if let Some(highway) = element.tags.get("highway") {
            let class = if matches!(
                highway.as_str(),
                "footway" | "pedestrian" | "cycleway" | "steps" | "corridor"
            ) {
                SemanticClass::Path
            } else {
                SemanticClass::Road
            };
            let width_mm = highway_width_mm(highway, element.tags.get("width"));
            for (index, segment) in points.windows(2).enumerate() {
                if let Ok(geometry) = buffered_segment(segment[0], segment[1], width_mm) {
                    state.objects.push(WorldObject::try_new(WorldObjectInput {
                        id: stable_id(&format!("osm:way:{}:segment:{index}", element.id))?,
                        name: element.tags.get("name").cloned(),
                        class,
                        geometry,
                        height_mm: 0,
                        floor_count: None,
                        roof: None,
                        material: Some(MaterialId::new(if class == SemanticClass::Path {
                            "path"
                        } else {
                            "asphalt"
                        })?),
                        confidence: Confidence::new(9_000)?,
                        source_ids: vec![SourceId::new(OSM_SOURCE)?],
                        review: ReviewStatus::Accepted,
                        review_note: None,
                        parent_id: None,
                    })?);
                }
            }
            continue;
        }
        let Some((class, material)) = surface_class(&element.tags) else {
            continue;
        };
        match lon_lat_polygon(points) {
            Ok(geometry) => state.objects.push(WorldObject::try_new(WorldObjectInput {
                id: stable_id(&format!("osm:way:{}:surface", element.id))?,
                name: element.tags.get("name").cloned(),
                class,
                geometry,
                height_mm: if class == SemanticClass::Vegetation {
                    12_000
                } else {
                    0
                },
                floor_count: None,
                roof: None,
                material: Some(MaterialId::new(material)?),
                confidence: Confidence::new(8_500)?,
                source_ids: vec![SourceId::new(OSM_SOURCE)?],
                review: ReviewStatus::Accepted,
                review_note: None,
                parent_id: None,
            })?),
            Err(_) => state.rejected_geometry_count += 1,
        }
    }
    Ok(())
}

fn add_unknown_cells(state: &mut BuildState) -> Result<usize, CompileError> {
    let (columns, rows) = hero_grid_dimensions();
    let known = state
        .objects
        .iter()
        .map(|object| object.geometry().clone())
        .collect::<Vec<_>>();
    let mut count = 0_usize;
    for row in 0..rows {
        for column in 0..columns {
            let min_x =
                hero_local_bounds().0 + i64::try_from(column).unwrap_or(0) * UNKNOWN_CELL_MM;
            let min_y = hero_local_bounds().1 + i64::try_from(row).unwrap_or(0) * UNKNOWN_CELL_MM;
            let center =
                WorldPoint::new(min_x + UNKNOWN_CELL_MM / 2, min_y + UNKNOWN_CELL_MM / 2, 0);
            if known
                .iter()
                .any(|geometry| geometry_contains(geometry, center))
            {
                continue;
            }
            count += 1;
            let geometry = rectangle_geometry(
                min_x,
                min_y,
                min_x + UNKNOWN_CELL_MM,
                min_y + UNKNOWN_CELL_MM,
            )?;
            state.objects.push(WorldObject::try_new(WorldObjectInput {
                id: stable_id(&format!("unknown:{column}:{row}"))?,
                name: None,
                class: SemanticClass::Unknown,
                geometry,
                height_mm: 0,
                floor_count: None,
                roof: None,
                material: None,
                confidence: Confidence::new(0)?,
                source_ids: vec![SourceId::new(OSM_SOURCE)?],
                review: ReviewStatus::UnreviewedConflict,
                review_note: Some("No accepted vector land-cover evidence; deferred to locked NAIP and LiDAR compilation".into()),
                parent_id: None,
            })?);
        }
    }
    Ok(count)
}

fn geojson_geometry(raw: &GeoGeometry) -> Result<Geometry, CompileError> {
    match raw.kind.as_str() {
        "Polygon" => polygon_from_value(&raw.coordinates).map(Geometry::Polygon),
        "MultiPolygon" => {
            let polygons = raw
                .coordinates
                .as_array()
                .ok_or_else(|| CompileError("GeoJSON multipolygon coordinates are invalid".into()))?
                .iter()
                .map(polygon_from_value)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Geometry::try_multipolygon(polygons)?)
        }
        _ => Err(CompileError(
            "Overture building geometry is not polygonal".into(),
        )),
    }
}

fn polygon_from_value(value: &Value) -> Result<Polygon, CompileError> {
    let rings = value
        .as_array()
        .ok_or_else(|| CompileError("GeoJSON polygon coordinates are invalid".into()))?
        .iter()
        .map(|ring| {
            let points = ring
                .as_array()
                .ok_or_else(|| CompileError("GeoJSON ring is invalid".into()))?
                .iter()
                .map(|coordinate| {
                    let pair = coordinate
                        .as_array()
                        .ok_or_else(|| CompileError("GeoJSON coordinate is invalid".into()))?;
                    if pair.len() < 2 {
                        return Err(CompileError("GeoJSON coordinate is incomplete".into()));
                    }
                    projected_point(
                        pair[0]
                            .as_f64()
                            .ok_or_else(|| CompileError("longitude is invalid".into()))?,
                        pair[1]
                            .as_f64()
                            .ok_or_else(|| CompileError("latitude is invalid".into()))?,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ring::try_new(deduplicate_ring(points)).map_err(CompileError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Polygon::try_new(rings).map_err(CompileError::from)
}

fn lon_lat_polygon(points: &[LonLat]) -> Result<Geometry, CompileError> {
    let projected = points
        .iter()
        .map(|point| projected_point(point.lon, point.lat))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Geometry::Polygon(Polygon::try_new(vec![Ring::try_new(
        deduplicate_ring(projected),
    )?])?))
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "bounded local millimeter deltas are exactly representable and rounded deliberately"
)]
fn buffered_segment(start: LonLat, end: LonLat, width_mm: i64) -> Result<Geometry, CompileError> {
    let start = projected_point(start.lon, start.lat)?;
    let end = projected_point(end.lon, end.lat)?;
    let dx = (end.x_mm - start.x_mm) as f64;
    let dy = (end.y_mm - start.y_mm) as f64;
    let length = dx.hypot(dy);
    if length < 1.0 {
        return Err(CompileError("zero-length OSM segment".into()));
    }
    let half = width_mm as f64 / 2.0;
    let offset_x = (-dy / length * half).round() as i64;
    let offset_y = (dx / length * half).round() as i64;
    let points = vec![
        WorldPoint::new(start.x_mm + offset_x, start.y_mm + offset_y, 0),
        WorldPoint::new(end.x_mm + offset_x, end.y_mm + offset_y, 0),
        WorldPoint::new(end.x_mm - offset_x, end.y_mm - offset_y, 0),
        WorldPoint::new(start.x_mm - offset_x, start.y_mm - offset_y, 0),
        WorldPoint::new(start.x_mm + offset_x, start.y_mm + offset_y, 0),
    ];
    Ok(Geometry::Polygon(Polygon::try_new(vec![Ring::try_new(
        points,
    )?])?))
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "validated Stanford coordinates are rounded into bounded local millimeters"
)]
fn projected_point(longitude: f64, latitude: f64) -> Result<WorldPoint, CompileError> {
    if !(-122.3..=-122.0).contains(&longitude) || !(37.3..=37.6).contains(&latitude) {
        return Err(CompileError(
            "coordinate lies outside the Stanford guard bounds".into(),
        ));
    }
    let (easting_m, northing_m) = wgs84_to_utm10(longitude, latitude);
    Ok(WorldPoint::new(
        (easting_m * 1_000.0).round() as i64 - ORIGIN_EASTING_MM,
        (northing_m * 1_000.0).round() as i64 - ORIGIN_NORTHING_MM,
        0,
    ))
}

#[allow(clippy::many_single_char_names)]
fn wgs84_to_utm10(longitude: f64, latitude: f64) -> (f64, f64) {
    let a = 6_378_137.0_f64;
    let e2 = 0.006_694_379_990_141_316_5_f64;
    let ep2 = e2 / (1.0 - e2);
    let k0 = 0.9996_f64;
    let lat = latitude.to_radians();
    let lon_delta = (longitude + 123.0).to_radians();
    let sin_lat = lat.sin();
    let cos_lat = lat.cos();
    let tan_lat = lat.tan();
    let n = a / (1.0 - e2 * sin_lat * sin_lat).sqrt();
    let t = tan_lat * tan_lat;
    let c = ep2 * cos_lat * cos_lat;
    let aa = cos_lat * lon_delta;
    let e4 = e2 * e2;
    let e6 = e4 * e2;
    let m = a
        * ((1.0 - e2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0) * lat
            - (3.0 * e2 / 8.0 + 3.0 * e4 / 32.0 + 45.0 * e6 / 1_024.0) * (2.0 * lat).sin()
            + (15.0 * e4 / 256.0 + 45.0 * e6 / 1_024.0) * (4.0 * lat).sin()
            - 35.0 * e6 / 3_072.0 * (6.0 * lat).sin());
    let easting = 500_000.0
        + k0 * n
            * (aa
                + (1.0 - t + c) * aa.powi(3) / 6.0
                + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * ep2) * aa.powi(5) / 120.0);
    let northing = k0
        * (m + n
            * tan_lat
            * (aa * aa / 2.0
                + (5.0 - t + 9.0 * c + 4.0 * c * c) * aa.powi(4) / 24.0
                + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * ep2) * aa.powi(6) / 720.0));
    (easting, northing)
}

fn roof_from_tags(tags: Option<&BTreeMap<String, String>>) -> Result<Option<Roof>, CompileError> {
    let Some(tags) = tags else { return Ok(None) };
    let kind = match tags.get("roof:shape").map(String::as_str) {
        Some("gabled") => RoofKind::Gabled,
        Some("hipped") => RoofKind::Hipped,
        Some("pyramidal") => RoofKind::Pyramidal,
        Some("skillion") => RoofKind::Shed,
        Some("flat") => RoofKind::Flat,
        Some(_) => RoofKind::Unknown,
        None => return Ok(None),
    };
    let direction = parse_f64(tags.get("roof:direction")).map_or(0, roof_direction);
    Ok(Some(Roof::new(kind, direction)?))
}

fn surface_class(tags: &BTreeMap<String, String>) -> Option<(SemanticClass, &'static str)> {
    if tags.get("natural").is_some_and(|value| value == "water") {
        Some((SemanticClass::Water, "water"))
    } else if tags.get("natural").is_some_and(|value| value == "wood") {
        Some((SemanticClass::Vegetation, "broadleaf"))
    } else if tags.get("amenity").is_some_and(|value| value == "parking") {
        Some((SemanticClass::Parking, "asphalt"))
    } else if tags.get("leisure").is_some_and(|value| value == "pitch") {
        Some((SemanticClass::AthleticSurface, "athletic-turf"))
    } else if tags.contains_key("landuse")
        || tags
            .get("leisure")
            .is_some_and(|value| matches!(value.as_str(), "park" | "common" | "garden"))
    {
        Some((SemanticClass::Terrain, "grass"))
    } else {
        None
    }
}

fn highway_width_mm(highway: &str, explicit: Option<&String>) -> i64 {
    let default = match highway {
        "tertiary" => 10_000,
        "residential" | "unclassified" => 7_000,
        "service" | "living_street" => 5_000,
        "pedestrian" => 4_000,
        "cycleway" => 2_500,
        "steps" => 2_000,
        _ => 2_200,
    };
    parse_f64(explicit)
        .and_then(|meters| meters_to_mm(meters).ok())
        .map_or(default, i64::from)
}

fn parse_u16(value: Option<&String>) -> Option<u16> {
    value?.split(';').next()?.trim().parse().ok()
}

fn parse_f64(value: Option<&String>) -> Option<f64> {
    value?.split(';').next()?.trim().parse().ok()
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the finite nonnegative range is checked immediately before rounding"
)]
fn meters_to_mm(value: f64) -> Result<u32, CompileError> {
    if !value.is_finite() || !(0.0..=1_000.0).contains(&value) {
        return Err(CompileError(
            "height or width is outside accepted limits".into(),
        ));
    }
    Ok((value * 1_000.0).round() as u32)
}

fn osm_way_id(record_id: &str) -> Option<u64> {
    record_id.strip_prefix('w')?.split('@').next()?.parse().ok()
}

fn stable_id(identity: &str) -> Result<ObjectId, CompileError> {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in identity.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    ObjectId::new(hash).map_err(|_| CompileError("stable identity hashed to zero".into()))
}

fn deduplicate_ring(points: Vec<WorldPoint>) -> Vec<WorldPoint> {
    let mut output = Vec::with_capacity(points.len());
    for point in points {
        if output.last() != Some(&point) {
            output.push(point);
        }
    }
    if output.len() >= 3 && output.first() != output.last() {
        output.push(output[0]);
    }
    output
}

fn rectangle_geometry(
    min_x: i64,
    min_y: i64,
    max_x: i64,
    max_y: i64,
) -> Result<Geometry, CompileError> {
    Ok(Geometry::Polygon(Polygon::try_new(vec![Ring::try_new(
        vec![
            WorldPoint::new(min_x, min_y, 0),
            WorldPoint::new(max_x, min_y, 0),
            WorldPoint::new(max_x, max_y, 0),
            WorldPoint::new(min_x, max_y, 0),
            WorldPoint::new(min_x, min_y, 0),
        ],
    )?])?))
}

fn hero_local_bounds() -> (i64, i64, i64, i64) {
    let south_west = projected_point(-122.1722, 37.4245).expect("accepted hero bounds project");
    let north_east = projected_point(-122.1653, 37.4299).expect("accepted hero bounds project");
    (
        south_west.x_mm,
        south_west.y_mm,
        north_east.x_mm,
        north_east.y_mm,
    )
}

fn hero_grid_dimensions() -> (usize, usize) {
    let (min_x, min_y, max_x, max_y) = hero_local_bounds();
    (
        usize::try_from((max_x - min_x + UNKNOWN_CELL_MM - 1) / UNKNOWN_CELL_MM).unwrap_or(0),
        usize::try_from((max_y - min_y + UNKNOWN_CELL_MM - 1) / UNKNOWN_CELL_MM).unwrap_or(0),
    )
}

fn geometry_contains(geometry: &Geometry, point: WorldPoint) -> bool {
    let polygons: &[Polygon] = match geometry {
        Geometry::Polygon(polygon) => std::slice::from_ref(polygon),
        Geometry::MultiPolygon(polygons) => polygons,
    };
    polygons.iter().any(|polygon| {
        point_in_ring(point, &polygon.rings()[0])
            && polygon.rings()[1..]
                .iter()
                .all(|hole| !point_in_ring(point, hole))
    })
}

fn point_in_ring(point: WorldPoint, ring: &Ring) -> bool {
    let mut inside = false;
    for edge in ring.points().windows(2) {
        let (left, right) = (edge[0], edge[1]);
        if (left.y_mm > point.y_mm) != (right.y_mm > point.y_mm) {
            let delta_y = i128::from(right.y_mm - left.y_mm);
            let left_side = i128::from(point.x_mm - left.x_mm) * delta_y;
            let right_side =
                i128::from(right.x_mm - left.x_mm) * i128::from(point.y_mm - left.y_mm);
            if (delta_y > 0 && left_side < right_side) || (delta_y < 0 && left_side > right_side) {
                inside = !inside;
            }
        }
    }
    inside
}

fn canonical_json(world: &World) -> Result<String, CompileError> {
    let artifact = Artifact {
        schema: ARTIFACT_SCHEMA,
        region_id: "stanford-hero-v1",
        crs: "local integer millimeters derived from EPSG:26910",
        origin: ArtifactOrigin {
            epsg: world.origin().epsg(),
            easting_mm: world.origin().easting_mm(),
            northing_mm: world.origin().northing_mm(),
            elevation_mm: world.origin().elevation_mm(),
        },
        contains_source_pixels: false,
        contains_transients: false,
        sources: world
            .sources()
            .iter()
            .map(|source| ArtifactSource {
                id: source.id.as_str(),
                license: &source.license,
                attribution: &source.attribution,
            })
            .collect(),
        features: world.objects().iter().map(artifact_feature).collect(),
    };
    pretty_json(&artifact)
}

fn artifact_feature(object: &WorldObject) -> ArtifactFeature<'_> {
    ArtifactFeature {
        id: object.id().get(),
        name: object.name(),
        class: class_name(object.class()),
        geometry: artifact_geometry(object.geometry()),
        height_mm: object.height_mm(),
        floor_count: object.floor_count(),
        roof: object.roof().map(|roof| ArtifactRoof {
            kind: roof_name(roof.kind()),
            direction_millidegrees: roof.direction_millidegrees(),
        }),
        material: object.material().map(MaterialId::as_str),
        confidence_bp: object.confidence().basis_points(),
        source_ids: object.source_ids().iter().map(SourceId::as_str).collect(),
        review: review_name(object.review()),
        review_note: object.review_note(),
        parent_id: object.parent_id().map(ObjectId::get),
    }
}

fn artifact_geometry(geometry: &Geometry) -> ArtifactGeometry {
    let polygon = |polygon: &Polygon| ArtifactPolygon {
        rings: polygon
            .rings()
            .iter()
            .map(|ring| {
                ring.points()
                    .iter()
                    .map(|point| [point.x_mm, point.y_mm, point.z_mm])
                    .collect()
            })
            .collect(),
    };
    match geometry {
        Geometry::Polygon(value) => ArtifactGeometry::Polygon {
            rings: polygon(value).rings,
        },
        Geometry::MultiPolygon(values) => ArtifactGeometry::Multipolygon {
            polygons: values.iter().map(polygon).collect(),
        },
    }
}

fn pretty_json<T: Serialize>(value: &T) -> Result<String, CompileError> {
    let mut json = serde_json::to_string_pretty(value)?;
    json.push('\n');
    Ok(json)
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to a String cannot fail");
            output
        })
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "rem_euclid constrains the rounded result to 0 through 359999"
)]
fn roof_direction(degrees: f64) -> u32 {
    (degrees.rem_euclid(360.0) * 1_000.0).round() as u32
}

const fn class_name(class: SemanticClass) -> &'static str {
    match class {
        SemanticClass::Terrain => "terrain",
        SemanticClass::Water => "water",
        SemanticClass::Road => "road",
        SemanticClass::Path => "path",
        SemanticClass::AthleticSurface => "athletic-surface",
        SemanticClass::Parking => "parking",
        SemanticClass::Building => "building",
        SemanticClass::Vegetation => "vegetation",
        SemanticClass::Unknown => "unknown",
    }
}

const fn roof_name(kind: RoofKind) -> &'static str {
    match kind {
        RoofKind::Flat => "flat",
        RoofKind::Gabled => "gabled",
        RoofKind::Hipped => "hipped",
        RoofKind::Pyramidal => "pyramidal",
        RoofKind::Shed => "shed",
        RoofKind::Complex => "complex",
        RoofKind::Unknown => "unknown",
    }
}

const fn review_name(review: ReviewStatus) -> &'static str {
    match review {
        ReviewStatus::Accepted => "accepted",
        ReviewStatus::AcceptedOverride => "accepted-override",
        ReviewStatus::UnreviewedConflict => "unreviewed-conflict",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OSM: &[u8] = include_bytes!("../../../fixtures/sources/osm-2026-07-15-hero.json");
    const OVERTURE: &[u8] = include_bytes!("../../../fixtures/sources/overture-buildings.geojson");
    const EXPECTED_MANIFEST: &str = include_str!("../../../world.manifest.json");

    #[test]
    fn utm_projection_matches_proj_control_point() {
        let point = projected_point(-122.1722, 37.4245).expect("control point projects");
        assert!((point.x_mm - 43_580).abs() <= 10);
        assert!((point.y_mm - 86_780).abs() <= 10);
    }

    #[test]
    fn hero_compile_is_deterministic_and_inspectable() {
        let first = compile_hero(OSM, OVERTURE).expect("locked sources compile");
        let second = compile_hero(OSM, OVERTURE).expect("locked sources recompile");
        assert_eq!(first.world_json, second.world_json);
        assert_eq!(first.manifest_json, second.manifest_json);
        assert_eq!(first.manifest_json, EXPECTED_MANIFEST);
        assert_eq!(
            World::from_artifact_json(&first.world_json).expect("artifact round trips"),
            first.world
        );
        assert_eq!(first.report.landmarks.len(), 3);
        assert!(first.report.object_count > 500);
        assert!(first.report.objects_by_class["building"] >= 80);
        assert!(!first.world.partitions().is_empty());
    }

    #[test]
    fn source_collection_order_does_not_change_world() {
        let baseline = compile_hero(OSM, OVERTURE).expect("baseline compiles");
        let mut osm: Value = serde_json::from_slice(OSM).expect("OSM fixture parses");
        osm["elements"]
            .as_array_mut()
            .expect("OSM elements are an array")
            .reverse();
        let mut overture: Value =
            serde_json::from_slice(OVERTURE).expect("Overture fixture parses");
        overture["features"]
            .as_array_mut()
            .expect("Overture features are an array")
            .reverse();
        let reordered = compile_hero(
            &serde_json::to_vec(&osm).expect("OSM reencodes"),
            &serde_json::to_vec(&overture).expect("Overture reencodes"),
        )
        .expect("reordered inputs compile");
        assert_eq!(baseline.world_json, reordered.world_json);
    }

    #[test]
    fn malformed_source_fails_closed() {
        let error = compile_hero(b"{}", OVERTURE).expect_err("missing elements must fail");
        assert!(error.to_string().contains("source JSON failed"));
    }
}

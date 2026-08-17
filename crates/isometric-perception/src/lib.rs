//! Bounded, deterministic compilation of locked raster and point-cloud evidence.
//!
//! Perception produces semantic evidence only. Source pixels and point records
//! never cross into the canonical world or renderer.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt::{Display, Formatter, Write as _},
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use las::{PointDataBuilder, Reader};
use proj4rs::{Proj, adaptors::transform_vertex_2d};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tiff::{decoder::Decoder, decoder::DecodingResult, tags::Tag};

/// Portable semantic-evidence schema.
pub const ARTIFACT_SCHEMA: &str = "isometric-perception-evidence/v1";
/// Accepted prototype region identifier.
pub const REGION_ID: &str = "stanford-hero-v1";
/// Fixed local world origin in EPSG:26910.
pub const ORIGIN_EASTING_MM: i64 = 573_200_000;
/// Fixed local world origin in EPSG:26910.
pub const ORIGIN_NORTHING_MM: i64 = 4_142_200_000;
/// Review-cell edge length.
pub const CELL_SIZE_MM: i64 = 20_000;
/// First prototype review-cell easting relative to the world origin.
pub const GRID_MIN_X_MM: i64 = 43_583;
/// First prototype review-cell northing relative to the world origin.
pub const GRID_MIN_Y_MM: i64 = 86_784;
/// Number of prototype review-cell columns.
pub const GRID_COLUMNS: u16 = 31;
/// Number of prototype review-cell rows.
pub const GRID_ROWS: u16 = 31;

const LIDAR_CHUNK_POINTS: u64 = 250_000;
const STATE_PLANE_PROJ: &str = concat!(
    "+proj=lcc +lat_0=36.5 +lon_0=-120.5 ",
    "+lat_1=38.4333333333333 +lat_2=37.0666666666667 ",
    "+x_0=2000000.0001016 +y_0=500000.0001016 +ellps=GRS80 ",
    "+towgs84=0,0,0,0,0,0,0 +units=us-ft +no_defs +type=crs"
);
const UTM10_PROJ: &str = concat!(
    "+proj=utm +zone=10 +ellps=GRS80 ",
    "+towgs84=0,0,0,0,0,0,0 +units=m +no_defs +type=crs"
);

/// One stable grid address.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct CellIndex {
    /// Zero-based column from west to east.
    pub column: u16,
    /// Zero-based row from south to north.
    pub row: u16,
}

impl CellIndex {
    /// Creates a cell inside the fixed prototype grid.
    ///
    /// # Errors
    ///
    /// Returns an error when the address is outside the accepted grid.
    pub const fn new(column: u16, row: u16) -> Result<Self, PerceptionError> {
        if column >= GRID_COLUMNS || row >= GRID_ROWS {
            Err(PerceptionError::Invalid(
                "cell lies outside the prototype grid",
            ))
        } else {
            Ok(Self { column, row })
        }
    }

    fn flat(self) -> usize {
        usize::from(self.row) * usize::from(GRID_COLUMNS) + usize::from(self.column)
    }
}

/// Semantic classes that may cross the perception-to-world boundary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceClass {
    /// Persistent low terrain or landscaping.
    Terrain,
    /// Persistent water.
    Water,
    /// Persistent canopy or woodland.
    Vegetation,
    /// Conflicting evidence that must remain explicit.
    Unknown,
}

/// One locked `LiDAR` artifact and its source identity.
#[derive(Clone, Debug)]
pub struct LidarInput {
    /// Source-lock record identifier.
    pub source_id: String,
    /// Verified content-addressed file.
    pub path: PathBuf,
}

/// Complete immutable input to one perception compilation.
#[derive(Clone, Debug)]
pub struct CompileInput {
    /// Verified NAIP `GeoTIFF`.
    pub naip_path: PathBuf,
    /// Ordered verified `LiDAR` artifacts.
    pub lidar: Vec<LidarInput>,
    /// Exact source hashes from the approved source lock.
    pub source_sha256: BTreeMap<String, String>,
    /// Cells not already covered by accepted vector semantics.
    pub eligible_cells: BTreeSet<CellIndex>,
}

/// One reviewed semantic cell written to the portable artifact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceCell {
    /// Stable grid address.
    pub index: CellIndex,
    /// Persistent semantic class.
    pub class: EvidenceClass,
    /// Validated material grammar identifier.
    pub material: Option<String>,
    /// Canopy height or zero for surfaces.
    pub height_mm: u32,
    /// Fused confidence in basis points.
    pub confidence_bp: u16,
    /// Contributing approved source records in sorted order.
    pub source_ids: Vec<String>,
    /// Accepted NAIP samples after cell-level vector masking.
    pub naip_sample_count: u32,
    /// Fraction of accepted NAIP samples with positive vegetation evidence.
    pub naip_green_fraction_bp: u16,
    /// Accepted `LiDAR` points used for persistent-class evidence.
    pub lidar_sample_count: u32,
    /// Fraction of all in-cell `LiDAR` points classified as vegetation.
    pub lidar_vegetation_fraction_bp: u16,
    /// Fraction of all in-cell `LiDAR` points classified as building.
    pub lidar_building_fraction_bp: u16,
    /// Unclassified low elevated returns excluded as transient candidates.
    pub transient_masked_points: u32,
}

/// Portable frozen evidence consumed by the canonical world compiler.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceArtifact {
    /// Schema identifier.
    pub schema: String,
    /// Accepted geographic region.
    pub region_id: String,
    /// Artifact lifecycle state.
    pub status: String,
    /// Canonical compiler implementation.
    pub compiler: String,
    /// Final artifact never retains source pixels.
    pub contains_source_pixels: bool,
    /// Final artifact cannot carry transient semantic classes.
    pub contains_transients: bool,
    /// Fixed grid metadata.
    pub grid: GridMetadata,
    /// Exact locked input hashes.
    pub source_sha256: BTreeMap<String, String>,
    /// Number of cells excluded because vectors already own their semantics.
    pub vector_masked_cell_count: u16,
    /// Number of cells compiled from raster or point evidence.
    pub evidence_cell_count: u16,
    /// Total decoded NAIP samples that landed inside the accepted grid.
    pub naip_sample_count: u64,
    /// Total streamed `LiDAR` points that landed inside the accepted grid.
    pub lidar_sample_count: u64,
    /// Peak reusable LAZ point buffer, independent of total source size.
    pub lidar_chunk_points: u32,
    /// Stable semantic cells.
    pub cells: Vec<EvidenceCell>,
}

/// Fixed portable grid description.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GridMetadata {
    /// Horizontal coordinate reference system.
    pub crs: String,
    /// Absolute easting of the local world origin.
    pub origin_easting_mm: i64,
    /// Absolute northing of the local world origin.
    pub origin_northing_mm: i64,
    /// West edge of the first cell in local millimeters.
    pub min_x_mm: i64,
    /// South edge of the first cell in local millimeters.
    pub min_y_mm: i64,
    /// Cell edge length.
    pub cell_size_mm: i64,
    /// Number of columns.
    pub columns: u16,
    /// Number of rows.
    pub rows: u16,
}

impl GridMetadata {
    fn prototype() -> Self {
        Self {
            crs: "local integer millimeters derived from EPSG:26910".into(),
            origin_easting_mm: ORIGIN_EASTING_MM,
            origin_northing_mm: ORIGIN_NORTHING_MM,
            min_x_mm: GRID_MIN_X_MM,
            min_y_mm: GRID_MIN_Y_MM,
            cell_size_mm: CELL_SIZE_MM,
            columns: GRID_COLUMNS,
            rows: GRID_ROWS,
        }
    }
}

/// A validated artifact plus deterministic serialized bytes.
#[derive(Debug)]
pub struct CompiledEvidence {
    /// Validated structured evidence.
    pub artifact: EvidenceArtifact,
    /// Canonical pretty JSON with one trailing newline.
    pub artifact_json: String,
    /// SHA-256 over `artifact_json` bytes.
    pub artifact_sha256: String,
}

/// Fail-closed evidence compilation error.
#[derive(Debug)]
pub enum PerceptionError {
    /// A semantic or policy invariant failed.
    Invalid(&'static str),
    /// I/O failed.
    Io(std::io::Error),
    /// TIFF decoding failed.
    Tiff(tiff::TiffError),
    /// LAS or LAZ decoding failed.
    Las(las::Error),
    /// Projection construction or transformation failed.
    Projection(String),
    /// JSON encoding or decoding failed.
    Json(serde_json::Error),
}

impl Display for PerceptionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::Io(error) => write!(formatter, "perception I/O failed: {error}"),
            Self::Tiff(error) => write!(formatter, "NAIP TIFF failed: {error}"),
            Self::Las(error) => write!(formatter, "LiDAR LAZ failed: {error}"),
            Self::Projection(error) => write!(formatter, "coordinate projection failed: {error}"),
            Self::Json(error) => write!(formatter, "perception JSON failed: {error}"),
        }
    }
}

impl Error for PerceptionError {}

impl From<std::io::Error> for PerceptionError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<tiff::TiffError> for PerceptionError {
    fn from(value: tiff::TiffError) -> Self {
        Self::Tiff(value)
    }
}

impl From<las::Error> for PerceptionError {
    fn from(value: las::Error) -> Self {
        Self::Las(value)
    }
}

impl From<serde_json::Error> for PerceptionError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Clone)]
struct CellAccumulator {
    naip_samples: u32,
    green_samples: u32,
    dark_water_samples: u32,
    red_sum: u64,
    green_sum: u64,
    blue_sum: u64,
    nir_sum: u64,
    lidar_samples: u32,
    ground_samples: u32,
    low_vegetation_samples: u32,
    vegetation_samples: u32,
    building_samples: u32,
    water_samples: u32,
    ground_min_mm: i32,
    highest_vegetation_mm: i32,
    unclassified_height_bins: [u32; 256],
    lidar_source_ids: BTreeSet<String>,
}

impl Default for CellAccumulator {
    fn default() -> Self {
        Self {
            naip_samples: 0,
            green_samples: 0,
            dark_water_samples: 0,
            red_sum: 0,
            green_sum: 0,
            blue_sum: 0,
            nir_sum: 0,
            lidar_samples: 0,
            ground_samples: 0,
            low_vegetation_samples: 0,
            vegetation_samples: 0,
            building_samples: 0,
            water_samples: 0,
            ground_min_mm: i32::MAX,
            highest_vegetation_mm: i32::MIN,
            unclassified_height_bins: [0; 256],
            lidar_source_ids: BTreeSet::new(),
        }
    }
}

/// Compiles one frozen evidence artifact from verified locked sources.
///
/// # Errors
///
/// Returns an error for malformed georeferencing, unsupported source layouts,
/// source-order ambiguity, missing coverage, or any output policy violation.
pub fn compile(input: &CompileInput) -> Result<CompiledEvidence, PerceptionError> {
    validate_compile_input(input)?;
    let mut cells = vec![CellAccumulator::default(); grid_len()];
    let naip_samples = accumulate_naip(&input.naip_path, &mut cells)?;
    let mut lidar_samples = 0_u64;
    for lidar in &input.lidar {
        lidar_samples = lidar_samples
            .checked_add(accumulate_lidar(lidar, &mut cells)?)
            .ok_or(PerceptionError::Invalid("LiDAR sample count overflowed"))?;
    }

    let mut evidence_cells = input
        .eligible_cells
        .iter()
        .map(|index| classify_cell(*index, &cells[index.flat()]))
        .collect::<Result<Vec<_>, _>>()?;
    evidence_cells.sort_by_key(|cell| cell.index);
    let artifact = EvidenceArtifact {
        schema: ARTIFACT_SCHEMA.into(),
        region_id: REGION_ID.into(),
        status: "compiled-prototype-evidence".into(),
        compiler: "rust-naip-lidar-consensus-v1".into(),
        contains_source_pixels: false,
        contains_transients: false,
        grid: GridMetadata::prototype(),
        source_sha256: input.source_sha256.clone(),
        vector_masked_cell_count: u16::try_from(grid_len() - input.eligible_cells.len())
            .map_err(|_| PerceptionError::Invalid("masked cell count overflowed"))?,
        evidence_cell_count: u16::try_from(evidence_cells.len())
            .map_err(|_| PerceptionError::Invalid("evidence cell count overflowed"))?,
        naip_sample_count: naip_samples,
        lidar_sample_count: lidar_samples,
        lidar_chunk_points: u32::try_from(LIDAR_CHUNK_POINTS)
            .map_err(|_| PerceptionError::Invalid("LiDAR chunk count overflowed"))?,
        cells: evidence_cells,
    };
    artifact.validate()?;
    let mut artifact_json = serde_json::to_string_pretty(&artifact)?;
    artifact_json.push('\n');
    let artifact_sha256 = sha256(artifact_json.as_bytes());
    Ok(CompiledEvidence {
        artifact,
        artifact_json,
        artifact_sha256,
    })
}

impl EvidenceArtifact {
    /// Decodes and validates frozen evidence.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed JSON or any provenance, grid, class,
    /// transient, ordering, or coverage violation.
    pub fn from_json(json: &str) -> Result<Self, PerceptionError> {
        let artifact: Self = serde_json::from_str(json)?;
        artifact.validate()?;
        Ok(artifact)
    }

    /// Validates the portable evidence contract.
    ///
    /// # Errors
    ///
    /// Returns an error when the artifact cannot safely enter world fusion.
    pub fn validate(&self) -> Result<(), PerceptionError> {
        if self.schema != ARTIFACT_SCHEMA
            || self.region_id != REGION_ID
            || self.status != "compiled-prototype-evidence"
            || self.compiler != "rust-naip-lidar-consensus-v1"
            || self.contains_source_pixels
            || self.contains_transients
            || self.grid != GridMetadata::prototype()
        {
            return Err(PerceptionError::Invalid(
                "evidence policy metadata is invalid",
            ));
        }
        if self.source_sha256.len() != 5
            || self.source_sha256.iter().any(|(id, digest)| {
                id.is_empty()
                    || digest.len() != 64
                    || !digest
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            })
        {
            return Err(PerceptionError::Invalid(
                "evidence provenance hashes are invalid",
            ));
        }
        if usize::from(self.vector_masked_cell_count) + usize::from(self.evidence_cell_count)
            != grid_len()
            || usize::from(self.evidence_cell_count) != self.cells.len()
            || self.lidar_chunk_points != u32::try_from(LIDAR_CHUNK_POINTS).unwrap_or(0)
        {
            return Err(PerceptionError::Invalid(
                "evidence coverage accounting is invalid",
            ));
        }
        let mut previous = None;
        for cell in &self.cells {
            CellIndex::new(cell.index.column, cell.index.row)?;
            if previous.is_some_and(|index| index >= cell.index)
                || cell.naip_sample_count == 0
                || cell.source_ids.is_empty()
                || cell.source_ids.windows(2).any(|pair| pair[0] >= pair[1])
                || cell.confidence_bp > 10_000
                || cell.naip_green_fraction_bp > 10_000
                || cell.lidar_vegetation_fraction_bp > 10_000
                || cell.lidar_building_fraction_bp > 10_000
            {
                return Err(PerceptionError::Invalid(
                    "evidence cells are invalid or unsorted",
                ));
            }
            if cell.class == EvidenceClass::Unknown
                && (cell.material.is_some() || cell.height_mm != 0)
            {
                return Err(PerceptionError::Invalid(
                    "unknown evidence invents material or height",
                ));
            }
            previous = Some(cell.index);
        }
        Ok(())
    }

    /// Finds one cell by stable address.
    #[must_use]
    pub fn cell(&self, index: CellIndex) -> Option<&EvidenceCell> {
        self.cells
            .binary_search_by_key(&index, |cell| cell.index)
            .ok()
            .map(|position| &self.cells[position])
    }
}

fn validate_compile_input(input: &CompileInput) -> Result<(), PerceptionError> {
    if !input.naip_path.is_file()
        || input.lidar.len() != 4
        || input.lidar.iter().any(|value| !value.path.is_file())
        || input.eligible_cells.is_empty()
        || input.eligible_cells.len() >= grid_len()
    {
        return Err(PerceptionError::Invalid(
            "perception source set is incomplete",
        ));
    }
    let lidar_ids = input
        .lidar
        .iter()
        .map(|value| value.source_id.as_str())
        .collect::<Vec<_>>();
    if !lidar_ids.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(PerceptionError::Invalid(
            "LiDAR inputs must be uniquely sorted",
        ));
    }
    let expected = input
        .lidar
        .iter()
        .map(|value| value.source_id.as_str())
        .chain(std::iter::once("naip-2024-hero"))
        .collect::<BTreeSet<_>>();
    if input
        .source_sha256
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != expected
    {
        return Err(PerceptionError::Invalid(
            "perception source hashes are incomplete",
        ));
    }
    Ok(())
}

fn accumulate_naip(path: &Path, cells: &mut [CellAccumulator]) -> Result<u64, PerceptionError> {
    let file = File::open(path)?;
    let mut decoder = Decoder::new(BufReader::new(file))?;
    let (width, height) = decoder.dimensions()?;
    let color_type = decoder.colortype()?;
    if color_type.bit_depth() != 8 || color_type.num_samples() != 4 {
        return Err(PerceptionError::Invalid(
            "NAIP must be four-band U8 imagery",
        ));
    }
    let scale = decoder.get_tag_f64_vec(Tag::ModelPixelScaleTag)?;
    let tie = decoder.get_tag_f64_vec(Tag::ModelTiepointTag)?;
    if scale.len() < 2 || tie.len() < 6 || scale[0] <= 0.0 || scale[1] <= 0.0 {
        return Err(PerceptionError::Invalid(
            "NAIP georeferencing tags are invalid",
        ));
    }
    let DecodingResult::U8(samples) = decoder.read_image()? else {
        return Err(PerceptionError::Invalid(
            "NAIP decoder did not return U8 samples",
        ));
    };
    let expected_len = usize::try_from(width)
        .ok()
        .and_then(|value| value.checked_mul(usize::try_from(height).ok()?))
        .and_then(|value| value.checked_mul(4))
        .ok_or(PerceptionError::Invalid("NAIP dimensions overflowed"))?;
    if samples.len() != expected_len {
        return Err(PerceptionError::Invalid(
            "NAIP sample length is inconsistent",
        ));
    }
    let mut accepted = 0_u64;
    for row in 0..height {
        let latitude = tie[4] - (f64::from(row) + 0.5 - tie[1]) * scale[1];
        for column in 0..width {
            let longitude = tie[3] + (f64::from(column) + 0.5 - tie[0]) * scale[0];
            let (easting, northing) = wgs84_to_utm10(longitude, latitude);
            let Some(index) = cell_for_absolute_meters(easting, northing) else {
                continue;
            };
            let offset = (usize::try_from(row).unwrap_or(0) * usize::try_from(width).unwrap_or(0)
                + usize::try_from(column).unwrap_or(0))
                * 4;
            let red = samples[offset];
            let green = samples[offset + 1];
            let blue = samples[offset + 2];
            let nir = samples[offset + 3];
            let cell = &mut cells[index.flat()];
            cell.naip_samples += 1;
            cell.red_sum += u64::from(red);
            cell.green_sum += u64::from(green);
            cell.blue_sum += u64::from(blue);
            cell.nir_sum += u64::from(nir);
            let ndvi_numerator = i32::from(nir) - i32::from(red);
            let ndvi_denominator = i32::from(nir) + i32::from(red);
            if ndvi_denominator > 0 && ndvi_numerator * 10_000 >= ndvi_denominator * 1_000 {
                cell.green_samples += 1;
            }
            if u16::from(nir) < 55 && u16::from(red) + u16::from(green) + u16::from(blue) < 180 {
                cell.dark_water_samples += 1;
            }
            accepted += 1;
        }
    }
    Ok(accepted)
}

fn accumulate_lidar(
    input: &LidarInput,
    cells: &mut [CellAccumulator],
) -> Result<u64, PerceptionError> {
    let source = Proj::from_proj_string(STATE_PLANE_PROJ)
        .map_err(|error| PerceptionError::Projection(error.to_string()))?;
    let destination = Proj::from_proj_string(UTM10_PROJ)
        .map_err(|error| PerceptionError::Projection(error.to_string()))?;
    let mut reader = Reader::from_path(&input.path)?;
    let mut points = PointDataBuilder::new().for_header(reader.header()).build();
    let mut accepted = 0_u64;
    loop {
        let count = reader.fill_points(LIDAR_CHUNK_POINTS, &mut points)?;
        if count == 0 {
            break;
        }
        for (((x, y), z), classification) in points
            .x()
            .zip(points.y())
            .zip(points.z())
            .zip(points.classification())
        {
            let (easting, northing) = transform_vertex_2d(&source, &destination, (x, y))
                .map_err(|error| PerceptionError::Projection(error.to_string()))?;
            let Some(index) = cell_for_absolute_meters(easting, northing) else {
                continue;
            };
            let height_mm = feet_to_millimeters(z)?;
            let cell = &mut cells[index.flat()];
            cell.lidar_samples += 1;
            cell.lidar_source_ids.insert(input.source_id.clone());
            match classification {
                2 => {
                    cell.ground_samples += 1;
                    cell.ground_min_mm = cell.ground_min_mm.min(height_mm);
                }
                3 => cell.low_vegetation_samples += 1,
                4..=5 => {
                    cell.vegetation_samples += 1;
                    cell.highest_vegetation_mm = cell.highest_vegetation_mm.max(height_mm);
                }
                6 => cell.building_samples += 1,
                9 => cell.water_samples += 1,
                1 => {
                    let bin = height_mm.div_euclid(500).clamp(0, 255);
                    cell.unclassified_height_bins[usize::try_from(bin).unwrap_or(0)] += 1;
                }
                _ => {}
            }
            accepted += 1;
        }
    }
    Ok(accepted)
}

fn classify_cell(
    index: CellIndex,
    accumulator: &CellAccumulator,
) -> Result<EvidenceCell, PerceptionError> {
    if accumulator.naip_samples == 0 {
        return Err(PerceptionError::Invalid(
            "eligible cell lacks NAIP coverage",
        ));
    }
    let green_bp = ratio_bp(accumulator.green_samples, accumulator.naip_samples);
    let dark_water_bp = ratio_bp(accumulator.dark_water_samples, accumulator.naip_samples);
    let vegetation_bp = ratio_bp(accumulator.vegetation_samples, accumulator.lidar_samples);
    let building_bp = ratio_bp(accumulator.building_samples, accumulator.lidar_samples);
    let water_bp = ratio_bp(accumulator.water_samples, accumulator.lidar_samples);
    let transient_masked_points = transient_candidates(accumulator);

    let (class, material, height_mm, confidence_bp) = if building_bp >= 7_000 {
        (EvidenceClass::Unknown, None, 0, 2_500)
    } else if water_bp >= 2_500 || dark_water_bp >= 7_000 {
        (EvidenceClass::Water, Some("water".into()), 0, 8_000)
    } else if vegetation_bp >= 800 && accumulator.vegetation_samples >= 20 {
        let height = if accumulator.ground_min_mm == i32::MAX
            || accumulator.highest_vegetation_mm == i32::MIN
        {
            12_000
        } else {
            u32::try_from(
                (accumulator.highest_vegetation_mm - accumulator.ground_min_mm)
                    .clamp(3_000, 30_000),
            )
            .unwrap_or(12_000)
        };
        (
            EvidenceClass::Vegetation,
            Some("canopy".into()),
            height,
            8_500,
        )
    } else if green_bp >= 4_500 {
        (EvidenceClass::Terrain, Some("grass".into()), 0, 7_500)
    } else {
        (EvidenceClass::Terrain, Some("dry-grass".into()), 0, 7_000)
    };
    let mut source_ids = vec!["naip-2024-hero".to_owned()];
    source_ids.extend(accumulator.lidar_source_ids.iter().cloned());
    source_ids.sort();
    source_ids.dedup();
    Ok(EvidenceCell {
        index,
        class,
        material,
        height_mm,
        confidence_bp,
        source_ids,
        naip_sample_count: accumulator.naip_samples,
        naip_green_fraction_bp: green_bp,
        lidar_sample_count: accumulator
            .ground_samples
            .saturating_add(accumulator.low_vegetation_samples)
            .saturating_add(accumulator.vegetation_samples)
            .saturating_add(accumulator.building_samples)
            .saturating_add(accumulator.water_samples),
        lidar_vegetation_fraction_bp: vegetation_bp,
        lidar_building_fraction_bp: building_bp,
        transient_masked_points,
    })
}

fn transient_candidates(accumulator: &CellAccumulator) -> u32 {
    if accumulator.ground_min_mm == i32::MAX {
        return 0;
    }
    let first = usize::try_from(
        (accumulator.ground_min_mm + 500)
            .div_euclid(500)
            .clamp(0, 255),
    )
    .unwrap_or(0);
    let last = usize::try_from(
        (accumulator.ground_min_mm + 4_000)
            .div_euclid(500)
            .clamp(0, 255),
    )
    .unwrap_or(255);
    accumulator.unclassified_height_bins[first..=last]
        .iter()
        .copied()
        .sum()
}

fn ratio_bp(numerator: u32, denominator: u32) -> u16 {
    if denominator == 0 {
        return 0;
    }
    u16::try_from((u64::from(numerator) * 10_000 / u64::from(denominator)).min(10_000)).unwrap_or(0)
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "finite locked Stanford coordinates are rounded into bounded local millimeters"
)]
fn cell_for_absolute_meters(easting: f64, northing: f64) -> Option<CellIndex> {
    if !easting.is_finite() || !northing.is_finite() {
        return None;
    }
    let local_easting_mm = (easting * 1_000.0).round() as i64 - ORIGIN_EASTING_MM;
    let local_northing_mm = (northing * 1_000.0).round() as i64 - ORIGIN_NORTHING_MM;
    let column = (local_easting_mm - GRID_MIN_X_MM).div_euclid(CELL_SIZE_MM);
    let row = (local_northing_mm - GRID_MIN_Y_MM).div_euclid(CELL_SIZE_MM);
    let column = u16::try_from(column).ok()?;
    let row = u16::try_from(row).ok()?;
    CellIndex::new(column, row).ok()
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "locked point-cloud elevations are bounded before integer conversion"
)]
fn feet_to_millimeters(feet: f64) -> Result<i32, PerceptionError> {
    let value = feet * 304.800_609_601_219_2;
    if !(-1_000_000.0..=10_000_000.0).contains(&value) {
        return Err(PerceptionError::Invalid(
            "LiDAR elevation is outside accepted bounds",
        ));
    }
    Ok(value.round() as i32)
}

fn grid_len() -> usize {
    usize::from(GRID_COLUMNS) * usize::from(GRID_ROWS)
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to a String cannot fail");
            output
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn hashes() -> BTreeMap<String, String> {
        [
            "naip-2024-hero",
            "usgs-lidar-07509800",
            "usgs-lidar-07509825",
            "usgs-lidar-07759800",
            "usgs-lidar-07759825",
        ]
        .into_iter()
        .map(|id| (id.to_owned(), "a".repeat(64)))
        .collect()
    }

    #[test]
    fn state_plane_projection_matches_proj_control_point() {
        let source = Proj::from_proj_string(STATE_PLANE_PROJ).expect("state plane projection");
        let destination = Proj::from_proj_string(UTM10_PROJ).expect("UTM projection");
        let (easting, northing) =
            transform_vertex_2d(&source, &destination, (6_077_000.0, 1_983_000.0))
                .expect("control point transforms");
        assert!((easting - 573_501.321_083).abs() < 0.002);
        assert!((northing - 4_142_789.374_784).abs() < 0.002);
    }

    #[test]
    fn artifact_rejects_transients_and_unsorted_cells() {
        let mut artifact = EvidenceArtifact {
            schema: ARTIFACT_SCHEMA.into(),
            region_id: REGION_ID.into(),
            status: "compiled-prototype-evidence".into(),
            compiler: "rust-naip-lidar-consensus-v1".into(),
            contains_source_pixels: false,
            contains_transients: false,
            grid: GridMetadata::prototype(),
            source_sha256: hashes(),
            vector_masked_cell_count: 959,
            evidence_cell_count: 2,
            naip_sample_count: 2,
            lidar_sample_count: 0,
            lidar_chunk_points: 250_000,
            cells: vec![
                EvidenceCell {
                    index: CellIndex::new(0, 0).expect("valid cell"),
                    class: EvidenceClass::Terrain,
                    material: Some("grass".into()),
                    height_mm: 0,
                    confidence_bp: 7_500,
                    source_ids: vec!["naip-2024-hero".into()],
                    naip_sample_count: 1,
                    naip_green_fraction_bp: 0,
                    lidar_sample_count: 0,
                    lidar_vegetation_fraction_bp: 0,
                    lidar_building_fraction_bp: 0,
                    transient_masked_points: 0,
                },
                EvidenceCell {
                    index: CellIndex::new(1, 0).expect("valid cell"),
                    class: EvidenceClass::Terrain,
                    material: Some("grass".into()),
                    height_mm: 0,
                    confidence_bp: 7_500,
                    source_ids: vec!["naip-2024-hero".into()],
                    naip_sample_count: 1,
                    naip_green_fraction_bp: 0,
                    lidar_sample_count: 0,
                    lidar_vegetation_fraction_bp: 0,
                    lidar_building_fraction_bp: 0,
                    transient_masked_points: 0,
                },
            ],
        };
        artifact.validate().expect("baseline artifact is valid");
        artifact.contains_transients = true;
        assert!(artifact.validate().is_err());
        artifact.contains_transients = false;
        artifact.cells.reverse();
        assert!(artifact.validate().is_err());
    }

    #[test]
    fn transient_candidates_are_excluded_from_class_evidence() {
        let mut accumulator = CellAccumulator {
            naip_samples: 100,
            ground_samples: 10,
            ground_min_mm: 20_000,
            ..CellAccumulator::default()
        };
        accumulator.unclassified_height_bins[42] = 7;
        accumulator.unclassified_height_bins[60] = 20;
        let cell = classify_cell(CellIndex::new(0, 0).expect("valid cell"), &accumulator)
            .expect("cell classifies");
        assert_eq!(cell.class, EvidenceClass::Terrain);
        assert_eq!(cell.transient_masked_points, 7);
        assert_eq!(cell.lidar_sample_count, 10);
    }
}

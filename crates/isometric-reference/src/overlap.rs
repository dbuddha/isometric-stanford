//! Bounded comparison of independently captured registered supertiles.

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{PngColorType, ReferenceError, encode_raw_png, encode_raw_png_crop};

/// Input contract for one independent-overlap comparison.
pub const OVERLAP_REQUEST_SCHEMA: &str = "isometric-reference-overlap-comparison/v1";
/// Machine-readable output schema for overlap evidence.
pub const OVERLAP_REPORT_SCHEMA: &str = "isometric-reference-overlap-report/v1";

const DEPTH_HEADER_BYTES: u64 = 16;
const DEPTH_MAGIC: &[u8; 8] = b"ISOD32V1";
const MAX_DIMENSION: u32 = 4_096;
const MAX_RAW_BYTES: u64 = 512 * 1_024 * 1_024;
const HEX: &[u8; 16] = b"0123456789abcdef";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
enum RawLayerKind {
    Color,
    Whitebox,
    LinearDepth,
    ViewNormal,
    FixedShadow,
    Coverage,
}

impl RawLayerKind {
    const ALL: [Self; 6] = [
        Self::Color,
        Self::Whitebox,
        Self::LinearDepth,
        Self::ViewNormal,
        Self::FixedShadow,
        Self::Coverage,
    ];

    const fn filename(self) -> &'static str {
        match self {
            Self::Color => "color.rgba8",
            Self::Whitebox => "whitebox.rgba8",
            Self::LinearDepth => "depth.u32le",
            Self::ViewNormal => "normal.rgba8",
            Self::FixedShadow => "fixed-shadow.gray8",
            Self::Coverage => "coverage.gray8",
        }
    }

    const fn bytes_per_pixel(self) -> u64 {
        match self {
            Self::FixedShadow | Self::Coverage => 1,
            Self::Color | Self::Whitebox | Self::LinearDepth | Self::ViewNormal => 4,
        }
    }

    const fn header_bytes(self) -> u64 {
        match self {
            Self::LinearDepth => DEPTH_HEADER_BYTES,
            _ => 0,
        }
    }

    const fn gate(self) -> LayerGate {
        match self {
            Self::Coverage => LayerGate {
                maximum_absolute_difference: 0,
                maximum_above_tolerance_ppm: 0,
            },
            Self::LinearDepth => LayerGate {
                maximum_absolute_difference: 250,
                maximum_above_tolerance_ppm: 100,
            },
            Self::ViewNormal => LayerGate {
                maximum_absolute_difference: 2,
                maximum_above_tolerance_ppm: 100,
            },
            Self::Whitebox => LayerGate {
                maximum_absolute_difference: 3,
                maximum_above_tolerance_ppm: 250,
            },
            Self::FixedShadow => LayerGate {
                maximum_absolute_difference: 16,
                maximum_above_tolerance_ppm: 1_000,
            },
            Self::Color => LayerGate {
                maximum_absolute_difference: 24,
                maximum_above_tolerance_ppm: 5_000,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct LayerGate {
    maximum_absolute_difference: u64,
    maximum_above_tolerance_ppm: u64,
}

/// One raw candidate directory and its complete registered dimensions.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawCandidate {
    /// Directory containing the six fixed raw layer filenames.
    pub directory: PathBuf,
    /// Total width including guards.
    pub width_px: u32,
    /// Total height including guards.
    pub height_px: u32,
}

/// Geometry and locations for one overlap comparison.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlapComparisonRequest {
    /// Request schema.
    pub schema: String,
    /// Left independently captured candidate.
    pub left: RawCandidate,
    /// Right independently captured candidate.
    pub right: RawCandidate,
    /// Larger monolithic oracle candidate.
    pub monolithic: RawCandidate,
    /// Saved width of each independent core.
    pub independent_core_width_px: u32,
    /// Saved height shared by independent and monolithic cores.
    pub core_height_px: u32,
    /// Symmetric guard on every image.
    pub guard_px: u32,
    /// New output directory for derived comparison evidence.
    pub output_directory: PathBuf,
}

/// Difference statistics for one layer and registered relation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DifferenceMetrics {
    /// Pixels whose value differs exactly.
    pub exact_mismatch_pixels: u64,
    /// Largest scalar or maximum-channel absolute difference.
    pub maximum_absolute_difference: u64,
    /// Mean scalar or maximum-channel difference times one million.
    pub mean_absolute_difference_microunits: u64,
    /// Pixels above the layer's documented tolerance.
    pub pixels_above_tolerance: u64,
    /// Above-tolerance pixels per million.
    pub pixels_above_tolerance_ppm: u64,
    /// Pixels compared.
    pub pixels_compared: u64,
    /// Whether the documented gate passed.
    pub passed: bool,
}

/// All registered comparisons for one layer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LayerComparison {
    /// Accepted tolerance contract.
    gate: LayerGate,
    /// Left and right independent guard overlap.
    pub left_vs_right_overlap: DifferenceMetrics,
    /// Independent source agreement in the saved seam's narrow corridor.
    pub left_vs_right_seam_corridor: DifferenceMetrics,
    /// Left independent guard and corresponding monolithic oracle crop.
    pub left_vs_monolithic_overlap: DifferenceMetrics,
    /// Right independent guard and corresponding monolithic oracle crop.
    pub right_vs_monolithic_overlap: DifferenceMetrics,
    /// Assembled saved cores and the monolithic saved core.
    pub joined_vs_monolithic_core: DifferenceMetrics,
    /// Assembled saved seam corridor and the same monolithic crop.
    pub joined_boundary_vs_monolithic: DifferenceMetrics,
}

/// Best bounded translation between the two independently captured overlaps.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RegistrationSearch {
    /// Above-tolerance structural pixels per million at zero translation.
    pub baseline_above_tolerance_ppm: u64,
    /// Horizontal translation applied to the right capture, in source pixels.
    pub best_dx_px: i32,
    /// Vertical translation applied to the right capture, in source pixels.
    pub best_dy_px: i32,
    /// Above-tolerance structural pixels per million at the best translation.
    pub best_above_tolerance_ppm: u64,
    /// Number of pixel-layer observations compared for each translation.
    pub observations_compared: u64,
    /// Inclusive translation radius searched in both dimensions.
    pub radius_px: u32,
}

/// One hash-identified derived image served by the review dashboard.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceImage {
    /// Exact private output byte length.
    pub byte_length: u64,
    /// Pixel height.
    pub height_px: u32,
    /// Output filename relative to the comparison directory.
    pub path: String,
    /// Exact lowercase SHA-256.
    pub sha256: String,
    /// Pixel width.
    pub width_px: u32,
}

/// Scoped gates that keep source registration separate from project lighting.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OverlapGates {
    /// All source and lighting relations pass their documented thresholds.
    pub all_relations: bool,
    /// Whitebox and fixed shadow agree in the independent seam corridor and oracle.
    pub lighting_seam: bool,
    /// Source-only registration gates.
    pub source: SourceOverlapGates,
}

/// Source-only overlap gates.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceOverlapGates {
    /// Color, coverage, depth, and normals agree in the independent seam corridor.
    pub independent_seam: bool,
    /// The joined source seam agrees with the monolithic oracle.
    pub monolithic_seam: bool,
}

/// Complete bounded overlap evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OverlapComparisonReport {
    /// Depth discontinuities within 32 pixels of the saved boundary.
    pub boundary_structural_edge_pixels: u64,
    /// Stable failure classes inferred from measured layer behavior.
    pub failure_classifications: Vec<String>,
    /// Hash-identified dashboard evidence.
    pub images: BTreeMap<String, EvidenceImage>,
    /// Scoped qualification results.
    pub gates: OverlapGates,
    /// Per-layer registered comparisons.
    pub layers: BTreeMap<String, LayerComparison>,
    /// True only when every documented layer gate passes.
    pub passed: bool,
    /// Bounded diagnostic that separates camera translation from LOD drift.
    pub registration_search: RegistrationSearch,
    /// Report schema.
    pub schema: String,
}

#[derive(Clone, Copy)]
enum Relation {
    LeftRightOverlap,
    LeftRightSeamCorridor,
    LeftMonolithicOverlap,
    RightMonolithicOverlap,
    JoinedMonolithicCore,
    JoinedBoundaryMonolithic,
}

fn corridor_half_width(request: &OverlapComparisonRequest) -> u32 {
    request.guard_px.min(32)
}

struct RawImage {
    file: BufReader<File>,
    height: u32,
    kind: RawLayerKind,
    width: u32,
}

struct RelationReaders {
    kind: RawLayerKind,
    left: RawImage,
    monolithic: RawImage,
    right: RawImage,
}

impl RawImage {
    fn open(candidate: &RawCandidate, kind: RawLayerKind) -> Result<Self, ReferenceError> {
        let path = candidate.directory.join(kind.filename());
        let metadata = path.metadata()?;
        let expected = kind
            .header_bytes()
            .checked_add(
                u64::from(candidate.width_px)
                    .checked_mul(u64::from(candidate.height_px))
                    .and_then(|pixels| pixels.checked_mul(kind.bytes_per_pixel()))
                    .ok_or_else(|| ReferenceError::Invalid("raw overlap size overflowed".into()))?,
            )
            .ok_or_else(|| ReferenceError::Invalid("raw overlap size overflowed".into()))?;
        if !metadata.is_file() || metadata.len() != expected || expected > MAX_RAW_BYTES {
            return Err(ReferenceError::Invalid(format!(
                "raw overlap layer {} contradicts its dimensions",
                kind.filename()
            )));
        }
        let mut file = BufReader::with_capacity(64 * 1_024, File::open(path)?);
        if kind == RawLayerKind::LinearDepth {
            let mut header = [0_u8; 16];
            file.read_exact(&mut header)?;
            if &header[..8] != DEPTH_MAGIC
                || u32::from_le_bytes(header[8..12].try_into().map_err(|_| {
                    ReferenceError::Invalid("raw depth width header is invalid".into())
                })?) != candidate.width_px
                || u32::from_le_bytes(header[12..16].try_into().map_err(|_| {
                    ReferenceError::Invalid("raw depth height header is invalid".into())
                })?) != candidate.height_px
            {
                return Err(ReferenceError::Invalid(
                    "raw depth header contradicts the overlap grid".into(),
                ));
            }
        }
        Ok(Self {
            file,
            height: candidate.height_px,
            kind,
            width: candidate.width_px,
        })
    }

    fn read_row(&mut self, y: u32, x: u32, width: u32) -> Result<Vec<u8>, ReferenceError> {
        if y >= self.height || x.checked_add(width).is_none_or(|right| right > self.width) {
            return Err(ReferenceError::Invalid(
                "raw overlap crop exceeds its registered image".into(),
            ));
        }
        let bytes_per_pixel = self.kind.bytes_per_pixel();
        let offset = self
            .kind
            .header_bytes()
            .checked_add(
                u64::from(y)
                    .checked_mul(u64::from(self.width))
                    .and_then(|row| row.checked_add(u64::from(x)))
                    .and_then(|pixels| pixels.checked_mul(bytes_per_pixel))
                    .ok_or_else(|| {
                        ReferenceError::Invalid("raw overlap offset overflowed".into())
                    })?,
            )
            .ok_or_else(|| ReferenceError::Invalid("raw overlap offset overflowed".into()))?;
        let length = usize::try_from(
            u64::from(width)
                .checked_mul(bytes_per_pixel)
                .ok_or_else(|| ReferenceError::Invalid("raw overlap row overflowed".into()))?,
        )
        .map_err(|_| ReferenceError::Invalid("raw overlap row does not fit memory".into()))?;
        let mut row = vec![0_u8; length];
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(&mut row)?;
        Ok(row)
    }
}

impl RelationReaders {
    fn open(
        request: &OverlapComparisonRequest,
        kind: RawLayerKind,
    ) -> Result<Self, ReferenceError> {
        Ok(Self {
            kind,
            left: RawImage::open(&request.left, kind)?,
            monolithic: RawImage::open(&request.monolithic, kind)?,
            right: RawImage::open(&request.right, kind)?,
        })
    }

    fn read_relation_row(
        &mut self,
        request: &OverlapComparisonRequest,
        relation: Relation,
        row: u32,
    ) -> Result<(Vec<u8>, Vec<u8>), ReferenceError> {
        let overlap_width = request.guard_px * 2;
        match relation {
            Relation::LeftRightOverlap => Ok((
                self.left
                    .read_row(row, request.independent_core_width_px, overlap_width)?,
                self.right.read_row(row, 0, overlap_width)?,
            )),
            Relation::LeftMonolithicOverlap => Ok((
                self.left
                    .read_row(row, request.independent_core_width_px, overlap_width)?,
                self.monolithic
                    .read_row(row, request.independent_core_width_px, overlap_width)?,
            )),
            Relation::RightMonolithicOverlap => Ok((
                self.right.read_row(row, 0, overlap_width)?,
                self.monolithic
                    .read_row(row, request.independent_core_width_px, overlap_width)?,
            )),
            Relation::LeftRightSeamCorridor => {
                let half = corridor_half_width(request);
                let source_y = row + request.guard_px;
                Ok((
                    self.left.read_row(
                        source_y,
                        request.independent_core_width_px + request.guard_px - half,
                        half * 2,
                    )?,
                    self.right
                        .read_row(source_y, request.guard_px - half, half * 2)?,
                ))
            }
            Relation::JoinedMonolithicCore | Relation::JoinedBoundaryMonolithic => {
                let source_y = row + request.guard_px;
                if matches!(relation, Relation::JoinedBoundaryMonolithic) {
                    let half = corridor_half_width(request);
                    let mut joined = self.left.read_row(
                        source_y,
                        request.guard_px + request.independent_core_width_px - half,
                        half,
                    )?;
                    joined.extend_from_slice(&self.right.read_row(
                        source_y,
                        request.guard_px,
                        half,
                    )?);
                    return Ok((
                        joined,
                        self.monolithic.read_row(
                            source_y,
                            request.guard_px + request.independent_core_width_px - half,
                            half * 2,
                        )?,
                    ));
                }
                let mut joined = self.left.read_row(
                    source_y,
                    request.guard_px,
                    request.independent_core_width_px,
                )?;
                joined.extend_from_slice(&self.right.read_row(
                    source_y,
                    request.guard_px,
                    request.independent_core_width_px,
                )?);
                Ok((
                    joined,
                    self.monolithic.read_row(
                        source_y,
                        request.guard_px,
                        request.independent_core_width_px * 2,
                    )?,
                ))
            }
        }
    }
}

fn validate_request(request: &OverlapComparisonRequest) -> Result<(), ReferenceError> {
    let independent_total_width = request
        .independent_core_width_px
        .checked_add(2 * request.guard_px)
        .ok_or_else(|| ReferenceError::Invalid("independent overlap width overflowed".into()))?;
    let total_height = request
        .core_height_px
        .checked_add(2 * request.guard_px)
        .ok_or_else(|| ReferenceError::Invalid("overlap height overflowed".into()))?;
    let monolithic_width = request
        .independent_core_width_px
        .checked_mul(2)
        .and_then(|value| value.checked_add(2 * request.guard_px))
        .ok_or_else(|| ReferenceError::Invalid("monolithic overlap width overflowed".into()))?;
    let dimensions = [
        request.independent_core_width_px,
        request.core_height_px,
        request.guard_px,
        independent_total_width,
        total_height,
        monolithic_width,
    ];
    if request.schema != OVERLAP_REQUEST_SCHEMA
        || dimensions
            .iter()
            .any(|value| *value == 0 || *value > MAX_DIMENSION)
        || request.left.width_px != independent_total_width
        || request.right.width_px != independent_total_width
        || request.left.height_px != total_height
        || request.right.height_px != total_height
        || request.monolithic.width_px != monolithic_width
        || request.monolithic.height_px != total_height
        || request.output_directory.exists()
    {
        return Err(ReferenceError::Invalid(
            "registered overlap request violates its bounded layout".into(),
        ));
    }
    Ok(())
}

fn pixel_difference(kind: RawLayerKind, left: &[u8], right: &[u8]) -> u64 {
    if kind == RawLayerKind::LinearDepth {
        let left = u32::from_le_bytes(left.try_into().unwrap_or([0; 4]));
        let right = u32::from_le_bytes(right.try_into().unwrap_or([0; 4]));
        u64::from(left.abs_diff(right))
    } else {
        left.iter()
            .zip(right)
            .map(|(left, right)| u64::from(left.abs_diff(*right)))
            .max()
            .unwrap_or(0)
    }
}

fn relation_dimensions(request: &OverlapComparisonRequest, relation: Relation) -> (u32, u32) {
    let overlap_width = request.guard_px * 2;
    match relation {
        Relation::LeftRightOverlap
        | Relation::LeftMonolithicOverlap
        | Relation::RightMonolithicOverlap => (overlap_width, request.left.height_px),
        Relation::LeftRightSeamCorridor | Relation::JoinedBoundaryMonolithic => {
            (corridor_half_width(request) * 2, request.core_height_px)
        }
        Relation::JoinedMonolithicCore => (
            request.independent_core_width_px * 2,
            request.core_height_px,
        ),
    }
}

fn compare_relation(
    request: &OverlapComparisonRequest,
    kind: RawLayerKind,
    relation: Relation,
) -> Result<DifferenceMetrics, ReferenceError> {
    let (width, height) = relation_dimensions(request, relation);
    let mut readers = RelationReaders::open(request, kind)?;
    let gate = kind.gate();
    let bytes_per_pixel = usize::try_from(kind.bytes_per_pixel())
        .map_err(|_| ReferenceError::Invalid("pixel width does not fit memory".into()))?;
    let mut exact_mismatch_pixels = 0_u64;
    let mut maximum_absolute_difference = 0_u64;
    let mut pixels_above_tolerance = 0_u64;
    let mut total_difference = 0_u128;
    for row in 0..height {
        let (observed_row, expected_row) = readers.read_relation_row(request, relation, row)?;
        for (left, right) in observed_row
            .chunks_exact(bytes_per_pixel)
            .zip(expected_row.chunks_exact(bytes_per_pixel))
        {
            let difference = pixel_difference(kind, left, right);
            exact_mismatch_pixels += u64::from(difference != 0);
            pixels_above_tolerance += u64::from(difference > gate.maximum_absolute_difference);
            maximum_absolute_difference = maximum_absolute_difference.max(difference);
            total_difference += u128::from(difference);
        }
    }
    let pixels_compared = u64::from(width) * u64::from(height);
    let pixels_above_tolerance_ppm = pixels_above_tolerance
        .saturating_mul(1_000_000)
        .checked_div(pixels_compared)
        .unwrap_or(u64::MAX);
    let mean_absolute_difference_microunits = u64::try_from(
        total_difference
            .saturating_mul(1_000_000)
            .checked_div(u128::from(pixels_compared))
            .unwrap_or(u128::from(u64::MAX)),
    )
    .unwrap_or(u64::MAX);
    Ok(DifferenceMetrics {
        exact_mismatch_pixels,
        maximum_absolute_difference,
        mean_absolute_difference_microunits,
        pixels_above_tolerance,
        pixels_above_tolerance_ppm,
        pixels_compared,
        passed: maximum_absolute_difference <= gate.maximum_absolute_difference
            || pixels_above_tolerance_ppm <= gate.maximum_above_tolerance_ppm,
    })
}

fn registration_score(
    request: &OverlapComparisonRequest,
    dx: i32,
    dy: i32,
    radius: u32,
) -> Result<(u64, u64), ReferenceError> {
    let overlap_width = request.guard_px * 2;
    let width = overlap_width
        .checked_sub(radius * 2)
        .ok_or_else(|| ReferenceError::Invalid("registration search width underflowed".into()))?;
    let height = request
        .left
        .height_px
        .checked_sub(radius * 2)
        .ok_or_else(|| ReferenceError::Invalid("registration search height underflowed".into()))?;
    let right_x = i64::from(radius) + i64::from(dx);
    if right_x < 0 || i64::from(dy) + i64::from(radius) < 0 {
        return Err(ReferenceError::Invalid(
            "registration search translation exceeds its bounded interior".into(),
        ));
    }
    let mut above = 0_u64;
    let mut observations = 0_u64;
    for kind in [
        RawLayerKind::Coverage,
        RawLayerKind::LinearDepth,
        RawLayerKind::ViewNormal,
        RawLayerKind::Whitebox,
    ] {
        let mut left = RawImage::open(&request.left, kind)?;
        let mut right = RawImage::open(&request.right, kind)?;
        let bytes_per_pixel = usize::try_from(kind.bytes_per_pixel())
            .map_err(|_| ReferenceError::Invalid("pixel width does not fit memory".into()))?;
        for row in 0..height {
            let left_row = left.read_row(
                row + radius,
                request.independent_core_width_px + radius,
                width,
            )?;
            let right_y = i64::from(row) + i64::from(radius) + i64::from(dy);
            let right_row = right.read_row(
                u32::try_from(right_y).map_err(|_| {
                    ReferenceError::Invalid("registration search row is negative".into())
                })?,
                u32::try_from(right_x).map_err(|_| {
                    ReferenceError::Invalid("registration search column is negative".into())
                })?,
                width,
            )?;
            for (observed, expected) in left_row
                .chunks_exact(bytes_per_pixel)
                .zip(right_row.chunks_exact(bytes_per_pixel))
            {
                above += u64::from(
                    pixel_difference(kind, observed, expected)
                        > kind.gate().maximum_absolute_difference,
                );
                observations += 1;
            }
        }
    }
    Ok((above, observations))
}

fn search_registration(
    request: &OverlapComparisonRequest,
) -> Result<RegistrationSearch, ReferenceError> {
    const RADIUS: u32 = 2;
    const RADIUS_I32: i32 = 2;
    let (baseline_above, observations) = registration_score(request, 0, 0, RADIUS)?;
    let mut best = (baseline_above, 0_i32, 0_i32);
    for dy in -RADIUS_I32..=RADIUS_I32 {
        for dx in -RADIUS_I32..=RADIUS_I32 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let (above, compared) = registration_score(request, dx, dy, RADIUS)?;
            if compared != observations {
                return Err(ReferenceError::Invalid(
                    "registration search compared inconsistent observations".into(),
                ));
            }
            let candidate_distance = dx.unsigned_abs() + dy.unsigned_abs();
            let best_distance = best.1.unsigned_abs() + best.2.unsigned_abs();
            if above < best.0
                || (above == best.0
                    && (
                        candidate_distance,
                        dy.unsigned_abs(),
                        dx.unsigned_abs(),
                        dy,
                        dx,
                    ) < (
                        best_distance,
                        best.2.unsigned_abs(),
                        best.1.unsigned_abs(),
                        best.2,
                        best.1,
                    ))
            {
                best = (above, dx, dy);
            }
        }
    }
    let ppm = |above: u64| {
        above
            .saturating_mul(1_000_000)
            .checked_div(observations)
            .unwrap_or(u64::MAX)
    };
    Ok(RegistrationSearch {
        baseline_above_tolerance_ppm: ppm(baseline_above),
        best_dx_px: best.1,
        best_dy_px: best.2,
        best_above_tolerance_ppm: ppm(best.0),
        observations_compared: observations,
        radius_px: RADIUS,
    })
}

fn write_composed_core(
    request: &OverlapComparisonRequest,
    path: &Path,
) -> Result<(), ReferenceError> {
    let mut left = RawImage::open(&request.left, RawLayerKind::Color)?;
    let mut right = RawImage::open(&request.right, RawLayerKind::Color)?;
    let output = OpenOptions::new().create_new(true).write(true).open(path)?;
    let mut writer = BufWriter::with_capacity(64 * 1_024, output);
    for row in 0..request.core_height_px {
        let source_y = row + request.guard_px;
        writer.write_all(&left.read_row(
            source_y,
            request.guard_px,
            request.independent_core_width_px,
        )?)?;
        writer.write_all(&right.read_row(
            source_y,
            request.guard_px,
            request.independent_core_width_px,
        )?)?;
    }
    writer.flush()?;
    writer.get_ref().sync_all()?;
    Ok(())
}

fn write_heatmap(
    request: &OverlapComparisonRequest,
    relation: Relation,
    path: &Path,
) -> Result<(u32, u32), ReferenceError> {
    let mut readers = RawLayerKind::ALL
        .iter()
        .map(|kind| RelationReaders::open(request, *kind))
        .collect::<Result<Vec<_>, _>>()?;
    let (width, height) = relation_dimensions(request, relation);
    let output = OpenOptions::new().create_new(true).write(true).open(path)?;
    let mut writer = BufWriter::with_capacity(64 * 1_024, output);
    for row in 0..height {
        let rows = readers
            .iter_mut()
            .map(|reader| {
                reader
                    .read_relation_row(request, relation, row)
                    .map(|values| (reader.kind, values))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for column in 0..usize::try_from(width)
            .map_err(|_| ReferenceError::Invalid("heatmap width does not fit memory".into()))?
        {
            let mut structural = 0_u8;
            let mut shadow = 0_u8;
            let mut color = 0_u8;
            for (kind, (observed, expected)) in &rows {
                let bytes = usize::try_from(kind.bytes_per_pixel()).map_err(|_| {
                    ReferenceError::Invalid("heatmap pixel width does not fit memory".into())
                })?;
                let start = column * bytes;
                let difference = pixel_difference(
                    *kind,
                    &observed[start..start + bytes],
                    &expected[start..start + bytes],
                );
                let intensity = match kind {
                    RawLayerKind::LinearDepth => difference.saturating_mul(255) / 250,
                    _ => difference.saturating_mul(16),
                }
                .min(255) as u8;
                match kind {
                    RawLayerKind::Color => color = color.max(intensity),
                    RawLayerKind::FixedShadow | RawLayerKind::Whitebox => {
                        shadow = shadow.max(intensity);
                    }
                    RawLayerKind::LinearDepth
                    | RawLayerKind::ViewNormal
                    | RawLayerKind::Coverage => structural = structural.max(intensity),
                }
            }
            writer.write_all(&[structural, shadow, color, 255])?;
        }
    }
    writer.flush()?;
    writer.get_ref().sync_all()?;
    Ok((width, height))
}

fn sha256_file(path: &Path) -> Result<String, ReferenceError> {
    let mut digest = Sha256::new();
    let mut reader = BufReader::with_capacity(64 * 1_024, File::open(path)?);
    let mut buffer = vec![0_u8; 64 * 1_024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    let bytes = digest.finalize();
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(output)
}

fn evidence_image(
    output: &Path,
    filename: &str,
    width: u32,
    height: u32,
) -> Result<EvidenceImage, ReferenceError> {
    let path = output.join(filename);
    Ok(EvidenceImage {
        byte_length: path.metadata()?.len(),
        height_px: height,
        path: filename.into(),
        sha256: sha256_file(&path)?,
        width_px: width,
    })
}

fn boundary_edges(request: &OverlapComparisonRequest) -> Result<u64, ReferenceError> {
    let mut depth = RawImage::open(&request.monolithic, RawLayerKind::LinearDepth)?;
    let core_width = request.independent_core_width_px * 2;
    let seam = request.independent_core_width_px;
    let start = seam.saturating_sub(32);
    let end = (seam + 32).min(core_width.saturating_sub(1));
    let mut count = 0_u64;
    for row in 0..request.core_height_px {
        let bytes = depth.read_row(
            row + request.guard_px,
            request.guard_px + start,
            end - start + 1,
        )?;
        for values in bytes.chunks_exact(4).collect::<Vec<_>>().windows(2) {
            let left = u32::from_le_bytes(values[0].try_into().unwrap_or([0; 4]));
            let right = u32::from_le_bytes(values[1].try_into().unwrap_or([0; 4]));
            count += u64::from(left != 0 && right != 0 && left.abs_diff(right) >= 500);
        }
    }
    Ok(count)
}

fn overlap_gates(layers: &BTreeMap<String, LayerComparison>) -> OverlapGates {
    let source = ["color", "coverage", "linear-depth", "view-normal"];
    let lighting = ["whitebox", "fixed-shadow"];
    let independent_source_seam = source.iter().all(|name| {
        layers
            .get(*name)
            .is_some_and(|layer| layer.left_vs_right_seam_corridor.passed)
    });
    let monolithic_source_seam = source.iter().all(|name| {
        layers
            .get(*name)
            .is_some_and(|layer| layer.joined_boundary_vs_monolithic.passed)
    });
    let lighting_seam = lighting.iter().all(|name| {
        layers.get(*name).is_some_and(|layer| {
            layer.left_vs_right_seam_corridor.passed && layer.joined_boundary_vs_monolithic.passed
        })
    });
    let all_relations = layers.values().all(|layer| {
        layer.left_vs_right_overlap.passed
            && layer.left_vs_right_seam_corridor.passed
            && layer.left_vs_monolithic_overlap.passed
            && layer.right_vs_monolithic_overlap.passed
            && layer.joined_vs_monolithic_core.passed
            && layer.joined_boundary_vs_monolithic.passed
    });
    OverlapGates {
        all_relations,
        lighting_seam,
        source: SourceOverlapGates {
            independent_seam: independent_source_seam,
            monolithic_seam: monolithic_source_seam,
        },
    }
}

fn classifications(
    layers: &BTreeMap<String, LayerComparison>,
    registration: &RegistrationSearch,
    gates: &OverlapGates,
) -> Vec<String> {
    let failed = |name: &str| {
        layers.get(name).is_some_and(|layer| {
            !layer.left_vs_right_overlap.passed
                || !layer.left_vs_monolithic_overlap.passed
                || !layer.right_vs_monolithic_overlap.passed
                || !layer.joined_vs_monolithic_core.passed
        })
    };
    let seam_failed = |name: &str| {
        layers
            .get(name)
            .is_some_and(|layer| !layer.left_vs_right_seam_corridor.passed)
    };
    let mut values = Vec::new();
    if seam_failed("coverage") {
        values.push("missing-source".into());
    } else if failed("coverage") {
        values.push("monolithic-oracle-coverage".into());
    }
    if seam_failed("linear-depth") || seam_failed("view-normal") {
        let translated = registration.best_dx_px != 0 || registration.best_dy_px != 0;
        let material_improvement = registration.best_above_tolerance_ppm.saturating_mul(2)
            < registration.baseline_above_tolerance_ppm;
        values.push(
            if translated && material_improvement {
                "subpixel-camera"
            } else {
                "level-of-detail"
            }
            .into(),
        );
    } else if !gates.source.monolithic_seam {
        values.push("monolithic-oracle-level-of-detail".into());
    }
    if failed("fixed-shadow") || failed("whitebox") {
        values.push("shadow-phase".into());
    }
    if seam_failed("color") {
        values.push("live-session-texture".into());
    } else if failed("color") {
        values.push("monolithic-oracle-texture".into());
    }
    values
}

fn compare_layers(
    request: &OverlapComparisonRequest,
) -> Result<BTreeMap<String, LayerComparison>, ReferenceError> {
    let mut layers = BTreeMap::new();
    for kind in RawLayerKind::ALL {
        let comparison = LayerComparison {
            gate: kind.gate(),
            left_vs_right_overlap: compare_relation(request, kind, Relation::LeftRightOverlap)?,
            left_vs_right_seam_corridor: compare_relation(
                request,
                kind,
                Relation::LeftRightSeamCorridor,
            )?,
            left_vs_monolithic_overlap: compare_relation(
                request,
                kind,
                Relation::LeftMonolithicOverlap,
            )?,
            right_vs_monolithic_overlap: compare_relation(
                request,
                kind,
                Relation::RightMonolithicOverlap,
            )?,
            joined_vs_monolithic_core: compare_relation(
                request,
                kind,
                Relation::JoinedMonolithicCore,
            )?,
            joined_boundary_vs_monolithic: compare_relation(
                request,
                kind,
                Relation::JoinedBoundaryMonolithic,
            )?,
        };
        let name = serde_json::to_value(kind)?
            .as_str()
            .ok_or_else(|| ReferenceError::Invalid("layer identity is invalid".into()))?
            .to_owned();
        layers.insert(name, comparison);
    }
    Ok(layers)
}

fn write_core_previews(request: &OverlapComparisonRequest) -> Result<(), ReferenceError> {
    let output = &request.output_directory;
    let joined_raw = output.join("joined-core.rgba8");
    write_composed_core(request, &joined_raw)?;
    encode_raw_png(
        &joined_raw,
        &output.join("joined-core.png"),
        request.independent_core_width_px * 2,
        request.core_height_px,
        PngColorType::Rgba,
    )?;
    fs::remove_file(&joined_raw)?;
    encode_raw_png_crop(
        &request
            .monolithic
            .directory
            .join(RawLayerKind::Color.filename()),
        &output.join("monolithic-core.png"),
        request.monolithic.width_px,
        request.monolithic.height_px,
        request.guard_px,
        request.guard_px,
        request.independent_core_width_px * 2,
        request.core_height_px,
        PngColorType::Rgba,
    )?;
    Ok(())
}

fn write_overlap_previews(request: &OverlapComparisonRequest) -> Result<(), ReferenceError> {
    let output = &request.output_directory;
    let overlap_width = request.guard_px * 2;
    for (filename, candidate, x) in [
        (
            "overlap-left.png",
            &request.left,
            request.independent_core_width_px,
        ),
        ("overlap-right.png", &request.right, 0),
        (
            "overlap-monolithic.png",
            &request.monolithic,
            request.independent_core_width_px,
        ),
    ] {
        encode_raw_png_crop(
            &candidate.directory.join(RawLayerKind::Color.filename()),
            &output.join(filename),
            candidate.width_px,
            candidate.height_px,
            x,
            0,
            overlap_width,
            candidate.height_px,
            PngColorType::Rgba,
        )?;
    }
    Ok(())
}

fn write_one_heatmap(
    request: &OverlapComparisonRequest,
    stem: &str,
    relation: Relation,
) -> Result<(u32, u32), ReferenceError> {
    let raw = request.output_directory.join(format!("{stem}.rgba8"));
    let dimensions = write_heatmap(request, relation, &raw)?;
    encode_raw_png(
        &raw,
        &request.output_directory.join(format!("{stem}.png")),
        dimensions.0,
        dimensions.1,
        PngColorType::Rgba,
    )?;
    fs::remove_file(raw)?;
    Ok(dimensions)
}

fn collect_evidence_images(
    request: &OverlapComparisonRequest,
    overlap_heatmap: (u32, u32),
    core_heatmap: (u32, u32),
) -> Result<BTreeMap<String, EvidenceImage>, ReferenceError> {
    let output = &request.output_directory;
    let overlap_width = request.guard_px * 2;
    let mut images = BTreeMap::new();
    for (name, filename, width, height) in [
        (
            "joined-core",
            "joined-core.png",
            request.independent_core_width_px * 2,
            request.core_height_px,
        ),
        (
            "monolithic-core",
            "monolithic-core.png",
            request.independent_core_width_px * 2,
            request.core_height_px,
        ),
        (
            "overlap-left",
            "overlap-left.png",
            overlap_width,
            request.left.height_px,
        ),
        (
            "overlap-right",
            "overlap-right.png",
            overlap_width,
            request.right.height_px,
        ),
        (
            "overlap-monolithic",
            "overlap-monolithic.png",
            overlap_width,
            request.monolithic.height_px,
        ),
        (
            "overlap-heatmap",
            "overlap-heatmap.png",
            overlap_heatmap.0,
            overlap_heatmap.1,
        ),
        (
            "core-oracle-heatmap",
            "core-oracle-heatmap.png",
            core_heatmap.0,
            core_heatmap.1,
        ),
    ] {
        images.insert(
            name.into(),
            evidence_image(output, filename, width, height)?,
        );
    }
    Ok(images)
}

/// Compare two independent captures and a monolithic oracle with bounded rows.
///
/// # Errors
///
/// Returns an error for an invalid layout, incomplete raw layer archive, an
/// existing output, arithmetic overflow, or local I/O failure.
pub fn compare_registered_overlap(
    request: &OverlapComparisonRequest,
) -> Result<OverlapComparisonReport, ReferenceError> {
    validate_request(request)?;
    fs::create_dir_all(&request.output_directory)?;
    let layers = compare_layers(request)?;
    let registration_search = search_registration(request)?;
    let gates = overlap_gates(&layers);
    write_core_previews(request)?;
    write_overlap_previews(request)?;
    let overlap_heatmap =
        write_one_heatmap(request, "overlap-heatmap", Relation::LeftRightOverlap)?;
    let core_heatmap = write_one_heatmap(
        request,
        "core-oracle-heatmap",
        Relation::JoinedMonolithicCore,
    )?;
    let images = collect_evidence_images(request, overlap_heatmap, core_heatmap)?;
    let failure_classifications = classifications(&layers, &registration_search, &gates);
    let passed = gates.all_relations;
    let report = OverlapComparisonReport {
        boundary_structural_edge_pixels: boundary_edges(request)?,
        failure_classifications,
        gates,
        images,
        layers,
        passed,
        registration_search,
        schema: OVERLAP_REPORT_SCHEMA.into(),
    };
    let report_path = request.output_directory.join("comparison.json");
    let mut report_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(report_path)?;
    serde_json::to_writer_pretty(&mut report_file, &report)?;
    report_file.write_all(b"\n")?;
    report_file.sync_all()?;
    Ok(report)
}

/// Read and execute a JSON overlap comparison request.
///
/// # Errors
///
/// Returns an error when the request cannot be decoded or the comparison fails.
pub fn compare_registered_overlap_file(
    request_path: &Path,
) -> Result<OverlapComparisonReport, ReferenceError> {
    let mut request: OverlapComparisonRequest =
        serde_json::from_reader(BufReader::new(File::open(request_path)?))?;
    let base = request_path.parent().unwrap_or_else(|| Path::new("."));
    for directory in [
        &mut request.left.directory,
        &mut request.right.directory,
        &mut request.monolithic.directory,
        &mut request.output_directory,
    ] {
        if directory.is_relative() {
            *directory = base.join(&*directory);
        }
    }
    compare_registered_overlap(&request)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("isometric-overlap-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create overlap fixture root");
        root
    }

    fn write_candidate(path: &Path, width: u32, height: u32, x_origin: u32) {
        fs::create_dir_all(path).expect("create raw candidate");
        for kind in RawLayerKind::ALL {
            let mut bytes = Vec::new();
            if kind == RawLayerKind::LinearDepth {
                bytes.extend_from_slice(DEPTH_MAGIC);
                bytes.extend_from_slice(&width.to_le_bytes());
                bytes.extend_from_slice(&height.to_le_bytes());
            }
            for y in 0..height {
                for x in 0..width {
                    let world_x = x_origin + x;
                    match kind {
                        RawLayerKind::LinearDepth => {
                            bytes.extend_from_slice(
                                &(1_000_u32 + world_x * 1_000 + y).to_le_bytes(),
                            );
                        }
                        RawLayerKind::FixedShadow | RawLayerKind::Coverage => {
                            bytes.push(if kind == RawLayerKind::Coverage {
                                255
                            } else {
                                ((world_x + y) % 251) as u8
                            });
                        }
                        _ => bytes.extend_from_slice(&[
                            ((world_x * 17) % 251) as u8,
                            (y % 251) as u8,
                            ((world_x + y) % 251) as u8,
                            255,
                        ]),
                    }
                }
            }
            fs::write(path.join(kind.filename()), bytes).expect("write raw layer");
        }
    }

    fn fixture(root: &Path) -> OverlapComparisonRequest {
        let core = 8;
        let guard = 3;
        let height = 8;
        write_candidate(&root.join("left"), 14, 14, 0);
        write_candidate(&root.join("right"), 14, 14, 8);
        write_candidate(&root.join("monolithic"), 22, 14, 0);
        OverlapComparisonRequest {
            schema: OVERLAP_REQUEST_SCHEMA.into(),
            left: RawCandidate {
                directory: root.join("left"),
                width_px: 14,
                height_px: 14,
            },
            right: RawCandidate {
                directory: root.join("right"),
                width_px: 14,
                height_px: 14,
            },
            monolithic: RawCandidate {
                directory: root.join("monolithic"),
                width_px: 22,
                height_px: 14,
            },
            independent_core_width_px: core,
            core_height_px: height,
            guard_px: guard,
            output_directory: root.join("evidence"),
        }
    }

    #[test]
    fn exact_registered_candidates_pass_and_emit_hashed_images() {
        let root = root("exact");
        let request = fixture(&root);
        let report = compare_registered_overlap(&request).expect("compare exact overlap");
        assert!(report.passed);
        assert!(report.failure_classifications.is_empty());
        assert_eq!(report.registration_search.best_dx_px, 0);
        assert_eq!(report.registration_search.best_dy_px, 0);
        assert_eq!(report.images.len(), 7);
        assert!(
            report
                .layers
                .values()
                .all(|layer| layer.joined_vs_monolithic_core.exact_mismatch_pixels == 0)
        );
        assert_eq!(
            report,
            serde_json::from_slice::<OverlapComparisonReport>(
                &fs::read(root.join("evidence/comparison.json")).expect("read report")
            )
            .expect("decode report")
        );
        fs::remove_dir_all(root).expect("remove exact fixture");
    }

    #[test]
    fn missing_source_is_classified_without_hiding_evidence() {
        let root = root("missing");
        let request = fixture(&root);
        let right_coverage = root.join("right/coverage.gray8");
        let mut bytes = fs::read(&right_coverage).expect("read right coverage");
        bytes[3 * 14] = 0;
        fs::write(&right_coverage, bytes).expect("change coverage");
        let report = compare_registered_overlap(&request).expect("compare failed overlap");
        assert!(!report.passed);
        assert!(
            report
                .failure_classifications
                .contains(&"missing-source".to_string())
        );
        fs::remove_dir_all(root).expect("remove missing fixture");
    }

    #[test]
    fn translated_capture_is_distinguished_from_level_of_detail_drift() {
        let root = root("translated");
        let request = fixture(&root);
        write_candidate(&root.join("right"), 14, 14, 7);
        let report = compare_registered_overlap(&request).expect("compare translated overlap");
        assert!(!report.passed);
        assert_eq!(report.registration_search.best_dx_px, 1);
        assert_eq!(report.registration_search.best_dy_px, 0);
        assert!(
            report
                .failure_classifications
                .contains(&"subpixel-camera".to_string())
        );
        fs::remove_dir_all(root).expect("remove translated fixture");
    }

    #[test]
    fn request_file_resolves_relative_raw_and_output_directories() {
        let root = root("relative");
        let request = fixture(&root);
        let request_path = root.join("request.json");
        let relative = serde_json::json!({
            "schema": request.schema,
            "left": { "directory": "left", "width_px": 14, "height_px": 14 },
            "right": { "directory": "right", "width_px": 14, "height_px": 14 },
            "monolithic": { "directory": "monolithic", "width_px": 22, "height_px": 14 },
            "independent_core_width_px": 8,
            "core_height_px": 8,
            "guard_px": 3,
            "output_directory": "relative-evidence"
        });
        fs::write(
            &request_path,
            serde_json::to_vec(&relative).expect("encode relative request"),
        )
        .expect("write relative request");
        let report = compare_registered_overlap_file(&request_path)
            .expect("compare relative overlap request");
        assert!(report.passed);
        assert!(root.join("relative-evidence/comparison.json").is_file());
        fs::remove_dir_all(root).expect("remove relative fixture");
    }

    #[test]
    fn unsafe_or_existing_output_fails_closed() {
        let root = root("invalid");
        let mut request = fixture(&root);
        request.guard_px = 0;
        assert!(compare_registered_overlap(&request).is_err());
        let request = fixture(&root);
        fs::create_dir_all(&request.output_directory).expect("precreate evidence");
        assert!(compare_registered_overlap(&request).is_err());
        fs::remove_dir_all(root).expect("remove invalid fixture");
    }
}

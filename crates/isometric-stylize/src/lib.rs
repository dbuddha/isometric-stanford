//! Deterministic reference-abstraction experiments over registered Google captures.
//!
//! The crate intentionally separates source qualification from final art approval.
//! It produces an RGB-only baseline, a geometry-guided comparison, and a narrowly
//! repaired candidate whose only replacement is high-confidence tree canopy.

use std::{
    collections::BTreeSet,
    error::Error,
    fmt::{Display, Formatter},
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use isometric_reference::{LayerKind, PngColorType, ReferenceManifest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

mod image;
mod palette;

use image::{
    RgbImage, apply_structural_outlines, canopy_mask, edge_aware_smooth, luminance, mask_image,
    output_edges, quantize, reduce_depth_2x, reduce_rgb_2x, relight, replace_canopy,
    structural_edges,
};
use palette::{BASE_PALETTE, CANOPY_PALETTE, GEOMETRY_PALETTE, Rgb, STRUCTURAL_OUTLINE};

/// Portable review-report schema.
pub const REVIEW_SCHEMA: &str = "isometric-reference-repair-review/v1";
/// Canonical report filename.
pub const REVIEW_FILENAME: &str = "repair-review.json";
/// Versioned deterministic algorithm identity.
pub const ALGORITHM_ID: &str = "reference-repair-rust/v1";

const DEPTH_MAGIC: &[u8; 8] = b"ISOD32V1";
const MAX_REPORT_BYTES: usize = 1_048_576;
const OUTPUT_IMAGE_NAMES: [&str; 6] = [
    "source-logical.png",
    "candidate-a-rgb.png",
    "candidate-b-geometry.png",
    "candidate-c-canopy-repair.png",
    "canopy-mask.png",
    "structural-edges.png",
];

/// One immutable output image record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReviewImageRecord {
    /// Stable image identity.
    pub id: String,
    /// Human-facing description.
    pub label: String,
    /// Allowlisted report-relative PNG path.
    pub path: String,
    /// Output width.
    pub width_px: u32,
    /// Output height.
    pub height_px: u32,
    /// Exact encoded byte length.
    pub byte_length: u64,
    /// SHA-256 over exact encoded bytes.
    pub sha256: String,
}

/// Objective measurements for one study candidate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CandidateMetrics {
    /// Stable candidate identity.
    pub candidate_id: String,
    /// Number of distinct output RGB colors.
    pub colors_used: u16,
    /// Structural guidance pixels that retain a visible output edge.
    pub structural_edge_recall_basis_points: u16,
    /// Output edge pixels not adjacent to a structural guidance edge.
    pub non_structural_edge_ppm: u32,
    /// Mean output luminance in integer micro-units.
    pub mean_luminance_microunits: u32,
    /// Pixel positions changed from the logical source.
    pub changed_from_source_ppm: u32,
    /// Edge density strictly inside accepted canopy regions.
    pub canopy_interior_edge_ppm: u32,
}

/// Machine-checkable interpretation of the bounded experiment.
#[expect(
    clippy::struct_excessive_bools,
    reason = "the portable report records independent binary qualification gates"
)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReviewGates {
    /// All canonical transformations are integer and deterministic after capture.
    pub deterministic_post_capture: bool,
    /// Every candidate stays within the 128-color contract.
    pub palette_bound: bool,
    /// Geometry guidance preserves at least the experimental edge floor.
    pub structural_edge_recall: bool,
    /// Canopy replacement reduces internal source fragmentation.
    pub canopy_fragmentation_improved: bool,
    /// Passenger cars are excluded from the transient repair policy.
    pub passenger_cars_preserved_by_policy: bool,
    /// Whether the experiment is sufficient to expand capture or publication.
    pub qualified_for_expansion: bool,
}

/// Complete deterministic review artifact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairReviewReport {
    /// Report schema.
    pub schema: String,
    /// Deterministic implementation identity.
    pub algorithm: String,
    /// Validated source bundle identity.
    pub source_bundle_id: String,
    /// Canonical source manifest digest.
    pub source_manifest_sha256: String,
    /// Source millimeters per pixel.
    pub source_millimeters_per_pixel: u32,
    /// Output millimeters per logical pixel.
    pub logical_millimeters_per_pixel: u32,
    /// Fixed camera azimuth.
    pub camera_azimuth_millidegrees: u32,
    /// Fixed camera elevation.
    pub camera_elevation_millidegrees: u32,
    /// Deterministic output images.
    pub images: Vec<ReviewImageRecord>,
    /// Objective measurements by candidate.
    pub candidates: Vec<CandidateMetrics>,
    /// High-confidence logical canopy pixels replaced by Candidate C.
    pub canopy_pixels: u64,
    /// Structural guidance pixels used for comparison.
    pub structural_edge_pixels: u64,
    /// Conservative peak bytes of simultaneously live canonical buffers.
    pub estimated_peak_working_bytes: u64,
    /// Explicit review blockers that software must not conceal.
    pub blocking_findings: Vec<String>,
    /// Qualification results.
    pub gates: ReviewGates,
}

/// Successful experiment evidence returned to the CLI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepairRunReport {
    /// Canonical review report digest.
    pub report_sha256: String,
    /// Number of deterministic output images.
    pub image_count: usize,
    /// Conservative working-memory estimate.
    pub estimated_peak_working_bytes: u64,
    /// Whether the result is qualified to expand.
    pub qualified_for_expansion: bool,
}

/// Fail-closed reference stylization error.
#[derive(Debug)]
pub enum StylizeError {
    /// An input, output, or algorithm invariant was violated.
    Invalid(String),
    /// Local I/O failed.
    Io(std::io::Error),
    /// PNG decoding failed.
    Png(png::DecodingError),
    /// JSON failed.
    Json(serde_json::Error),
    /// Registered reference validation failed.
    Reference(isometric_reference::ReferenceError),
}

impl Display for StylizeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::Io(error) => write!(formatter, "reference stylization I/O failed: {error}"),
            Self::Png(error) => write!(formatter, "reference stylization PNG failed: {error}"),
            Self::Json(error) => write!(formatter, "reference stylization JSON failed: {error}"),
            Self::Reference(error) => {
                write!(formatter, "reference stylization input failed: {error}")
            }
        }
    }
}

impl Error for StylizeError {}

impl From<std::io::Error> for StylizeError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<png::DecodingError> for StylizeError {
    fn from(value: png::DecodingError) -> Self {
        Self::Png(value)
    }
}

impl From<serde_json::Error> for StylizeError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<isometric_reference::ReferenceError> for StylizeError {
    fn from(value: isometric_reference::ReferenceError) -> Self {
        Self::Reference(value)
    }
}

struct StagingDirectory {
    keep: bool,
    path: PathBuf,
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// Run the bounded three-candidate reference-repair experiment.
///
/// # Errors
///
/// Returns an error if the source bundle is invalid, the capture does not use
/// the qualified two-samples-per-logical-pixel contract, the output exists, or
/// any output cannot be written and validated.
#[expect(
    clippy::too_many_lines,
    reason = "the linear experiment pipeline keeps candidate inputs and gates visibly co-located"
)]
pub fn run_reference_repair(
    bundle_root: &Path,
    output_root: &Path,
) -> Result<RepairRunReport, StylizeError> {
    if output_root.exists() {
        return Err(StylizeError::Invalid(
            "reference repair output already exists".into(),
        ));
    }
    let manifest = isometric_reference::read_manifest(
        &bundle_root.join(isometric_reference::MANIFEST_FILENAME),
    )?;
    let source_report = isometric_reference::validate_bundle(bundle_root, &manifest)?;
    validate_capture_contract(&manifest)?;

    let parent = output_root.parent().ok_or_else(|| {
        StylizeError::Invalid("reference repair output requires a parent directory".into())
    })?;
    fs::create_dir_all(parent)?;
    let name = output_root
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| StylizeError::Invalid("reference repair output name is invalid".into()))?;
    let staging_path = parent.join(format!(".{name}.staging-{}", std::process::id()));
    if staging_path.exists() {
        return Err(StylizeError::Invalid(
            "reference repair staging directory already exists".into(),
        ));
    }
    fs::create_dir(&staging_path)?;
    let mut staging = StagingDirectory {
        keep: false,
        path: staging_path.clone(),
    };

    let source_color = decode_core_png(bundle_root, &manifest, LayerKind::Color)?;
    let source_width = source_color.width;
    let source_height = source_color.height;
    let logical_source = reduce_rgb_2x(&source_color)?;
    drop(source_color);
    let source_normal = decode_core_png(bundle_root, &manifest, LayerKind::ViewNormal)?;
    let logical_normals = reduce_rgb_2x(&source_normal)?;
    drop(source_normal);
    let source_depth = read_core_depth(bundle_root, &manifest)?;
    let logical_depth = reduce_depth_2x(source_width, source_height, &source_depth)?;
    drop(source_depth);

    let rgb_smooth = edge_aware_smooth(&logical_source, None, None, 2)?;
    let candidate_a = quantize(&rgb_smooth, &BASE_PALETTE)?;
    let geometry_smooth = edge_aware_smooth(
        &logical_source,
        Some(&logical_normals),
        Some(&logical_depth),
        2,
    )?;
    let geometry_relit = relight(&geometry_smooth, &logical_normals)?;
    let canopy = canopy_mask(&geometry_relit, &logical_normals)?;
    let structural = structural_edges(&logical_normals, &logical_depth)?;
    let candidate_b = apply_structural_outlines(
        &quantize(&geometry_relit, &GEOMETRY_PALETTE)?,
        &structural,
        &canopy,
        STRUCTURAL_OUTLINE,
    )?;
    let canopy_repaired = replace_canopy(
        &candidate_a,
        &geometry_relit,
        &logical_normals,
        &canopy,
        &CANOPY_PALETTE,
    )?;
    let candidate_c =
        apply_structural_outlines(&canopy_repaired, &structural, &canopy, STRUCTURAL_OUTLINE)?;
    let canopy_visual = mask_image(&canopy, logical_source.width, logical_source.height)?;
    let structural_visual = mask_image(&structural, logical_source.width, logical_source.height)?;

    let images = [
        (
            "source-logical",
            "Google source at logical scale",
            &logical_source,
        ),
        (
            "candidate-a-rgb",
            "Candidate A: RGB-only abstraction",
            &candidate_a,
        ),
        (
            "candidate-b-geometry",
            "Candidate B: geometry-guided abstraction",
            &candidate_b,
        ),
        (
            "candidate-c-canopy-repair",
            "Candidate C: filtered architecture plus canopy repair",
            &candidate_c,
        ),
        (
            "canopy-mask",
            "High-confidence canopy repair mask",
            &canopy_visual,
        ),
        (
            "structural-edges",
            "Depth and normal structural edges",
            &structural_visual,
        ),
    ];
    let mut image_records = Vec::with_capacity(images.len());
    for (id, label, image) in images {
        image_records.push(write_png(&staging_path, id, label, image)?);
    }

    let candidate_metrics = vec![
        measure_candidate(
            "candidate-a-rgb",
            &logical_source,
            &candidate_a,
            &structural,
            &canopy,
        ),
        measure_candidate(
            "candidate-b-geometry",
            &logical_source,
            &candidate_b,
            &structural,
            &canopy,
        ),
        measure_candidate(
            "candidate-c-canopy-repair",
            &logical_source,
            &candidate_c,
            &structural,
            &canopy,
        ),
    ];
    let palette_bound = candidate_metrics
        .iter()
        .all(|metrics| metrics.colors_used <= 128);
    let structural_edge_recall = candidate_metrics[1].structural_edge_recall_basis_points >= 9_000;
    let canopy_fragmentation_improved = candidate_metrics[2].canopy_interior_edge_ppm
        < candidate_metrics[0].canopy_interior_edge_ppm;
    let pixel_count = logical_source.pixels.len();
    let estimated_peak_working_bytes = estimate_peak_bytes(pixel_count)?;
    let report = RepairReviewReport {
        schema: REVIEW_SCHEMA.into(),
        algorithm: ALGORITHM_ID.into(),
        source_bundle_id: manifest.bundle_id.clone(),
        source_manifest_sha256: source_report.manifest_sha256,
        source_millimeters_per_pixel: manifest.tile.millimeters_per_pixel,
        logical_millimeters_per_pixel: manifest.tile.millimeters_per_pixel * 2,
        camera_azimuth_millidegrees: manifest.camera.azimuth_millidegrees,
        camera_elevation_millidegrees: manifest.camera.elevation_millidegrees,
        images: image_records,
        candidates: candidate_metrics,
        canopy_pixels: u64::try_from(canopy.iter().filter(|value| **value).count())
            .map_err(|_| StylizeError::Invalid("canopy pixel count overflowed u64".into()))?,
        structural_edge_pixels: u64::try_from(structural.iter().filter(|value| **value).count())
            .map_err(|_| StylizeError::Invalid("structural pixel count overflowed u64".into()))?,
        estimated_peak_working_bytes,
        blocking_findings: vec![
            "construction-region-lacks-an-accepted-instance-mask".into(),
            "candidate-c-repairs-only-high-confidence-canopy".into(),
            "visual-style-approval-is-human-owned".into(),
        ],
        gates: ReviewGates {
            deterministic_post_capture: true,
            palette_bound,
            structural_edge_recall,
            canopy_fragmentation_improved,
            passenger_cars_preserved_by_policy: true,
            qualified_for_expansion: false,
        },
    };
    let report_json = canonical_report_json(&report)?;
    let report_path = staging_path.join(REVIEW_FILENAME);
    write_new(&report_path, report_json.as_bytes())?;
    validate_report(&staging_path, &report)?;
    let report_sha256 = sha256_bytes(report_json.as_bytes());
    fs::rename(&staging_path, output_root)?;
    staging.keep = true;
    Ok(RepairRunReport {
        report_sha256,
        image_count: report.images.len(),
        estimated_peak_working_bytes,
        qualified_for_expansion: report.gates.qualified_for_expansion,
    })
}

/// Read a review report without trusting its referenced output files.
///
/// # Errors
///
/// Returns an error if the file exceeds the bounded report size or JSON fails.
pub fn read_report(path: &Path) -> Result<RepairReviewReport, StylizeError> {
    let metadata = path.metadata()?;
    if metadata.len() > MAX_REPORT_BYTES as u64 {
        return Err(StylizeError::Invalid(
            "reference repair report exceeds one MiB".into(),
        ));
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

/// Validate a complete immutable review artifact.
///
/// # Errors
///
/// Returns an error if any identity, metric, path, digest, or image header is invalid.
pub fn validate_report(
    root: &Path,
    report: &RepairReviewReport,
) -> Result<RepairRunReport, StylizeError> {
    if report.schema != REVIEW_SCHEMA
        || report.algorithm != ALGORITHM_ID
        || report.source_bundle_id.is_empty()
        || !is_sha256(&report.source_manifest_sha256)
        || report.source_millimeters_per_pixel == 0
        || report.logical_millimeters_per_pixel != report.source_millimeters_per_pixel * 2
        || report.images.len() != OUTPUT_IMAGE_NAMES.len()
        || report.candidates.len() != 3
        || !report.gates.deterministic_post_capture
        || !report.gates.palette_bound
        || !report.gates.passenger_cars_preserved_by_policy
        || report.gates.qualified_for_expansion
        || report.blocking_findings.is_empty()
        || report.estimated_peak_working_bytes > 96 * 1_024 * 1_024
    {
        return Err(StylizeError::Invalid(
            "reference repair report contract is invalid".into(),
        ));
    }
    for (record, expected_path) in report.images.iter().zip(OUTPUT_IMAGE_NAMES) {
        if record.path != expected_path
            || record.width_px == 0
            || record.height_px == 0
            || record.width_px > 2_048
            || record.height_px > 2_048
            || !is_sha256(&record.sha256)
        {
            return Err(StylizeError::Invalid(
                "reference repair image record is invalid".into(),
            ));
        }
        let path = root.join(&record.path);
        let metadata = path.metadata()?;
        if metadata.len() != record.byte_length || sha256_file(&path)? != record.sha256 {
            return Err(StylizeError::Invalid(
                "reference repair image digest is invalid".into(),
            ));
        }
        validate_png_header(&path, record.width_px, record.height_px)?;
    }
    let ids = report
        .candidates
        .iter()
        .map(|candidate| candidate.candidate_id.as_str())
        .collect::<Vec<_>>();
    if ids
        != [
            "candidate-a-rgb",
            "candidate-b-geometry",
            "candidate-c-canopy-repair",
        ]
        || report
            .candidates
            .iter()
            .any(|metrics| metrics.colors_used == 0 || metrics.colors_used > 128)
    {
        return Err(StylizeError::Invalid(
            "reference repair candidate metrics are invalid".into(),
        ));
    }
    let report_json = canonical_report_json(report)?;
    Ok(RepairRunReport {
        report_sha256: sha256_bytes(report_json.as_bytes()),
        image_count: report.images.len(),
        estimated_peak_working_bytes: report.estimated_peak_working_bytes,
        qualified_for_expansion: report.gates.qualified_for_expansion,
    })
}

fn validate_capture_contract(manifest: &ReferenceManifest) -> Result<(), StylizeError> {
    if manifest.capture.provider != "google-photorealistic-3d-tiles"
        || manifest.camera.projection != "orthographic"
        || manifest.camera.azimuth_millidegrees != 330_000
        || manifest.camera.elevation_millidegrees != 42_000
        || manifest.tile.millimeters_per_pixel != 125
        || !u32::from(manifest.tile.core_width_px).is_multiple_of(2)
        || !u32::from(manifest.tile.core_height_px).is_multiple_of(2)
    {
        return Err(StylizeError::Invalid(
            "reference repair requires the qualified 330/42 degree, 125 mm Google capture".into(),
        ));
    }
    Ok(())
}

fn decode_core_png(
    root: &Path,
    manifest: &ReferenceManifest,
    kind: LayerKind,
) -> Result<RgbImage, StylizeError> {
    let record = manifest
        .layers
        .iter()
        .find(|record| record.kind == kind)
        .ok_or_else(|| StylizeError::Invalid("required reference PNG is missing".into()))?;
    let mut decoder = png::Decoder::new(BufReader::new(File::open(root.join(&record.path))?));
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info()?;
    let info = reader.info();
    if info.width != record.width_px
        || info.height != record.height_px
        || info.color_type != png::ColorType::Rgba
        || info.bit_depth != png::BitDepth::Eight
        || info.interlaced
    {
        return Err(StylizeError::Invalid(
            "reference PNG violates its registered RGBA8 contract".into(),
        ));
    }
    let source_width = usize::try_from(record.width_px)
        .map_err(|_| StylizeError::Invalid("reference width overflowed usize".into()))?;
    let guard = usize::from(manifest.tile.guard_px);
    let width = usize::from(manifest.tile.core_width_px);
    let height = usize::from(manifest.tile.core_height_px);
    let mut pixels = Vec::with_capacity(width * height);
    let expected_row_bytes = source_width
        .checked_mul(4)
        .ok_or_else(|| StylizeError::Invalid("reference PNG row overflowed memory".into()))?;
    let mut source_y = 0_usize;
    while let Some(row) = reader.next_row()? {
        if row.data().len() != expected_row_bytes {
            return Err(StylizeError::Invalid(
                "reference PNG row violates its registered width".into(),
            ));
        }
        if source_y >= guard && source_y < guard + height {
            for x in guard..guard + width {
                let offset = x * 4;
                pixels.push([
                    row.data()[offset],
                    row.data()[offset + 1],
                    row.data()[offset + 2],
                ]);
            }
        }
        source_y += 1;
    }
    if source_y != usize::try_from(record.height_px).expect("bounded reference height") {
        return Err(StylizeError::Invalid(
            "reference PNG row count violates its registration".into(),
        ));
    }
    RgbImage::new(width, height, pixels)
}

fn read_core_depth(root: &Path, manifest: &ReferenceManifest) -> Result<Vec<u32>, StylizeError> {
    let record = manifest
        .layers
        .iter()
        .find(|record| record.kind == LayerKind::LinearDepth)
        .ok_or_else(|| StylizeError::Invalid("required reference depth is missing".into()))?;
    let mut reader = BufReader::with_capacity(64 * 1_024, File::open(root.join(&record.path))?);
    let mut header = [0_u8; 16];
    reader.read_exact(&mut header)?;
    let width = u32::from_le_bytes(header[8..12].try_into().expect("fixed depth width"));
    let height = u32::from_le_bytes(header[12..16].try_into().expect("fixed depth height"));
    if &header[..8] != DEPTH_MAGIC || width != record.width_px || height != record.height_px {
        return Err(StylizeError::Invalid(
            "reference depth header violates its registration".into(),
        ));
    }
    let guard = u64::from(manifest.tile.guard_px);
    let core_width = usize::from(manifest.tile.core_width_px);
    let core_height = usize::from(manifest.tile.core_height_px);
    let mut output = Vec::with_capacity(core_width * core_height);
    let mut row = vec![0_u8; core_width * 4];
    for y in 0..core_height {
        let source_y = guard + u64::try_from(y).expect("bounded source row");
        let offset = 16_u64
            .checked_add(
                source_y
                    .checked_mul(u64::from(width))
                    .and_then(|value| value.checked_add(guard))
                    .and_then(|value| value.checked_mul(4))
                    .ok_or_else(|| StylizeError::Invalid("depth crop offset overflowed".into()))?,
            )
            .ok_or_else(|| StylizeError::Invalid("depth crop offset overflowed".into()))?;
        reader.seek(SeekFrom::Start(offset))?;
        reader.read_exact(&mut row)?;
        output.extend(
            row.chunks_exact(4)
                .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("fixed depth pixel"))),
        );
    }
    Ok(output)
}

fn write_png(
    root: &Path,
    id: &str,
    label: &str,
    image: &RgbImage,
) -> Result<ReviewImageRecord, StylizeError> {
    let path = format!("{id}.png");
    if !OUTPUT_IMAGE_NAMES.contains(&path.as_str()) {
        return Err(StylizeError::Invalid(
            "reference repair output path is not allowlisted".into(),
        ));
    }
    let raw_path = root.join(format!(".{id}.rgba"));
    let mut writer = BufWriter::with_capacity(
        64 * 1_024,
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&raw_path)?,
    );
    for pixel in &image.pixels {
        writer.write_all(&[pixel[0], pixel[1], pixel[2], 0xff])?;
    }
    writer.flush()?;
    writer.get_ref().sync_all()?;
    drop(writer);
    let output_path = root.join(&path);
    let width_px = u32::try_from(image.width)
        .map_err(|_| StylizeError::Invalid("output width overflowed u32".into()))?;
    let height_px = u32::try_from(image.height)
        .map_err(|_| StylizeError::Invalid("output height overflowed u32".into()))?;
    let byte_length = isometric_reference::encode_raw_png(
        &raw_path,
        &output_path,
        width_px,
        height_px,
        PngColorType::Rgba,
    )?;
    fs::remove_file(raw_path)?;
    Ok(ReviewImageRecord {
        id: id.into(),
        label: label.into(),
        path,
        width_px,
        height_px,
        byte_length,
        sha256: sha256_file(&output_path)?,
    })
}

fn measure_candidate(
    id: &str,
    source: &RgbImage,
    candidate: &RgbImage,
    structural: &[bool],
    canopy: &[bool],
) -> CandidateMetrics {
    let edges = output_edges(candidate);
    let structural_count = structural
        .iter()
        .enumerate()
        .filter(|(index, value)| {
            **value && !is_mask_interior(canopy, *index, source.width, source.height)
        })
        .count();
    let recalled = structural
        .iter()
        .enumerate()
        .filter(|(index, value)| {
            **value
                && !is_mask_interior(canopy, *index, source.width, source.height)
                && has_edge_near(&edges, *index, source.width, source.height)
        })
        .count();
    let non_structural = edges
        .iter()
        .enumerate()
        .filter(|(index, value)| {
            **value
                && !has_accepted_structural_near(
                    structural,
                    canopy,
                    *index,
                    source.width,
                    source.height,
                )
        })
        .count();
    let changed = source
        .pixels
        .iter()
        .zip(&candidate.pixels)
        .filter(|(left, right)| left != right)
        .count();
    let canopy_interior = canopy
        .iter()
        .enumerate()
        .filter(|(index, value)| {
            **value && is_mask_interior(canopy, *index, source.width, source.height)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let canopy_edges = canopy_interior
        .iter()
        .filter(|index| edges[**index])
        .count();
    let colors = candidate
        .pixels
        .iter()
        .copied()
        .collect::<BTreeSet<Rgb>>()
        .len();
    let luma_sum = candidate
        .pixels
        .iter()
        .copied()
        .map(luminance)
        .map(u64::from)
        .sum::<u64>();
    CandidateMetrics {
        candidate_id: id.into(),
        colors_used: u16::try_from(colors).expect("bounded palette count"),
        structural_edge_recall_basis_points: u16::try_from(ratio(
            recalled,
            structural_count,
            10_000,
        ))
        .expect("basis-point ratio fits"),
        non_structural_edge_ppm: ratio(non_structural, source.pixels.len(), 1_000_000),
        mean_luminance_microunits: u32::try_from(
            luma_sum * 1_000_000 / u64::try_from(candidate.pixels.len()).expect("pixel count fits"),
        )
        .expect("u8 mean in micro-units fits u32"),
        changed_from_source_ppm: ratio(changed, source.pixels.len(), 1_000_000),
        canopy_interior_edge_ppm: ratio(canopy_edges, canopy_interior.len(), 1_000_000),
    }
}

fn has_edge_near(mask: &[bool], index: usize, width: usize, height: usize) -> bool {
    let x = index % width;
    let y = index / width;
    for dy in -1_isize..=1 {
        for dx in -1_isize..=1 {
            let Some(nx) = x.checked_add_signed(dx) else {
                continue;
            };
            let Some(ny) = y.checked_add_signed(dy) else {
                continue;
            };
            if nx < width && ny < height && mask[ny * width + nx] {
                return true;
            }
        }
    }
    false
}

fn has_accepted_structural_near(
    structural: &[bool],
    canopy: &[bool],
    index: usize,
    width: usize,
    height: usize,
) -> bool {
    let x = index % width;
    let y = index / width;
    for dy in -1_isize..=1 {
        for dx in -1_isize..=1 {
            let Some(nx) = x.checked_add_signed(dx) else {
                continue;
            };
            let Some(ny) = y.checked_add_signed(dy) else {
                continue;
            };
            if nx < width && ny < height {
                let neighbor = ny * width + nx;
                if structural[neighbor] && !is_mask_interior(canopy, neighbor, width, height) {
                    return true;
                }
            }
        }
    }
    false
}

fn is_mask_interior(mask: &[bool], index: usize, width: usize, height: usize) -> bool {
    let x = index % width;
    let y = index / width;
    x > 0
        && y > 0
        && x + 1 < width
        && y + 1 < height
        && mask[index - 1]
        && mask[index + 1]
        && mask[index - width]
        && mask[index + width]
}

fn ratio(numerator: usize, denominator: usize, scale: u64) -> u32 {
    if denominator == 0 {
        return 0;
    }
    u32::try_from(
        (u64::try_from(numerator).expect("bounded numerator") * scale
            / u64::try_from(denominator).expect("bounded denominator"))
        .min(u64::from(u32::MAX)),
    )
    .expect("clamped ratio fits")
}

fn estimate_peak_bytes(pixel_count: usize) -> Result<u64, StylizeError> {
    let logical = u64::try_from(pixel_count)
        .map_err(|_| StylizeError::Invalid("logical pixel count overflowed".into()))?;
    // Row-streamed source decode plus retained core layers is the input peak.
    // The conservative bound includes two 2048 RGB cores, one depth core, and
    // the simultaneously live logical working planes.
    let source_decode = 2_560_u64 * 4;
    let source_cores = 2 * 2_048_u64 * 2_048 * 3 + 2_048_u64 * 2_048 * 4;
    let logical_buffers = logical * (3 * 6 + 4 + 4);
    source_decode
        .checked_add(source_cores)
        .and_then(|value| value.checked_add(logical_buffers))
        .ok_or_else(|| StylizeError::Invalid("working memory estimate overflowed".into()))
}

fn canonical_report_json(report: &RepairReviewReport) -> Result<String, StylizeError> {
    let mut encoded = serde_json::to_string_pretty(report)?;
    encoded.push('\n');
    if encoded.len() > MAX_REPORT_BYTES {
        return Err(StylizeError::Invalid(
            "reference repair report exceeds one MiB".into(),
        ));
    }
    Ok(encoded)
}

fn validate_png_header(path: &Path, width: u32, height: u32) -> Result<(), StylizeError> {
    let decoder = png::Decoder::new(BufReader::new(File::open(path)?));
    let reader = decoder.read_info()?;
    let info = reader.info();
    if info.width != width
        || info.height != height
        || info.color_type != png::ColorType::Rgba
        || info.bit_depth != png::BitDepth::Eight
        || info.interlaced
    {
        return Err(StylizeError::Invalid(
            "reference repair PNG header is invalid".into(),
        ));
    }
    Ok(())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), StylizeError> {
    let mut writer = BufWriter::with_capacity(
        64 * 1_024,
        OpenOptions::new().create_new(true).write(true).open(path)?,
    );
    writer.write_all(bytes)?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, StylizeError> {
    let mut reader = BufReader::with_capacity(64 * 1_024, File::open(path)?);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1_024].into_boxed_slice();
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex_digest(hasher.finalize()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex_digest(Sha256::digest(bytes))
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = bytes.as_ref();
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use isometric_reference::{
        CameraSpec, CaptureSpec, LayerRecord, LightingSpec, ReferenceManifest, TileSpec,
        canonical_manifest_json,
    };

    fn root(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "isometric-stylize-{name}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("fixture root");
        path
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the complete six-layer fixture remains auditable in one test helper"
    )]
    fn write_fixture(path: &Path) {
        let tile = TileSpec {
            region_id: "synthetic-hoover".into(),
            column: 0,
            row: 0,
            core_width_px: 8,
            core_height_px: 8,
            guard_px: 2,
            millimeters_per_pixel: 125,
            center_longitude_e7: -1_221_670_000,
            center_latitude_e7: 374_276_111,
        };
        let width = tile.total_width_px();
        let height = tile.total_height_px();
        let mut layers = Vec::new();
        for kind in [
            LayerKind::Color,
            LayerKind::Whitebox,
            LayerKind::LinearDepth,
            LayerKind::ViewNormal,
            LayerKind::FixedShadow,
            LayerKind::Coverage,
        ] {
            let output = path.join(kind.filename());
            if kind == LayerKind::LinearDepth {
                let mut bytes = Vec::new();
                bytes.extend_from_slice(DEPTH_MAGIC);
                bytes.extend_from_slice(&width.to_le_bytes());
                bytes.extend_from_slice(&height.to_le_bytes());
                for y in 0..height {
                    for x in 0..width {
                        let value = 1_000_000 + x * 100 + y * 100;
                        bytes.extend_from_slice(&value.to_le_bytes());
                    }
                }
                fs::write(&output, bytes).expect("fixture depth");
            } else {
                let color = match kind {
                    LayerKind::FixedShadow | LayerKind::Coverage => PngColorType::Grayscale,
                    _ => PngColorType::Rgba,
                };
                let channels = match color {
                    PngColorType::Grayscale => 1,
                    PngColorType::Rgba => 4,
                };
                let raw = path.join(format!("{}.raw", kind.filename()));
                let mut pixels = Vec::new();
                for y in 0..height {
                    for x in 0..width {
                        if channels == 1 {
                            pixels.push(255);
                        } else if kind == LayerKind::ViewNormal {
                            pixels.extend_from_slice(&[128, 220, 215, 255]);
                        } else {
                            let green = if x > 3 && x < 9 && y > 3 && y < 9 {
                                [45, 75, 40, 255]
                            } else {
                                [150, 80, 60, 255]
                            };
                            pixels.extend_from_slice(&green);
                        }
                    }
                }
                fs::write(&raw, pixels).expect("fixture raw");
                isometric_reference::encode_raw_png(&raw, &output, width, height, color)
                    .expect("fixture PNG");
                fs::remove_file(raw).expect("remove fixture raw");
            }
            layers.push(LayerRecord {
                kind,
                path: kind.filename().into(),
                encoding: kind.encoding().into(),
                width_px: width,
                height_px: height,
                byte_length: output.metadata().expect("fixture metadata").len(),
                sha256: sha256_file(&output).expect("fixture hash"),
            });
        }
        let manifest = ReferenceManifest {
            schema: isometric_reference::MANIFEST_SCHEMA.into(),
            bundle_id: "synthetic-hoover-repair".into(),
            tile,
            camera: CameraSpec {
                projection: "orthographic".into(),
                azimuth_millidegrees: 330_000,
                elevation_millidegrees: 42_000,
                target_altitude_mm: 20_000,
                near_mm: 1_000,
                far_mm: 5_000_000,
                orthographic_width_mm: 1_500,
                orthographic_height_mm: 1_500,
                camera_distance_mm: 2_000_000,
            },
            lighting: LightingSpec {
                sun_azimuth_millidegrees: 315_000,
                sun_elevation_millidegrees: 42_000,
            },
            capture: CaptureSpec {
                renderer: "threejs-google-3d-tiles".into(),
                renderer_version: "synthetic-test".into(),
                provider: "google-photorealistic-3d-tiles".into(),
                source_epoch: "synthetic".into(),
                complete: true,
                attributions: vec!["Google Maps".into()],
            },
            core_coverage_basis_points: 10_000,
            layers,
        };
        fs::write(
            path.join(isometric_reference::MANIFEST_FILENAME),
            canonical_manifest_json(&manifest).expect("fixture manifest"),
        )
        .expect("write fixture manifest");
    }

    #[test]
    fn synthetic_experiment_is_byte_deterministic_and_bounded() {
        let source = root("source");
        write_fixture(&source);
        let first = root("first-parent").join("result");
        let second = root("second-parent").join("result");
        let first_report = run_reference_repair(&source, &first).expect("first run");
        let second_report = run_reference_repair(&source, &second).expect("second run");
        assert_eq!(first_report, second_report);
        assert!(first_report.estimated_peak_working_bytes < 96 * 1_024 * 1_024);
        assert!(!first_report.qualified_for_expansion);
        for name in OUTPUT_IMAGE_NAMES {
            assert_eq!(
                fs::read(first.join(name)).expect("first output"),
                fs::read(second.join(name)).expect("second output")
            );
        }
        assert_eq!(
            fs::read(first.join(REVIEW_FILENAME)).expect("first report"),
            fs::read(second.join(REVIEW_FILENAME)).expect("second report")
        );
    }

    #[test]
    fn report_validation_rejects_a_mutated_image() {
        let source = root("mutation-source");
        write_fixture(&source);
        let output = root("mutation-parent").join("result");
        run_reference_repair(&source, &output).expect("run");
        let report = read_report(&output.join(REVIEW_FILENAME)).expect("read report");
        let image = output.join(&report.images[0].path);
        let mut bytes = fs::read(&image).expect("read image");
        bytes.push(0);
        fs::write(image, bytes).expect("mutate image");
        assert!(validate_report(&output, &report).is_err());
    }
}

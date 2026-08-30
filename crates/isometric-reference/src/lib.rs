//! Registered, content-addressed reference-layer contracts.
//!
//! Reference capture is intentionally separate from canonical stylization.
//! This crate validates that every captured layer uses one pixel grid and one
//! orthographic camera before any mask or art algorithm may consume it.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt::{Display, Formatter, Write as _},
    fs::{File, OpenOptions, remove_file},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write as IoWrite},
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Portable reference-manifest schema.
pub const MANIFEST_SCHEMA: &str = "isometric-reference-manifest/v2";
/// Canonical manifest filename inside a reference bundle.
pub const MANIFEST_FILENAME: &str = "reference.manifest.json";
/// Minimum accepted valid-pixel coverage for a pilot core.
pub const MIN_CORE_COVERAGE_BASIS_POINTS: u16 = 9_950;

const HASH_BUFFER_BYTES: usize = 64 * 1024;
const MAX_CAPTURE_DIMENSION: u32 = 4_096;
const DEPTH_MAGIC: &[u8; 8] = b"ISOD32V1";
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const STORED_DEFLATE_BLOCK_BYTES: usize = 65_535;

/// Portable PNG color layouts accepted by the reference encoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PngColorType {
    /// One 8-bit grayscale channel.
    Grayscale,
    /// Four 8-bit red, green, blue, and alpha channels.
    Rgba,
}

impl PngColorType {
    const fn channels(self) -> u64 {
        match self {
            Self::Grayscale => 1,
            Self::Rgba => 4,
        }
    }

    const fn png_value(self) -> u8 {
        match self {
            Self::Grayscale => 0,
            Self::Rgba => 6,
        }
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                0xedb8_8320 ^ (crc >> 1)
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn write_png_chunk(
    writer: &mut BufWriter<File>,
    kind: [u8; 4],
    data: &[u8],
) -> Result<(), ReferenceError> {
    let length = u32::try_from(data.len())
        .map_err(|_| ReferenceError::Invalid("PNG chunk exceeds u32 length".into()))?;
    writer.write_all(&length.to_be_bytes())?;
    writer.write_all(&kind)?;
    writer.write_all(data)?;
    let mut crc_input = Vec::with_capacity(kind.len() + data.len());
    crc_input.extend_from_slice(&kind);
    crc_input.extend_from_slice(data);
    writer.write_all(&crc32(&crc_input).to_be_bytes())?;
    Ok(())
}

fn update_adler32(s1: &mut u32, s2: &mut u32, bytes: &[u8]) {
    const MODULUS: u32 = 65_521;
    for byte in bytes {
        *s1 = (*s1 + u32::from(*byte)) % MODULUS;
        *s2 = (*s2 + *s1) % MODULUS;
    }
}

/// Encode a tightly packed raw image as a deterministic, bounded-memory PNG.
///
/// The encoder uses filter zero and stored DEFLATE blocks. Reference bundles
/// prioritize exactness, bounded memory, and reproducibility over compression;
/// release DZI tiles are encoded later by the publication pipeline.
///
/// # Errors
///
/// Returns an error when dimensions are invalid, the raw file length does not
/// match the declared layout, the destination exists, or local I/O fails.
pub fn encode_raw_png(
    raw_path: &Path,
    output_path: &Path,
    width: u32,
    height: u32,
    color_type: PngColorType,
) -> Result<u64, ReferenceError> {
    encode_raw_png_crop(
        raw_path,
        output_path,
        width,
        height,
        0,
        0,
        width,
        height,
        color_type,
    )
}

struct PngCropLayout {
    crop_stride: u64,
    expected_raw_length: u64,
    source_stride: u64,
}

struct PartialPng<'a> {
    keep: bool,
    path: &'a Path,
}

impl Drop for PartialPng<'_> {
    fn drop(&mut self) {
        if !self.keep {
            let _ = remove_file(self.path);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn png_crop_layout(
    source_width: u32,
    source_height: u32,
    crop_x: u32,
    crop_y: u32,
    crop_width: u32,
    crop_height: u32,
    color_type: PngColorType,
) -> Result<PngCropLayout, ReferenceError> {
    if source_width == 0
        || source_height == 0
        || crop_width == 0
        || crop_height == 0
        || source_width > MAX_CAPTURE_DIMENSION
        || source_height > MAX_CAPTURE_DIMENSION
        || crop_x
            .checked_add(crop_width)
            .is_none_or(|right| right > source_width)
        || crop_y
            .checked_add(crop_height)
            .is_none_or(|bottom| bottom > source_height)
    {
        return Err(ReferenceError::Invalid(
            "PNG source or crop dimensions violate the reference capture bound".into(),
        ));
    }
    let source_stride = u64::from(source_width)
        .checked_mul(color_type.channels())
        .ok_or_else(|| ReferenceError::Invalid("PNG source row length overflowed".into()))?;
    let crop_stride = u64::from(crop_width)
        .checked_mul(color_type.channels())
        .ok_or_else(|| ReferenceError::Invalid("PNG crop row length overflowed".into()))?;
    let expected_raw_length = source_stride
        .checked_mul(u64::from(source_height))
        .ok_or_else(|| ReferenceError::Invalid("PNG image length overflowed".into()))?;
    Ok(PngCropLayout {
        crop_stride,
        expected_raw_length,
        source_stride,
    })
}

/// Crop a tightly packed raw image while encoding it as deterministic PNG.
///
/// # Errors
///
/// Returns an error when the source or crop geometry is invalid, the raw file
/// length is inconsistent, the destination exists, or local I/O fails.
#[allow(clippy::too_many_arguments)]
pub fn encode_raw_png_crop(
    raw_path: &Path,
    output_path: &Path,
    source_width: u32,
    source_height: u32,
    crop_x: u32,
    crop_y: u32,
    crop_width: u32,
    crop_height: u32,
    color_type: PngColorType,
) -> Result<u64, ReferenceError> {
    let layout = png_crop_layout(
        source_width,
        source_height,
        crop_x,
        crop_y,
        crop_width,
        crop_height,
        color_type,
    )?;
    if raw_path.metadata()?.len() != layout.expected_raw_length {
        return Err(ReferenceError::Invalid(
            "raw PNG source length contradicts its dimensions".into(),
        ));
    }

    let mut input = BufReader::with_capacity(64 * 1024, File::open(raw_path)?);
    let output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output_path)?;
    let mut partial = PartialPng {
        keep: false,
        path: output_path,
    };
    let mut writer = BufWriter::with_capacity(64 * 1024, output);
    writer.write_all(PNG_SIGNATURE)?;
    let mut header = [0_u8; 13];
    header[0..4].copy_from_slice(&crop_width.to_be_bytes());
    header[4..8].copy_from_slice(&crop_height.to_be_bytes());
    header[8] = 8;
    header[9] = color_type.png_value();
    write_png_chunk(&mut writer, *b"IHDR", &header)?;

    let filtered_length = (layout.crop_stride + 1)
        .checked_mul(u64::from(crop_height))
        .ok_or_else(|| ReferenceError::Invalid("filtered PNG length overflowed".into()))?;
    let mut remaining = filtered_length;
    let mut row_remaining = 0_u64;
    let mut row_index = 0_u32;
    let mut first_block = true;
    let mut adler_s1 = 1_u32;
    let mut adler_s2 = 0_u32;
    while remaining > 0 {
        let block_length = usize::try_from(remaining.min(STORED_DEFLATE_BLOCK_BYTES as u64))
            .map_err(|_| ReferenceError::Invalid("PNG block length overflowed".into()))?;
        let mut block = Vec::with_capacity(block_length);
        while block.len() < block_length {
            if row_remaining == 0 {
                let source_row = crop_y
                    .checked_add(row_index)
                    .ok_or_else(|| ReferenceError::Invalid("PNG crop row overflowed".into()))?;
                let source_offset = u64::from(source_row)
                    .checked_mul(layout.source_stride)
                    .and_then(|offset| {
                        u64::from(crop_x)
                            .checked_mul(color_type.channels())
                            .and_then(|column| offset.checked_add(column))
                    })
                    .ok_or_else(|| ReferenceError::Invalid("PNG crop offset overflowed".into()))?;
                input.seek(SeekFrom::Start(source_offset))?;
                block.push(0);
                row_remaining = layout.crop_stride;
                row_index += 1;
                continue;
            }
            let available = block_length - block.len();
            let count = usize::try_from(row_remaining.min(available as u64))
                .map_err(|_| ReferenceError::Invalid("PNG row segment overflowed".into()))?;
            let start = block.len();
            block.resize(start + count, 0);
            input.read_exact(&mut block[start..])?;
            row_remaining -= count as u64;
        }
        update_adler32(&mut adler_s1, &mut adler_s2, &block);
        remaining -= block_length as u64;
        let final_block = remaining == 0;
        let length = u16::try_from(block_length)
            .map_err(|_| ReferenceError::Invalid("stored PNG block exceeds u16".into()))?;
        let mut idat = Vec::with_capacity(block.len() + 11);
        if first_block {
            idat.extend_from_slice(&[0x78, 0x01]);
            first_block = false;
        }
        idat.push(u8::from(final_block));
        idat.extend_from_slice(&length.to_le_bytes());
        idat.extend_from_slice(&(!length).to_le_bytes());
        idat.extend_from_slice(&block);
        if final_block {
            idat.extend_from_slice(&((adler_s2 << 16) | adler_s1).to_be_bytes());
        }
        write_png_chunk(&mut writer, *b"IDAT", &idat)?;
    }
    write_png_chunk(&mut writer, *b"IEND", &[])?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    drop(writer);
    let length = output_path.metadata()?.len();
    partial.keep = true;
    Ok(length)
}

/// One guarded capture region in a stable world grid.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TileSpec {
    /// Stable region identifier.
    pub region_id: String,
    /// Stable supertile column.
    pub column: i32,
    /// Stable supertile row.
    pub row: i32,
    /// Saved core width before downstream art-cell subdivision.
    pub core_width_px: u16,
    /// Saved core height before downstream art-cell subdivision.
    pub core_height_px: u16,
    /// Context rendered outside every core edge.
    pub guard_px: u16,
    /// Ground scale in integer millimeters.
    pub millimeters_per_pixel: u32,
    /// Longitude of the orthographic target in degrees times ten million.
    pub center_longitude_e7: i32,
    /// Latitude of the orthographic target in degrees times ten million.
    pub center_latitude_e7: i32,
}

impl TileSpec {
    /// Total registered layer width including both guards.
    #[must_use]
    pub fn total_width_px(&self) -> u32 {
        u32::from(self.core_width_px) + 2 * u32::from(self.guard_px)
    }

    /// Total registered layer height including both guards.
    #[must_use]
    pub fn total_height_px(&self) -> u32 {
        u32::from(self.core_height_px) + 2 * u32::from(self.guard_px)
    }
}

/// Integer orthographic camera shared by every registered layer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraSpec {
    /// Projection contract. Version one accepts only `orthographic`.
    pub projection: String,
    /// Clockwise azimuth in degrees times one thousand.
    pub azimuth_millidegrees: u32,
    /// Elevation above the horizon in degrees times one thousand.
    pub elevation_millidegrees: u32,
    /// Camera target altitude relative to the reference ellipsoid.
    pub target_altitude_mm: i64,
    /// Positive near clipping plane in millimeters.
    pub near_mm: u64,
    /// Far clipping plane in millimeters.
    pub far_mm: u64,
    /// Exact horizontal orthographic span.
    pub orthographic_width_mm: u64,
    /// Exact vertical orthographic span.
    pub orthographic_height_mm: u64,
    /// Camera distance from the geographic target along the view direction.
    pub camera_distance_mm: u64,
}

/// Fixed project lighting used for the registered shadow pass.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LightingSpec {
    /// Clockwise sun azimuth in degrees times one thousand.
    pub sun_azimuth_millidegrees: u32,
    /// Sun elevation in degrees times one thousand.
    pub sun_elevation_millidegrees: u32,
}

/// Capture implementation and upstream identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CaptureSpec {
    /// Stable renderer implementation identifier.
    pub renderer: String,
    /// Pinned renderer build or commit.
    pub renderer_version: String,
    /// Reference provider identifier.
    pub provider: String,
    /// Provider epoch or acquisition timestamp.
    pub source_epoch: String,
    /// Whether the capture reached the strict readiness contract.
    pub complete: bool,
    /// Provider attribution records visible during reference capture.
    pub attributions: Vec<String>,
}

/// Registered layer type.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayerKind {
    /// Textured color render.
    Color,
    /// Neutral untextured geometry.
    Whitebox,
    /// Linear camera depth in integer millimeters.
    LinearDepth,
    /// Encoded view-space surface normals.
    ViewNormal,
    /// Shadow-only pass generated with [`LightingSpec`].
    FixedShadow,
    /// Valid-source coverage mask.
    Coverage,
}

impl LayerKind {
    /// Required bundle-relative filename.
    #[must_use]
    pub const fn filename(self) -> &'static str {
        match self {
            Self::Color => "color.png",
            Self::Whitebox => "whitebox.png",
            Self::LinearDepth => "depth.bin",
            Self::ViewNormal => "normal.png",
            Self::FixedShadow => "fixed-shadow.png",
            Self::Coverage => "coverage.png",
        }
    }

    /// Required portable encoding identifier.
    #[must_use]
    pub const fn encoding(self) -> &'static str {
        match self {
            Self::Color | Self::Whitebox | Self::ViewNormal => "png-rgba8",
            Self::LinearDepth => "raw-u32le-millimeters",
            Self::FixedShadow | Self::Coverage => "png-gray8",
        }
    }

    const fn png_color_type(self) -> Option<u8> {
        match self {
            Self::Color | Self::Whitebox | Self::ViewNormal => Some(6),
            Self::FixedShadow | Self::Coverage => Some(0),
            Self::LinearDepth => None,
        }
    }
}

/// One immutable registered-layer record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LayerRecord {
    /// Semantic layer identity.
    pub kind: LayerKind,
    /// Safe path relative to the bundle directory.
    pub path: String,
    /// Portable encoding identifier.
    pub encoding: String,
    /// Registered image width.
    pub width_px: u32,
    /// Registered image height.
    pub height_px: u32,
    /// Exact file length.
    pub byte_length: u64,
    /// Lowercase SHA-256 over exact file bytes.
    pub sha256: String,
}

/// Complete immutable contract for one registered capture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReferenceManifest {
    /// Schema identifier.
    pub schema: String,
    /// Stable content role identifier.
    pub bundle_id: String,
    /// Guarded world-grid region.
    pub tile: TileSpec,
    /// Shared orthographic camera.
    pub camera: CameraSpec,
    /// Shared fixed lighting.
    pub lighting: LightingSpec,
    /// Capture implementation identity.
    pub capture: CaptureSpec,
    /// Valid-pixel coverage inside the saved core.
    pub core_coverage_basis_points: u16,
    /// Exactly one record for every [`LayerKind`].
    pub layers: Vec<LayerRecord>,
}

/// Verified bundle evidence returned to downstream stages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleReport {
    /// SHA-256 over canonical manifest JSON.
    pub manifest_sha256: String,
    /// Verified layer digests keyed by stable layer identity.
    pub layer_sha256: BTreeMap<LayerKind, String>,
    /// Sum of exact registered layer bytes.
    pub total_layer_bytes: u64,
}

/// Fail-closed reference contract error.
#[derive(Debug)]
pub enum ReferenceError {
    /// A manifest or artifact violates a reference invariant.
    Invalid(String),
    /// Local I/O failed.
    Io(std::io::Error),
    /// JSON decoding or encoding failed.
    Json(serde_json::Error),
}

impl Display for ReferenceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::Io(error) => write!(formatter, "reference I/O failed: {error}"),
            Self::Json(error) => write!(formatter, "reference JSON failed: {error}"),
        }
    }
}

impl Error for ReferenceError {}

impl From<std::io::Error> for ReferenceError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for ReferenceError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

/// Read a reference manifest without trusting its artifacts.
///
/// # Errors
///
/// Returns an error when the JSON cannot be read or decoded.
pub fn read_manifest(path: &Path) -> Result<ReferenceManifest, ReferenceError> {
    Ok(serde_json::from_reader(BufReader::new(File::open(path)?))?)
}

/// Serialize a manifest into its stable repository representation.
///
/// # Errors
///
/// Returns an error when the manifest contract is invalid or JSON encoding
/// fails.
pub fn canonical_manifest_json(manifest: &ReferenceManifest) -> Result<String, ReferenceError> {
    validate_manifest(manifest)?;
    let mut encoded = serde_json::to_string_pretty(manifest)?;
    encoded.push('\n');
    Ok(encoded)
}

/// Validate one complete reference bundle and all content hashes.
///
/// # Errors
///
/// Returns an error for any invalid contract, unsafe path, wrong length,
/// incorrect digest, malformed layer header, or registration mismatch.
pub fn validate_bundle(
    root: &Path,
    manifest: &ReferenceManifest,
) -> Result<BundleReport, ReferenceError> {
    validate_manifest(manifest)?;
    let expected_width = manifest.tile.total_width_px();
    let expected_height = manifest.tile.total_height_px();
    let mut layer_sha256 = BTreeMap::new();
    let mut total_layer_bytes = 0_u64;

    for layer in &manifest.layers {
        let path = root.join(&layer.path);
        let metadata = path.symlink_metadata()?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() != layer.byte_length
        {
            return Err(ReferenceError::Invalid(format!(
                "reference layer {} byte length does not match",
                layer.kind.filename()
            )));
        }
        let digest = sha256_file(&path)?;
        if digest != layer.sha256 {
            return Err(ReferenceError::Invalid(format!(
                "reference layer {} SHA-256 does not match",
                layer.kind.filename()
            )));
        }
        let (width, height) = match layer.kind.png_color_type() {
            Some(color_type) => read_png_header(&path, color_type)?,
            None => read_depth_header(&path)?,
        };
        if width != expected_width
            || height != expected_height
            || width != layer.width_px
            || height != layer.height_px
        {
            return Err(ReferenceError::Invalid(format!(
                "reference layer {} is not registered to the shared grid",
                layer.kind.filename()
            )));
        }
        total_layer_bytes = total_layer_bytes
            .checked_add(layer.byte_length)
            .ok_or_else(|| ReferenceError::Invalid("layer byte total overflowed".into()))?;
        layer_sha256.insert(layer.kind, digest);
    }

    let manifest_json = canonical_manifest_json(manifest)?;
    Ok(BundleReport {
        manifest_sha256: sha256_bytes(manifest_json.as_bytes()),
        layer_sha256,
        total_layer_bytes,
    })
}

fn validate_manifest(manifest: &ReferenceManifest) -> Result<(), ReferenceError> {
    if manifest.schema != MANIFEST_SCHEMA {
        return Err(ReferenceError::Invalid(format!(
            "reference schema must be {MANIFEST_SCHEMA}"
        )));
    }
    if !is_identifier(&manifest.bundle_id) || !is_identifier(&manifest.tile.region_id) {
        return Err(ReferenceError::Invalid(
            "bundle and region IDs must use lowercase safe identifiers".into(),
        ));
    }
    let width = manifest.tile.total_width_px();
    let height = manifest.tile.total_height_px();
    validate_tile_and_camera(manifest, width, height)?;
    validate_capture(manifest)?;
    validate_layers(manifest, width, height)
}

fn validate_tile_and_camera(
    manifest: &ReferenceManifest,
    width: u32,
    height: u32,
) -> Result<(), ReferenceError> {
    if manifest.tile.core_width_px == 0
        || manifest.tile.core_height_px == 0
        || manifest.tile.guard_px == 0
        || width > MAX_CAPTURE_DIMENSION
        || height > MAX_CAPTURE_DIMENSION
        || manifest.tile.millimeters_per_pixel == 0
    {
        return Err(ReferenceError::Invalid(
            "reference tile dimensions, guard, or scale are unsafe".into(),
        ));
    }
    if !(-1_800_000_000..=1_800_000_000).contains(&manifest.tile.center_longitude_e7)
        || !(-900_000_000..=900_000_000).contains(&manifest.tile.center_latitude_e7)
    {
        return Err(ReferenceError::Invalid(
            "reference target longitude or latitude is invalid".into(),
        ));
    }
    if manifest.camera.projection != "orthographic"
        || manifest.camera.azimuth_millidegrees >= 360_000
        || !(1_000..90_000).contains(&manifest.camera.elevation_millidegrees)
        || manifest.camera.near_mm == 0
        || manifest.camera.far_mm <= manifest.camera.near_mm
        || manifest.camera.camera_distance_mm <= manifest.camera.near_mm
        || manifest.camera.camera_distance_mm >= manifest.camera.far_mm
    {
        return Err(ReferenceError::Invalid(
            "reference camera contract is invalid".into(),
        ));
    }
    let expected_width_mm = u64::from(width)
        .checked_mul(u64::from(manifest.tile.millimeters_per_pixel))
        .ok_or_else(|| ReferenceError::Invalid("orthographic width overflowed".into()))?;
    let expected_height_mm = u64::from(height)
        .checked_mul(u64::from(manifest.tile.millimeters_per_pixel))
        .ok_or_else(|| ReferenceError::Invalid("orthographic height overflowed".into()))?;
    if manifest.camera.orthographic_width_mm != expected_width_mm
        || manifest.camera.orthographic_height_mm != expected_height_mm
    {
        return Err(ReferenceError::Invalid(
            "orthographic span does not match the registered pixel grid".into(),
        ));
    }
    if manifest.lighting.sun_azimuth_millidegrees >= 360_000
        || !(1_000..90_000).contains(&manifest.lighting.sun_elevation_millidegrees)
    {
        return Err(ReferenceError::Invalid(
            "reference lighting contract is invalid".into(),
        ));
    }
    Ok(())
}

fn validate_capture(manifest: &ReferenceManifest) -> Result<(), ReferenceError> {
    if manifest.capture.renderer.is_empty()
        || manifest.capture.renderer_version.is_empty()
        || manifest.capture.provider != "google-photorealistic-3d-tiles"
        || manifest.capture.source_epoch.is_empty()
        || !manifest.capture.complete
        || manifest.capture.attributions.is_empty()
        || manifest.capture.attributions.len() > 64
        || manifest.capture.attributions.iter().any(|attribution| {
            attribution.is_empty()
                || attribution.len() > 2_048
                || attribution.chars().any(char::is_control)
        })
    {
        return Err(ReferenceError::Invalid(
            "reference capture is incomplete or lacks identity".into(),
        ));
    }
    if !(MIN_CORE_COVERAGE_BASIS_POINTS..=10_000).contains(&manifest.core_coverage_basis_points) {
        return Err(ReferenceError::Invalid(
            "reference core coverage is below the pilot gate".into(),
        ));
    }
    Ok(())
}

fn validate_layers(
    manifest: &ReferenceManifest,
    width: u32,
    height: u32,
) -> Result<(), ReferenceError> {
    let expected_kinds = [
        LayerKind::Color,
        LayerKind::Whitebox,
        LayerKind::LinearDepth,
        LayerKind::ViewNormal,
        LayerKind::FixedShadow,
        LayerKind::Coverage,
    ];
    if manifest.layers.len() != expected_kinds.len()
        || manifest
            .layers
            .iter()
            .map(|layer| layer.kind)
            .ne(expected_kinds)
    {
        return Err(ReferenceError::Invalid(
            "reference manifest must contain each required layer exactly once".into(),
        ));
    }
    for layer in &manifest.layers {
        if layer.path != layer.kind.filename()
            || layer.encoding != layer.kind.encoding()
            || layer.width_px != width
            || layer.height_px != height
            || layer.byte_length == 0
            || !is_sha256(&layer.sha256)
        {
            return Err(ReferenceError::Invalid(format!(
                "reference layer {} violates its portable contract",
                layer.kind.filename()
            )));
        }
    }
    Ok(())
}

fn read_png_header(path: &Path, expected_color_type: u8) -> Result<(u32, u32), ReferenceError> {
    let mut file = File::open(path)?;
    let mut header = [0_u8; 26];
    file.read_exact(&mut header)?;
    if &header[..8] != b"\x89PNG\r\n\x1a\n"
        || &header[8..12] != 13_u32.to_be_bytes().as_slice()
        || &header[12..16] != b"IHDR"
        || header[24] != 8
        || header[25] != expected_color_type
    {
        return Err(ReferenceError::Invalid(format!(
            "reference PNG {} has an invalid IHDR contract",
            path.display()
        )));
    }
    Ok((
        u32::from_be_bytes(header[16..20].try_into().expect("fixed PNG width slice")),
        u32::from_be_bytes(header[20..24].try_into().expect("fixed PNG height slice")),
    ))
}

fn read_depth_header(path: &Path) -> Result<(u32, u32), ReferenceError> {
    let mut file = File::open(path)?;
    let mut header = [0_u8; 16];
    file.read_exact(&mut header)?;
    if &header[..8] != DEPTH_MAGIC {
        return Err(ReferenceError::Invalid(
            "reference depth layer has an invalid magic header".into(),
        ));
    }
    let width = u32::from_le_bytes(header[8..12].try_into().expect("fixed depth width slice"));
    let height = u32::from_le_bytes(header[12..16].try_into().expect("fixed depth height slice"));
    let payload_length = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| ReferenceError::Invalid("reference depth length overflowed".into()))?;
    let expected_length = 16_u64
        .checked_add(payload_length)
        .ok_or_else(|| ReferenceError::Invalid("reference depth length overflowed".into()))?;
    if file.metadata()?.len() != expected_length {
        return Err(ReferenceError::Invalid(
            "reference depth payload length does not match its dimensions".into(),
        ));
    }
    Ok((width, height))
}

fn sha256_file(path: &Path) -> Result<String, ReferenceError> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex_digest(digest.finalize()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex_digest(Sha256::digest(bytes))
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;

    fn fixture_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "isometric-reference-{name}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create reference fixture root");
        root
    }

    fn png_header(width: u32, height: u32, color_type: u8) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, color_type, 0, 0, 0]);
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(b"IEND");
        bytes
    }

    fn depth_bytes(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(DEPTH_MAGIC);
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        let width = usize::try_from(width).expect("fixture width fits usize");
        let height = usize::try_from(height).expect("fixture height fits usize");
        bytes.resize(16 + width * height * 4, 0);
        bytes
    }

    fn write_bundle(root: &Path) -> ReferenceManifest {
        let tile = TileSpec {
            region_id: "hoover-pilot".into(),
            column: 0,
            row: 0,
            core_width_px: 32,
            core_height_px: 32,
            guard_px: 8,
            millimeters_per_pixel: 250,
            center_longitude_e7: -1_221_700_000,
            center_latitude_e7: 374_280_000,
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
            let bytes = kind.png_color_type().map_or_else(
                || depth_bytes(width, height),
                |color_type| png_header(width, height, color_type),
            );
            fs::write(root.join(kind.filename()), &bytes).expect("write fixture layer");
            layers.push(LayerRecord {
                kind,
                path: kind.filename().into(),
                encoding: kind.encoding().into(),
                width_px: width,
                height_px: height,
                byte_length: u64::try_from(bytes.len()).expect("fixture length fits u64"),
                sha256: sha256_bytes(&bytes),
            });
        }
        ReferenceManifest {
            schema: MANIFEST_SCHEMA.into(),
            bundle_id: "hoover-0000-0000".into(),
            camera: CameraSpec {
                projection: "orthographic".into(),
                azimuth_millidegrees: 315_000,
                elevation_millidegrees: 42_000,
                target_altitude_mm: 20_000,
                near_mm: 1_000,
                far_mm: 5_000_000,
                orthographic_width_mm: u64::from(width) * 250,
                orthographic_height_mm: u64::from(height) * 250,
                camera_distance_mm: 2_000_000,
            },
            lighting: LightingSpec {
                sun_azimuth_millidegrees: 315_000,
                sun_elevation_millidegrees: 42_000,
            },
            capture: CaptureSpec {
                renderer: "threejs-google-3d-tiles".into(),
                renderer_version: "fixture-v1".into(),
                provider: "google-photorealistic-3d-tiles".into(),
                source_epoch: "2026-08-18T00:00:00Z".into(),
                complete: true,
                attributions: vec!["copyright:fixture-provider".into()],
            },
            core_coverage_basis_points: 10_000,
            tile,
            layers,
        }
    }

    #[test]
    fn valid_bundle_is_registered_and_manifest_is_deterministic() {
        let root = fixture_root("valid");
        let manifest = write_bundle(&root);
        let first = validate_bundle(&root, &manifest).expect("valid reference bundle");
        let second = validate_bundle(&root, &manifest).expect("repeat valid reference bundle");
        let third = validate_bundle(&root, &manifest).expect("third valid reference bundle");
        assert_eq!(first, second);
        assert_eq!(second, third);
        assert_eq!(first.layer_sha256.len(), 6);
        assert_eq!(
            canonical_manifest_json(&manifest).expect("canonical manifest"),
            canonical_manifest_json(&manifest).expect("repeat canonical manifest")
        );
        fs::remove_dir_all(root).expect("remove fixture root");
    }

    #[test]
    fn raw_png_encoder_is_deterministic_bounded_and_fail_closed() {
        let root = fixture_root("raw-png");
        let raw = root.join("pixels.raw");
        let first = root.join("first.png");
        let second = root.join("second.png");
        let pixels = (0_u8..24).collect::<Vec<_>>();
        fs::write(&raw, &pixels).expect("write raw pixels");
        let first_length =
            encode_raw_png(&raw, &first, 3, 2, PngColorType::Rgba).expect("encode first PNG");
        let second_length =
            encode_raw_png(&raw, &second, 3, 2, PngColorType::Rgba).expect("encode second PNG");
        let first_bytes = fs::read(&first).expect("read first PNG");
        let second_bytes = fs::read(&second).expect("read second PNG");
        assert_eq!(first_bytes, second_bytes);
        assert_eq!(first_length, second_length);
        assert_eq!(&first_bytes[..8], PNG_SIGNATURE);
        assert_eq!(&first_bytes[16..20], &3_u32.to_be_bytes());
        assert_eq!(&first_bytes[20..24], &2_u32.to_be_bytes());
        assert!(encode_raw_png(&raw, &first, 3, 2, PngColorType::Rgba).is_err());

        fs::write(&raw, [0_u8; 3]).expect("truncate raw pixels");
        assert!(encode_raw_png(&raw, &root.join("invalid.png"), 3, 2, PngColorType::Rgba).is_err());
        fs::remove_dir_all(root).expect("remove fixture root");
    }

    #[test]
    fn rejects_incomplete_misaligned_and_low_coverage_manifests() {
        let root = fixture_root("invalid-manifest");
        let manifest = write_bundle(&root);

        let mut invalid = manifest.clone();
        invalid.capture.complete = false;
        assert!(canonical_manifest_json(&invalid).is_err());

        invalid = manifest.clone();
        invalid.capture.attributions.clear();
        assert!(canonical_manifest_json(&invalid).is_err());

        invalid = manifest.clone();
        invalid.camera.orthographic_width_mm += 1;
        assert!(canonical_manifest_json(&invalid).is_err());

        invalid = manifest.clone();
        invalid.camera.camera_distance_mm = invalid.camera.far_mm;
        assert!(canonical_manifest_json(&invalid).is_err());

        invalid = manifest.clone();
        invalid.layers[0].width_px += 1;
        assert!(canonical_manifest_json(&invalid).is_err());

        invalid = manifest.clone();
        invalid.core_coverage_basis_points = MIN_CORE_COVERAGE_BASIS_POINTS - 1;
        assert!(canonical_manifest_json(&invalid).is_err());

        invalid = manifest;
        invalid.layers.pop();
        assert!(canonical_manifest_json(&invalid).is_err());

        let mut invalid = write_bundle(&root);
        invalid.layers.swap(0, 1);
        assert!(canonical_manifest_json(&invalid).is_err());
        fs::remove_dir_all(root).expect("remove fixture root");
    }

    #[test]
    fn rejects_corrupt_layers_and_unsafe_contracts() {
        let root = fixture_root("corrupt");
        let manifest = write_bundle(&root);
        fs::write(root.join("color.png"), b"corrupt").expect("corrupt fixture layer");
        assert!(validate_bundle(&root, &manifest).is_err());

        let root = fixture_root("unsafe");
        let mut manifest = write_bundle(&root);
        manifest.layers[0].path = "../color.png".into();
        assert!(validate_bundle(&root, &manifest).is_err());

        let mut manifest = write_bundle(&root);
        let depth = manifest
            .layers
            .iter_mut()
            .find(|layer| layer.kind == LayerKind::LinearDepth)
            .expect("depth layer");
        let mut bytes = fs::read(root.join("depth.bin")).expect("read depth fixture");
        bytes.pop();
        fs::write(root.join("depth.bin"), &bytes).expect("truncate depth fixture");
        depth.byte_length = u64::try_from(bytes.len()).expect("fixture length fits u64");
        depth.sha256 = sha256_bytes(&bytes);
        assert!(validate_bundle(&root, &manifest).is_err());
        fs::remove_dir_all(root).expect("remove fixture root");
    }
}

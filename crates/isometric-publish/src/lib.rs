//! Bounded deterministic DZI publication.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fmt::Write as _,
    fs, io,
    io::Cursor,
    path::{Path, PathBuf},
};

use image_webp::{ColorType, DecodingError, EncodingError, WebPDecoder, WebPEncoder};
use isometric_render::{
    IndexedImage, RenderError, TileRequest, render_layout, render_tile, required_tile_guard,
};
use isometric_style::{Rgb8, StylePack};
use isometric_world::World;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const RAW_MAGIC: &[u8; 8] = b"ISIDX1\0\0";
const DZI_NAMESPACE: &str = "http://schemas.microsoft.com/deepzoom/2008";
const FORMAT: &str = "webp";
const BASE_NAME: &str = "hero";
const MAX_MANIFEST_BYTES: u64 = 8 * 1_024 * 1_024;
const MAX_DESCRIPTOR_BYTES: u64 = 4 * 1_024;
const MAX_WEBP_TILE_BYTES: u64 = 4 * 1_024 * 1_024;

/// Immutable inputs recorded in the release artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputDigests {
    /// SHA-256 of the canonical world artifact bytes.
    pub world_sha256: String,
    /// SHA-256 of the exact style-pack bytes used for publication.
    pub style_sha256: String,
}

impl InputDigests {
    /// Creates validated hexadecimal digests.
    ///
    /// # Errors
    ///
    /// Returns [`PublishError::InvalidOptions`] unless both values are lowercase
    /// SHA-256 strings.
    pub fn new(world_sha256: String, style_sha256: String) -> Result<Self, PublishError> {
        if !valid_sha256(&world_sha256) || !valid_sha256(&style_sha256) {
            return Err(PublishError::InvalidOptions);
        }
        Ok(Self {
            world_sha256,
            style_sha256,
        })
    }
}

/// DZI publication settings that affect canonical bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DziOptions {
    /// Saved tile edge in pixels.
    pub tile_size: u32,
    /// World millimeters represented by one isometric half-step at maximum level.
    pub world_mm_per_half_step: i64,
}

impl DziOptions {
    /// Accepted prototype settings: 512-pixel tiles and 250 mm logical scale.
    #[must_use]
    pub const fn prototype() -> Self {
        Self {
            tile_size: 512,
            world_mm_per_half_step: 250,
        }
    }
}

/// Summary of one completed publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishReport {
    /// Maximum-resolution artwork width.
    pub width: u32,
    /// Maximum-resolution artwork height.
    pub height: u32,
    /// Highest DZI level.
    pub max_level: u32,
    /// Total WebP tiles across all levels.
    pub tile_count: usize,
    /// Total encoded WebP bytes across all levels.
    pub encoded_bytes: u64,
    /// SHA-256 over the sorted tile path and digest chain.
    pub tile_set_sha256: String,
}

/// Publishes a lossless WebP DZI and canonical indexed level tiles.
///
/// The maximum level is rendered as independently guarded tiles. Every lower
/// level is a top-left nearest-neighbor reduction of the preceding canonical
/// indexed level. Output is staged beside the final directory and renamed only
/// after every tile and manifest succeeds.
///
/// # Errors
///
/// Returns an error for invalid options, existing output, render or encoder
/// failures, malformed staged tiles, I/O errors, or serialization failures.
pub fn publish_dzi(
    world: &World,
    style: &StylePack,
    inputs: &InputDigests,
    output: &Path,
    options: DziOptions,
) -> Result<PublishReport, PublishError> {
    validate_options(options)?;
    if output.exists() {
        return Err(PublishError::OutputExists);
    }
    let staging = staging_path(output)?;
    if staging.exists() {
        return Err(PublishError::OutputExists);
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&staging)?;

    let result = publish_staged(world, style, inputs, &staging, options).and_then(|report| {
        fs::rename(&staging, output)?;
        Ok(report)
    });
    if result.is_err() && staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    result
}

/// Validates every canonical and encoded tile in an existing DZI artifact.
///
/// # Errors
///
/// Returns an error when metadata, completeness, hashes, indexed bytes, WebP
/// losslessness, dimensions, or decoded palette colors do not match.
pub fn validate_dzi(output: &Path, palette: &[Rgb8]) -> Result<PublishReport, PublishError> {
    let manifest_bytes = read_bounded(&output.join("release.json"), MAX_MANIFEST_BYTES)?;
    let manifest: ReleaseArtifact = serde_json::from_slice(&manifest_bytes)?;
    if manifest.schema != "isometric-release/v1"
        || manifest.status != "artifact-candidate"
        || manifest.qualified
        || manifest.dzi.descriptor != "hero.dzi"
        || manifest.dzi.format != FORMAT
        || manifest.dzi.tile_size != 512
        || manifest.dzi.overlap != 0
        || manifest.dzi.world_mm_per_half_step != 250
        || manifest.dzi.canonical_directory != "canonical"
        || manifest.dzi.tile_directory != "hero_files"
        || manifest.dzi.width == 0
        || manifest.dzi.height == 0
        || manifest.dzi.max_level != dzi_max_level(manifest.dzi.width.max(manifest.dzi.height))
        || palette.is_empty()
        || palette.len() > 128
        || !valid_sha256(&manifest.world_sha256)
        || !valid_sha256(&manifest.style_sha256)
        || !valid_sha256(&manifest.dzi.descriptor_sha256)
        || !valid_sha256(&manifest.dzi.tile_set_sha256)
    {
        return Err(PublishError::InvalidManifest);
    }

    let descriptor_bytes = read_bounded(&output.join("hero.dzi"), MAX_DESCRIPTOR_BYTES)?;
    if sha256_hex(&descriptor_bytes) != manifest.dzi.descriptor_sha256
        || descriptor_bytes
            != descriptor(
                manifest.dzi.width,
                manifest.dzi.height,
                manifest.dzi.tile_size,
            )
            .as_bytes()
    {
        return Err(PublishError::InvalidManifest);
    }

    let mut sorted = manifest.tiles.clone();
    sorted.sort_by_key(|entry| (entry.level, entry.row, entry.column));
    if sorted != manifest.tiles
        || sorted.len() != manifest.dzi.tile_count
        || tile_set_hash(&sorted) != manifest.dzi.tile_set_sha256
    {
        return Err(PublishError::InvalidManifest);
    }
    let dimensions = level_dimensions(
        manifest.dzi.width,
        manifest.dzi.height,
        manifest.dzi.max_level,
    )?;
    let expected = expected_tiles(&dimensions, manifest.dzi.tile_size)?;
    let actual = sorted
        .iter()
        .map(|entry| (entry.level, entry.column, entry.row))
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(PublishError::IncompletePyramid);
    }

    let mut encoded_bytes = 0_u64;
    for entry in &sorted {
        validate_tile_entry(output, entry, &dimensions, manifest.dzi.tile_size, palette)?;
        encoded_bytes = encoded_bytes
            .checked_add(entry.encoded_bytes)
            .ok_or(PublishError::CapacityOverflow)?;
    }
    if encoded_bytes != manifest.dzi.encoded_bytes {
        return Err(PublishError::InvalidManifest);
    }

    Ok(PublishReport {
        width: manifest.dzi.width,
        height: manifest.dzi.height,
        max_level: manifest.dzi.max_level,
        tile_count: manifest.dzi.tile_count,
        encoded_bytes,
        tile_set_sha256: manifest.dzi.tile_set_sha256,
    })
}

fn validate_tile_entry(
    output: &Path,
    entry: &TileManifest,
    dimensions: &[(u32, u32)],
    tile_size: u32,
    palette: &[Rgb8],
) -> Result<(), PublishError> {
    let (level_width, level_height) = dimensions
        .get(usize::try_from(entry.level).map_err(|_| PublishError::CapacityOverflow)?)
        .copied()
        .ok_or(PublishError::InvalidManifest)?;
    let expected_width = level_width
        .saturating_sub(entry.column.saturating_mul(tile_size))
        .min(tile_size);
    let expected_height = level_height
        .saturating_sub(entry.row.saturating_mul(tile_size))
        .min(tile_size);
    if entry.width != expected_width
        || entry.height != expected_height
        || !valid_sha256(&entry.canonical_sha256)
        || !valid_sha256(&entry.webp_sha256)
    {
        return Err(PublishError::InvalidManifest);
    }
    let raw_path = raw_tile_path(output, entry.level, entry.column, entry.row);
    let webp_path = webp_tile_path(output, entry.level, entry.column, entry.row);
    let raw_size = 16_u64
        .checked_add(u64::from(entry.width) * u64::from(entry.height))
        .ok_or(PublishError::CapacityOverflow)?;
    let raw_bytes = read_bounded(&raw_path, raw_size)?;
    let webp_bytes = read_bounded(&webp_path, MAX_WEBP_TILE_BYTES)?;
    if sha256_hex(&raw_bytes) != entry.canonical_sha256
        || sha256_hex(&webp_bytes) != entry.webp_sha256
        || u64::try_from(webp_bytes.len()).map_err(|_| PublishError::CapacityOverflow)?
            != entry.encoded_bytes
    {
        return Err(PublishError::HashMismatch);
    }
    let raw = RawTile::read(&raw_path, entry.width, entry.height, palette.len())?;
    validate_webp(&webp_bytes, &raw, palette)
}

fn publish_staged(
    world: &World,
    style: &StylePack,
    inputs: &InputDigests,
    staging: &Path,
    options: DziOptions,
) -> Result<PublishReport, PublishError> {
    let mut render_style = style.clone();
    render_style.world_mm_per_half_step = options.world_mm_per_half_step;
    render_style
        .validate()
        .map_err(|_| PublishError::InvalidOptions)?;
    let layout = render_layout(world, &render_style)?;
    let max_level = dzi_max_level(layout.width().max(layout.height()));
    let dimensions = level_dimensions(layout.width(), layout.height(), max_level)?;
    let guard = required_tile_guard(&render_style)?;
    let mut entries = Vec::new();

    publish_max_level(
        world,
        &render_style,
        layout,
        staging,
        options.tile_size,
        max_level,
        guard,
        &mut entries,
    )?;
    for level in (0..max_level).rev() {
        let source_dimensions = dimensions
            .get(usize::try_from(level + 1).map_err(|_| PublishError::InvalidOptions)?)
            .copied()
            .ok_or(PublishError::InvalidOptions)?;
        let target_dimensions = dimensions
            .get(usize::try_from(level).map_err(|_| PublishError::InvalidOptions)?)
            .copied()
            .ok_or(PublishError::InvalidOptions)?;
        publish_reduced_level(
            staging,
            style,
            options.tile_size,
            level,
            source_dimensions,
            target_dimensions,
            &mut entries,
        )?;
    }

    entries.sort_by_key(|entry| (entry.level, entry.row, entry.column));
    let tile_set_sha256 = tile_set_hash(&entries);
    let encoded_bytes = entries.iter().try_fold(0_u64, |total, entry| {
        total
            .checked_add(entry.encoded_bytes)
            .ok_or(PublishError::CapacityOverflow)
    })?;
    let descriptor = descriptor(layout.width(), layout.height(), options.tile_size);
    let descriptor_sha256 = sha256_hex(descriptor.as_bytes());
    fs::write(staging.join(format!("{BASE_NAME}.dzi")), descriptor)?;
    let manifest = ReleaseManifest {
        schema: "isometric-release/v1",
        status: "artifact-candidate",
        qualified: false,
        world_sha256: &inputs.world_sha256,
        style_sha256: &inputs.style_sha256,
        dzi: DziManifest {
            descriptor: "hero.dzi",
            width: layout.width(),
            height: layout.height(),
            tile_size: options.tile_size,
            overlap: 0,
            format: FORMAT,
            max_level,
            world_mm_per_half_step: options.world_mm_per_half_step,
            tile_count: entries.len(),
            encoded_bytes,
            descriptor_sha256: &descriptor_sha256,
            tile_set_sha256: &tile_set_sha256,
            canonical_directory: "canonical",
            tile_directory: "hero_files",
        },
        tiles: &entries,
    };
    let mut manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    manifest_bytes.push(b'\n');
    fs::write(staging.join("release.json"), manifest_bytes)?;

    Ok(PublishReport {
        width: layout.width(),
        height: layout.height(),
        max_level,
        tile_count: entries.len(),
        encoded_bytes,
        tile_set_sha256,
    })
}

#[allow(clippy::too_many_arguments)]
fn publish_max_level(
    world: &World,
    style: &StylePack,
    layout: isometric_render::RenderLayout,
    staging: &Path,
    tile_size: u32,
    level: u32,
    guard: u32,
    entries: &mut Vec<TileManifest>,
) -> Result<(), PublishError> {
    let columns = layout.width().div_ceil(tile_size);
    let rows = layout.height().div_ceil(tile_size);
    for row in 0..rows {
        for column in 0..columns {
            let rendered = render_tile(
                world,
                style,
                layout,
                TileRequest {
                    column,
                    row,
                    tile_size,
                    guard,
                },
            )?;
            let raw = RawTile::from_image(&rendered.image);
            write_tile(staging, style, level, column, row, &raw, entries)?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn publish_reduced_level(
    staging: &Path,
    style: &StylePack,
    tile_size: u32,
    level: u32,
    source_dimensions: (u32, u32),
    target_dimensions: (u32, u32),
    entries: &mut Vec<TileManifest>,
) -> Result<(), PublishError> {
    let columns = target_dimensions.0.div_ceil(tile_size);
    let rows = target_dimensions.1.div_ceil(tile_size);
    for row in 0..rows {
        for column in 0..columns {
            let width = tile_size.min(target_dimensions.0 - column * tile_size);
            let height = tile_size.min(target_dimensions.1 - row * tile_size);
            let raw = reduce_tile(
                staging,
                tile_size,
                level + 1,
                column,
                row,
                width,
                height,
                source_dimensions,
                style.palette.len(),
            )?;
            write_tile(staging, style, level, column, row, &raw, entries)?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn reduce_tile(
    staging: &Path,
    tile_size: u32,
    source_level: u32,
    column: u32,
    row: u32,
    width: u32,
    height: u32,
    source_dimensions: (u32, u32),
    palette_len: usize,
) -> Result<RawTile, PublishError> {
    let mut source_tiles = BTreeMap::new();
    for source_row in row * 2..=(row * 2 + 1) {
        if source_row * tile_size >= source_dimensions.1 {
            continue;
        }
        for source_column in column * 2..=(column * 2 + 1) {
            if source_column * tile_size >= source_dimensions.0 {
                continue;
            }
            let expected_width = tile_size.min(source_dimensions.0 - source_column * tile_size);
            let expected_height = tile_size.min(source_dimensions.1 - source_row * tile_size);
            let path = raw_tile_path(staging, source_level, source_column, source_row);
            let tile = RawTile::read(&path, expected_width, expected_height, palette_len)?;
            source_tiles.insert((source_column, source_row), tile);
        }
    }

    let mut pixels = Vec::with_capacity(pixel_capacity(width, height)?);
    for local_y in 0..height {
        let source_y = (row * tile_size + local_y) * 2;
        for local_x in 0..width {
            let source_x = (column * tile_size + local_x) * 2;
            let source_column = source_x / tile_size;
            let source_row = source_y / tile_size;
            let source = source_tiles
                .get(&(source_column, source_row))
                .ok_or(PublishError::InvalidCanonicalTile)?;
            pixels.push(source.pixel(source_x % tile_size, source_y % tile_size)?);
        }
    }
    RawTile::new(width, height, pixels, palette_len)
}

fn write_tile(
    staging: &Path,
    style: &StylePack,
    level: u32,
    column: u32,
    row: u32,
    raw: &RawTile,
    entries: &mut Vec<TileManifest>,
) -> Result<(), PublishError> {
    let raw_path = raw_tile_path(staging, level, column, row);
    let webp_path = webp_tile_path(staging, level, column, row);
    create_parent(&raw_path)?;
    create_parent(&webp_path)?;
    let raw_bytes = raw.to_bytes()?;
    let webp_bytes = encode_webp(raw, &style.palette)?;
    fs::write(&raw_path, &raw_bytes)?;
    fs::write(&webp_path, &webp_bytes)?;
    entries.push(TileManifest {
        level,
        column,
        row,
        width: raw.width,
        height: raw.height,
        canonical_sha256: sha256_hex(&raw_bytes),
        webp_sha256: sha256_hex(&webp_bytes),
        encoded_bytes: u64::try_from(webp_bytes.len())
            .map_err(|_| PublishError::CapacityOverflow)?,
    });
    Ok(())
}

fn encode_webp(raw: &RawTile, palette: &[Rgb8]) -> Result<Vec<u8>, PublishError> {
    let rgb_capacity = raw
        .pixels
        .len()
        .checked_mul(3)
        .ok_or(PublishError::CapacityOverflow)?;
    let mut rgb = Vec::with_capacity(rgb_capacity);
    for &index in &raw.pixels {
        let color = palette
            .get(usize::from(index))
            .ok_or(PublishError::InvalidCanonicalTile)?;
        rgb.extend_from_slice(&[color.red, color.green, color.blue]);
    }
    let mut output = Vec::new();
    WebPEncoder::new(&mut output).encode(&rgb, raw.width, raw.height, ColorType::Rgb8)?;
    Ok(output)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RawTile {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl RawTile {
    fn new(
        width: u32,
        height: u32,
        pixels: Vec<u8>,
        palette_len: usize,
    ) -> Result<Self, PublishError> {
        if width == 0
            || height == 0
            || pixels.len() != pixel_capacity(width, height)?
            || pixels
                .iter()
                .any(|index| usize::from(*index) >= palette_len)
        {
            return Err(PublishError::InvalidCanonicalTile);
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    fn from_image(image: &IndexedImage) -> Self {
        Self {
            width: image.width(),
            height: image.height(),
            pixels: image.pixels().to_vec(),
        }
    }

    fn pixel(&self, x: u32, y: u32) -> Result<u8, PublishError> {
        if x >= self.width || y >= self.height {
            return Err(PublishError::InvalidCanonicalTile);
        }
        let index = usize::try_from(y)
            .ok()
            .and_then(|row| row.checked_mul(usize::try_from(self.width).ok()?))
            .and_then(|start| start.checked_add(usize::try_from(x).ok()?))
            .ok_or(PublishError::CapacityOverflow)?;
        self.pixels
            .get(index)
            .copied()
            .ok_or(PublishError::InvalidCanonicalTile)
    }

    fn to_bytes(&self) -> Result<Vec<u8>, PublishError> {
        let mut bytes = Vec::with_capacity(
            RAW_MAGIC
                .len()
                .checked_add(8)
                .and_then(|header| header.checked_add(self.pixels.len()))
                .ok_or(PublishError::CapacityOverflow)?,
        );
        bytes.extend_from_slice(RAW_MAGIC);
        bytes.extend_from_slice(&self.width.to_le_bytes());
        bytes.extend_from_slice(&self.height.to_le_bytes());
        bytes.extend_from_slice(&self.pixels);
        Ok(bytes)
    }

    fn read(
        path: &Path,
        expected_width: u32,
        expected_height: u32,
        palette_len: usize,
    ) -> Result<Self, PublishError> {
        let bytes = fs::read(path)?;
        if bytes.len() < 16 || bytes.get(..8) != Some(RAW_MAGIC) {
            return Err(PublishError::InvalidCanonicalTile);
        }
        let width = u32::from_le_bytes(
            bytes[8..12]
                .try_into()
                .map_err(|_| PublishError::InvalidCanonicalTile)?,
        );
        let height = u32::from_le_bytes(
            bytes[12..16]
                .try_into()
                .map_err(|_| PublishError::InvalidCanonicalTile)?,
        );
        if width != expected_width || height != expected_height {
            return Err(PublishError::InvalidCanonicalTile);
        }
        Self::new(width, height, bytes[16..].to_vec(), palette_len)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TileManifest {
    level: u32,
    column: u32,
    row: u32,
    width: u32,
    height: u32,
    canonical_sha256: String,
    webp_sha256: String,
    encoded_bytes: u64,
}

#[derive(Serialize)]
struct ReleaseManifest<'a> {
    schema: &'static str,
    status: &'static str,
    qualified: bool,
    world_sha256: &'a str,
    style_sha256: &'a str,
    dzi: DziManifest<'a>,
    tiles: &'a [TileManifest],
}

#[derive(Serialize)]
struct DziManifest<'a> {
    descriptor: &'static str,
    width: u32,
    height: u32,
    tile_size: u32,
    overlap: u32,
    format: &'static str,
    max_level: u32,
    world_mm_per_half_step: i64,
    tile_count: usize,
    encoded_bytes: u64,
    descriptor_sha256: &'a str,
    tile_set_sha256: &'a str,
    canonical_directory: &'static str,
    tile_directory: &'static str,
}

#[derive(Deserialize)]
struct ReleaseArtifact {
    schema: String,
    status: String,
    qualified: bool,
    world_sha256: String,
    style_sha256: String,
    dzi: DziArtifact,
    tiles: Vec<TileManifest>,
}

#[derive(Deserialize)]
struct DziArtifact {
    descriptor: String,
    width: u32,
    height: u32,
    tile_size: u32,
    overlap: u32,
    format: String,
    max_level: u32,
    world_mm_per_half_step: i64,
    tile_count: usize,
    encoded_bytes: u64,
    descriptor_sha256: String,
    tile_set_sha256: String,
    canonical_directory: String,
    tile_directory: String,
}

fn validate_options(options: DziOptions) -> Result<(), PublishError> {
    if options.tile_size != 512 || options.world_mm_per_half_step != 250 {
        return Err(PublishError::InvalidOptions);
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn level_dimensions(
    width: u32,
    height: u32,
    max_level: u32,
) -> Result<Vec<(u32, u32)>, PublishError> {
    let count = usize::try_from(max_level)
        .ok()
        .and_then(|level| level.checked_add(1))
        .ok_or(PublishError::CapacityOverflow)?;
    let mut dimensions = vec![(0, 0); count];
    dimensions[usize::try_from(max_level).map_err(|_| PublishError::CapacityOverflow)?] =
        (width, height);
    for level in (0..max_level).rev() {
        let source =
            dimensions[usize::try_from(level + 1).map_err(|_| PublishError::CapacityOverflow)?];
        dimensions[usize::try_from(level).map_err(|_| PublishError::CapacityOverflow)?] =
            (source.0.div_ceil(2), source.1.div_ceil(2));
    }
    Ok(dimensions)
}

fn expected_tiles(
    dimensions: &[(u32, u32)],
    tile_size: u32,
) -> Result<BTreeSet<(u32, u32, u32)>, PublishError> {
    let mut expected = BTreeSet::new();
    for (level, &(width, height)) in dimensions.iter().enumerate() {
        let level = u32::try_from(level).map_err(|_| PublishError::CapacityOverflow)?;
        for row in 0..height.div_ceil(tile_size) {
            for column in 0..width.div_ceil(tile_size) {
                expected.insert((level, column, row));
            }
        }
    }
    Ok(expected)
}

const fn dzi_max_level(max_dimension: u32) -> u32 {
    if max_dimension <= 1 {
        0
    } else {
        u32::BITS - (max_dimension - 1).leading_zeros()
    }
}

fn descriptor(width: u32, height: u32, tile_size: u32) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Image TileSize=\"{tile_size}\" Overlap=\"0\" Format=\"{FORMAT}\" xmlns=\"{DZI_NAMESPACE}\">\n  <Size Width=\"{width}\" Height=\"{height}\"/>\n</Image>\n"
    )
}

fn raw_tile_path(staging: &Path, level: u32, column: u32, row: u32) -> PathBuf {
    staging
        .join("canonical")
        .join(level.to_string())
        .join(format!("{column}_{row}.idx"))
}

fn webp_tile_path(staging: &Path, level: u32, column: u32, row: u32) -> PathBuf {
    staging
        .join(format!("{BASE_NAME}_files"))
        .join(level.to_string())
        .join(format!("{column}_{row}.{FORMAT}"))
}

fn create_parent(path: &Path) -> Result<(), PublishError> {
    let parent = path.parent().ok_or(PublishError::InvalidOutputPath)?;
    fs::create_dir_all(parent)?;
    Ok(())
}

fn read_bounded(path: &Path, maximum_bytes: u64) -> Result<Vec<u8>, PublishError> {
    let length = fs::metadata(path)?.len();
    if length > maximum_bytes {
        return Err(PublishError::ArtifactTooLarge);
    }
    fs::read(path).map_err(PublishError::from)
}

fn validate_webp(webp_bytes: &[u8], raw: &RawTile, palette: &[Rgb8]) -> Result<(), PublishError> {
    let mut decoder = WebPDecoder::new(Cursor::new(webp_bytes))?;
    decoder.set_memory_limit(4 * 1_024 * 1_024);
    if decoder.dimensions() != (raw.width, raw.height) || decoder.is_lossy() {
        return Err(PublishError::InvalidWebP);
    }
    let expected_length = raw
        .pixels
        .len()
        .checked_mul(3)
        .ok_or(PublishError::CapacityOverflow)?;
    if decoder.output_buffer_size() != Some(expected_length) {
        return Err(PublishError::InvalidWebP);
    }
    let mut rgb_output = vec![0; expected_length];
    decoder.read_image(&mut rgb_output)?;
    for (rgb, &index) in rgb_output.chunks_exact(3).zip(&raw.pixels) {
        let color = palette
            .get(usize::from(index))
            .ok_or(PublishError::InvalidCanonicalTile)?;
        if rgb != [color.red, color.green, color.blue] {
            return Err(PublishError::InvalidWebP);
        }
    }
    Ok(())
}

fn staging_path(output: &Path) -> Result<PathBuf, PublishError> {
    let file_name = output
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(PublishError::InvalidOutputPath)?;
    Ok(output.with_file_name(format!("{file_name}.partial")))
}

fn pixel_capacity(width: u32, height: u32) -> Result<usize, PublishError> {
    usize::try_from(width)
        .ok()
        .and_then(|value| value.checked_mul(usize::try_from(height).ok()?))
        .ok_or(PublishError::CapacityOverflow)
}

fn tile_set_hash(entries: &[TileManifest]) -> String {
    let mut hasher = Sha256::new();
    for entry in entries {
        hasher.update(format!(
            "{}/{}_{}.webp",
            entry.level, entry.column, entry.row
        ));
        hasher.update([0]);
        hasher.update(entry.webp_sha256.as_bytes());
        hasher.update([b'\n']);
    }
    digest_hex(hasher.finalize())
}

/// Returns the lowercase SHA-256 for bytes.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    digest_hex(Sha256::digest(bytes))
}

fn digest_hex(digest: impl AsRef<[u8]>) -> String {
    let bytes = digest.as_ref();
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

/// DZI publication failures.
#[derive(Debug)]
pub enum PublishError {
    /// Publication settings are outside the accepted prototype contract.
    InvalidOptions,
    /// The final or staging output already exists.
    OutputExists,
    /// The output path has no safe final component.
    InvalidOutputPath,
    /// Canonical indexed bytes are malformed or outside the palette.
    InvalidCanonicalTile,
    /// Release metadata is malformed or inconsistent.
    InvalidManifest,
    /// One or more expected level tiles are missing or duplicated.
    IncompletePyramid,
    /// Artifact bytes do not match the manifest hash chain.
    HashMismatch,
    /// A WebP tile is lossy, malformed, or differs from canonical indexed colors.
    InvalidWebP,
    /// An artifact exceeds its bounded validation allocation.
    ArtifactTooLarge,
    /// Capacity arithmetic overflowed.
    CapacityOverflow,
    /// Rendering failed.
    Render(RenderError),
    /// WebP encoding failed.
    Encode(EncodingError),
    /// WebP decoding failed during release validation.
    Decode(DecodingError),
    /// Filesystem I/O failed.
    Io(io::Error),
    /// Release-manifest serialization failed.
    Json(serde_json::Error),
}

impl fmt::Display for PublishError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOptions => formatter.write_str("invalid DZI publication options"),
            Self::OutputExists => formatter.write_str("DZI output or staging path already exists"),
            Self::InvalidOutputPath => formatter.write_str("invalid DZI output path"),
            Self::InvalidCanonicalTile => formatter.write_str("invalid canonical indexed tile"),
            Self::InvalidManifest => formatter.write_str("invalid DZI release manifest"),
            Self::IncompletePyramid => formatter.write_str("incomplete DZI tile pyramid"),
            Self::HashMismatch => formatter.write_str("DZI artifact hash mismatch"),
            Self::InvalidWebP => formatter.write_str("invalid or non-lossless WebP tile"),
            Self::ArtifactTooLarge => formatter.write_str("DZI artifact exceeds size limit"),
            Self::CapacityOverflow => formatter.write_str("DZI capacity arithmetic overflow"),
            Self::Render(error) => write!(formatter, "render failed: {error}"),
            Self::Encode(error) => write!(formatter, "WebP encoding failed: {error}"),
            Self::Decode(error) => write!(formatter, "WebP decoding failed: {error}"),
            Self::Io(error) => write!(formatter, "DZI I/O failed: {error}"),
            Self::Json(error) => write!(formatter, "release manifest failed: {error}"),
        }
    }
}

impl std::error::Error for PublishError {}

impl From<RenderError> for PublishError {
    fn from(error: RenderError) -> Self {
        Self::Render(error)
    }
}

impl From<EncodingError> for PublishError {
    fn from(error: EncodingError) -> Self {
        Self::Encode(error)
    }
}

impl From<DecodingError> for PublishError {
    fn from(error: DecodingError) -> Self {
        Self::Decode(error)
    }
}

impl From<io::Error> for PublishError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for PublishError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::Cursor,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static DIRECTORY_ID: AtomicU64 = AtomicU64::new(1);

    fn test_output(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "isometric-publish-{name}-{}-{}",
            std::process::id(),
            DIRECTORY_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn inputs() -> InputDigests {
        InputDigests::new("a".repeat(64), "b".repeat(64)).expect("digests")
    }

    fn remove_test_output(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("remove test output");
        }
    }

    #[test]
    fn webp_encoding_is_byte_deterministic_and_lossless() {
        let style = StylePack::stanford_v1();
        let raw =
            RawTile::new(3, 2, vec![0, 1, 2, 3, 4, 5], style.palette.len()).expect("raw tile");
        let first = encode_webp(&raw, &style.palette).expect("first encode");
        let second = encode_webp(&raw, &style.palette).expect("second encode");
        assert_eq!(first, second);

        let mut decoder = WebPDecoder::new(Cursor::new(first)).expect("decoder");
        assert_eq!(decoder.dimensions(), (3, 2));
        assert!(!decoder.is_lossy());
        let mut rgb_output = vec![0; decoder.output_buffer_size().expect("buffer")];
        decoder.read_image(&mut rgb_output).expect("decode");
        let expected = raw
            .pixels
            .iter()
            .flat_map(|index| {
                let color = &style.palette[usize::from(*index)];
                [color.red, color.green, color.blue]
            })
            .collect::<Vec<_>>();
        assert_eq!(rgb_output, expected);
    }

    #[test]
    fn fixture_publication_is_complete_and_repeatable() {
        let first = test_output("first");
        let second = test_output("second");
        let first_staging = staging_path(&first).expect("staging");
        let world = World::reference_fixture();
        let style = StylePack::stanford_v1();
        let report_a = publish_dzi(&world, &style, &inputs(), &first, DziOptions::prototype())
            .expect("first publish");
        let report_b = publish_dzi(&world, &style, &inputs(), &second, DziOptions::prototype())
            .expect("second publish");
        assert_eq!(report_a, report_b);
        assert_eq!(
            validate_dzi(&first, &style.palette).expect("validate"),
            report_a
        );
        assert!(report_a.tile_count > usize::try_from(report_a.max_level).expect("level"));
        assert_eq!(directory_bytes(&first), directory_bytes(&second));
        assert!(first.join("hero.dzi").is_file());
        assert!(first.join("release.json").is_file());
        assert!(!first_staging.exists());
        remove_test_output(&first);
        remove_test_output(&second);
    }

    #[test]
    fn validation_rejects_corrupted_artifact_bytes() {
        let output = test_output("corrupt");
        let world = World::reference_fixture();
        let style = StylePack::stanford_v1();
        publish_dzi(&world, &style, &inputs(), &output, DziOptions::prototype()).expect("publish");
        let tile = output.join("hero_files/0/0_0.webp");
        let mut bytes = fs::read(&tile).expect("read tile");
        bytes[0] ^= 0xff;
        fs::write(tile, bytes).expect("corrupt tile");
        assert!(matches!(
            validate_dzi(&output, &style.palette),
            Err(PublishError::HashMismatch)
        ));
        remove_test_output(&output);
    }

    #[test]
    fn validation_rejects_manifest_dimension_tampering() {
        let output = test_output("tampered-dimension");
        let world = World::reference_fixture();
        let style = StylePack::stanford_v1();
        publish_dzi(&world, &style, &inputs(), &output, DziOptions::prototype()).expect("publish");
        let manifest_path = output.join("release.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).expect("manifest"))
                .expect("manifest JSON");
        manifest["tiles"][0]["width"] = serde_json::json!(511);
        fs::write(
            manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("manifest bytes"),
        )
        .expect("tamper manifest");
        assert!(matches!(
            validate_dzi(&output, &style.palette),
            Err(PublishError::InvalidManifest)
        ));
        remove_test_output(&output);
    }

    #[test]
    fn publication_fails_closed_for_existing_output_and_invalid_options() {
        let output = test_output("existing");
        fs::create_dir(&output).expect("create existing");
        let world = World::reference_fixture();
        let style = StylePack::stanford_v1();
        assert!(matches!(
            publish_dzi(&world, &style, &inputs(), &output, DziOptions::prototype()),
            Err(PublishError::OutputExists)
        ));
        let invalid_output = test_output("invalid");
        assert!(matches!(
            publish_dzi(
                &world,
                &style,
                &inputs(),
                &invalid_output,
                DziOptions {
                    tile_size: 256,
                    world_mm_per_half_step: 250,
                },
            ),
            Err(PublishError::InvalidOptions)
        ));
        assert!(!invalid_output.exists());
        remove_test_output(&output);
    }

    #[test]
    fn level_dimensions_preserve_odd_edges() {
        assert_eq!(
            level_dimensions(5, 3, 3).expect("levels"),
            vec![(1, 1), (2, 1), (3, 2), (5, 3)]
        );
        assert_eq!(dzi_max_level(1), 0);
        assert_eq!(dzi_max_level(8), 3);
        assert_eq!(dzi_max_level(9), 4);
    }

    fn directory_bytes(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut output = Vec::new();
        collect_directory(root, root, &mut output);
        output.sort_by(|left, right| left.0.cmp(&right.0));
        output
    }

    fn collect_directory(root: &Path, directory: &Path, output: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries = fs::read_dir(directory)
            .expect("read directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("directory entries");
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                collect_directory(root, &path, output);
            } else {
                output.push((
                    path.strip_prefix(root).expect("relative").to_path_buf(),
                    fs::read(path).expect("read file"),
                ));
            }
        }
    }
}

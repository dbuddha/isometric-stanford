//! Canonical tiled atlas compilation from frozen registered Google bundles.
//!
//! Raw provider captures are diagnostic inputs. This module establishes the
//! exact downstream boundary by assigning every saved atlas pixel to exactly
//! one validated source bundle and materializing canonical registered tiles.

use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    CameraSpec, LayerKind, ReferenceError, ReferenceManifest, encode_raw_png, read_manifest,
    validate_bundle,
};

/// Portable request schema for atlas compilation.
pub const ATLAS_REQUEST_SCHEMA: &str = "isometric-reference-atlas-request/v1";
/// Portable immutable atlas-manifest schema.
pub const ATLAS_MANIFEST_SCHEMA: &str = "isometric-reference-atlas-manifest/v1";
/// Canonical manifest filename inside an atlas directory.
pub const ATLAS_MANIFEST_FILENAME: &str = "reference-atlas.manifest.json";

const OWNERSHIP_MAGIC: &[u8; 8] = b"ISOOWNV1";
const DEPTH_MAGIC: &[u8; 8] = b"ISOD32V1";
const MAX_REQUEST_BYTES: u64 = 1024 * 1024;
const MAX_ATLAS_CELLS: usize = 4_096;
const MAX_SESSION_TEXT: usize = 256;

/// Non-secret identity for one provider root session.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionRecord {
    /// Stable, non-secret session identifier.
    pub session_id: String,
    /// Hash of the exact root tileset response used by the renderer.
    pub root_tileset_sha256: String,
    /// RFC 3339 acquisition start time.
    pub started_at: String,
    /// RFC 3339 session expiry time.
    pub expires_at: String,
}

/// User request for one canonical atlas compilation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AtlasCompileRequest {
    /// Request schema identity.
    pub schema: String,
    /// Stable atlas identifier.
    pub atlas_id: String,
    /// Provider session shared by the frozen bundles.
    pub source_session: SessionRecord,
    /// Reference-bundle directories, relative to the request file or absolute.
    pub bundle_directories: Vec<String>,
}

/// Rectangular world-grid contract for an atlas.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AtlasGrid {
    /// Smallest source supertile column.
    pub minimum_column: i32,
    /// Smallest source supertile row.
    pub minimum_row: i32,
    /// Number of saved core columns.
    pub columns: u16,
    /// Number of saved core rows.
    pub rows: u16,
    /// Saved core width per atlas tile.
    pub core_width_px: u16,
    /// Saved core height per atlas tile.
    pub core_height_px: u16,
    /// Source context outside every core edge.
    pub source_guard_px: u16,
    /// Source ground sampling distance.
    pub millimeters_per_pixel: u32,
    /// Complete atlas width without guards.
    pub width_px: u32,
    /// Complete atlas height without guards.
    pub height_px: u32,
    /// Integer-grid registration error. Canonical compilation requires zero.
    pub registration_error_micropixels: u32,
}

/// One validated input bundle in canonical source order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AtlasSourceRecord {
    /// Dense zero-based ownership identifier.
    pub source_index: u16,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Bundle manifest digest.
    pub manifest_sha256: String,
    /// World-grid column.
    pub column: i32,
    /// World-grid row.
    pub row: i32,
    /// Core valid-pixel coverage recorded by capture.
    pub core_coverage_basis_points: u16,
}

/// One canonical layer tile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AtlasLayerTileRecord {
    /// World-grid column.
    pub column: i32,
    /// World-grid row.
    pub row: i32,
    /// Registered layer identity.
    pub kind: LayerKind,
    /// Safe atlas-relative path.
    pub path: String,
    /// Portable encoding identifier.
    pub encoding: String,
    /// Saved tile width.
    pub width_px: u32,
    /// Saved tile height.
    pub height_px: u32,
    /// Exact file length.
    pub byte_length: u64,
    /// Exact file digest.
    pub sha256: String,
}

/// One dense per-pixel source-ownership tile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OwnershipTileRecord {
    /// World-grid column.
    pub column: i32,
    /// World-grid row.
    pub row: i32,
    /// Safe atlas-relative path.
    pub path: String,
    /// Saved tile width.
    pub width_px: u32,
    /// Saved tile height.
    pub height_px: u32,
    /// Exact file length.
    pub byte_length: u64,
    /// Exact file digest.
    pub sha256: String,
}

/// Immutable canonical `ReferenceAtlas` contract.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReferenceAtlasManifest {
    /// Atlas schema identity.
    pub schema: String,
    /// Stable atlas identity.
    pub atlas_id: String,
    /// Fixed canonical grid.
    pub grid: AtlasGrid,
    /// Shared orthographic camera.
    pub camera: CameraSpec,
    /// Sole geographic provider.
    pub provider: String,
    /// Pinned capture renderer identity.
    pub renderer: String,
    /// Pinned capture renderer version.
    pub renderer_version: String,
    /// Provider epoch shared by all inputs.
    pub source_epoch: String,
    /// Provider session identity.
    pub source_session: SessionRecord,
    /// Stable aggregated provider attributions.
    pub attributions: Vec<String>,
    /// Validated sources in ownership-index order.
    pub sources: Vec<AtlasSourceRecord>,
    /// Canonical registered layer tiles.
    pub layer_tiles: Vec<AtlasLayerTileRecord>,
    /// Canonical dense source-ownership tiles.
    pub ownership_tiles: Vec<OwnershipTileRecord>,
    /// Digest over ordered ownership tile identities and hashes.
    pub ownership_map_sha256: String,
}

/// Reproducible evidence returned by atlas compilation or inspection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasReport {
    /// Canonical manifest digest.
    pub manifest_sha256: String,
    /// Number of canonical saved tiles.
    pub tile_count: usize,
    /// Total exact atlas artifact bytes.
    pub total_bytes: u64,
    /// Conservative maximum row-buffer bytes used by compilation.
    pub peak_row_buffer_bytes: u64,
}

struct SourceBundle {
    root: PathBuf,
    manifest: ReferenceManifest,
    manifest_sha256: String,
}

struct DecodedSource {
    layers: BTreeMap<LayerKind, DecodedLayer>,
}

struct DecodedLayer {
    file: RefCell<File>,
    width: u32,
    height: u32,
    bytes_per_pixel: u8,
    data_offset: u64,
}

impl DecodedLayer {
    fn read_row(&self, row: u32) -> Result<Vec<u8>, ReferenceError> {
        if row >= self.height {
            return Err(ReferenceError::Invalid(
                "atlas source row exceeds decoded layer".into(),
            ));
        }
        let row_bytes = u64::from(self.width)
            .checked_mul(u64::from(self.bytes_per_pixel))
            .ok_or_else(|| ReferenceError::Invalid("atlas source row overflowed".into()))?;
        let offset = self
            .data_offset
            .checked_add(
                u64::from(row)
                    .checked_mul(row_bytes)
                    .ok_or_else(|| ReferenceError::Invalid("atlas row offset overflowed".into()))?,
            )
            .ok_or_else(|| ReferenceError::Invalid("atlas row offset overflowed".into()))?;
        let length = usize::try_from(row_bytes)
            .map_err(|_| ReferenceError::Invalid("atlas row does not fit memory".into()))?;
        let mut file = self.file.borrow_mut();
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![0_u8; length];
        file.read_exact(&mut bytes)?;
        Ok(bytes)
    }
}

struct PartialDirectory {
    path: PathBuf,
    keep: bool,
}

impl Drop for PartialDirectory {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OwnershipScore {
    edge_distance: u32,
    structural_stability: u8,
    sampling_density: u32,
    source_index: u16,
}

impl Ord for OwnershipScore {
    fn cmp(&self, other: &Self) -> Ordering {
        self.edge_distance
            .cmp(&other.edge_distance)
            .then(self.structural_stability.cmp(&other.structural_stability))
            .then(other.sampling_density.cmp(&self.sampling_density))
            .then(other.source_index.cmp(&self.source_index))
    }
}

impl PartialOrd for OwnershipScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Compile a request file into a new atomic `ReferenceAtlas` directory.
///
/// # Errors
///
/// Returns an error when the request, any source bundle, the grid, ownership,
/// output path, or an artifact violates the canonical atlas contract.
pub fn compile_atlas_file(
    request_path: &Path,
    output_root: &Path,
) -> Result<AtlasReport, ReferenceError> {
    let metadata = request_path.symlink_metadata()?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_REQUEST_BYTES
    {
        return Err(ReferenceError::Invalid(
            "atlas request is not a bounded regular file".into(),
        ));
    }
    let request: AtlasCompileRequest =
        serde_json::from_reader(BufReader::new(File::open(request_path)?))?;
    let request_root = request_path.parent().unwrap_or_else(|| Path::new("."));
    compile_atlas(request_root, &request, output_root)
}

/// Compile one validated request into a new atomic `ReferenceAtlas` directory.
///
/// # Errors
///
/// Returns an error when any source or output violates the atlas contract.
#[expect(
    clippy::too_many_lines,
    reason = "the atomic atlas transaction is kept visible as one ordered operation"
)]
pub fn compile_atlas(
    request_root: &Path,
    request: &AtlasCompileRequest,
    output_root: &Path,
) -> Result<AtlasReport, ReferenceError> {
    validate_request(request)?;
    if output_root.exists() {
        return Err(ReferenceError::Invalid(
            "atlas output path already exists".into(),
        ));
    }
    let mut sources = load_sources(request_root, request)?;
    validate_source_set(&sources)?;
    sources.sort_by(|left, right| source_sort_key(left).cmp(&source_sort_key(right)));
    let grid = atlas_grid(&sources)?;

    let parent = output_root.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let file_name = output_root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ReferenceError::Invalid("atlas output path has no safe filename".into()))?;
    let staging = parent.join(format!(".{file_name}.stage-{}", std::process::id()));
    if staging.exists() {
        return Err(ReferenceError::Invalid(
            "atlas staging path already exists".into(),
        ));
    }
    fs::create_dir(&staging)?;
    let mut partial = PartialDirectory {
        path: staging.clone(),
        keep: false,
    };
    let decoded_root = staging.join(".decoded");
    let scratch_root = staging.join(".scratch");
    fs::create_dir(&decoded_root)?;
    fs::create_dir(&scratch_root)?;
    let decoded = decode_sources(&sources, &decoded_root)?;

    let mut layer_tiles = Vec::new();
    let mut ownership_tiles = Vec::new();
    let mut peak_row_buffer_bytes = 0_u64;
    for row_offset in 0..grid.rows {
        for column_offset in 0..grid.columns {
            let column = grid.minimum_column + i32::from(column_offset);
            let row = grid.minimum_row + i32::from(row_offset);
            let tile_directory = tile_directory(column, row);
            fs::create_dir_all(staging.join(&tile_directory))?;
            let (ownership, row_bytes) =
                build_ownership_tile(&sources, &decoded, &grid, column, row)?;
            peak_row_buffer_bytes = peak_row_buffer_bytes.max(row_bytes);
            ownership_tiles.push(write_ownership_tile(
                &staging,
                &tile_directory,
                column,
                row,
                &grid,
                &ownership,
            )?);
            for kind in required_layer_kinds() {
                let (record, layer_row_bytes) = write_layer_tile(
                    &staging,
                    &scratch_root,
                    &tile_directory,
                    &sources,
                    &decoded,
                    &grid,
                    column,
                    row,
                    kind,
                    &ownership,
                )?;
                peak_row_buffer_bytes = peak_row_buffer_bytes.max(layer_row_bytes);
                layer_tiles.push(record);
            }
        }
    }

    drop(decoded);
    fs::remove_dir_all(&decoded_root)?;
    fs::remove_dir_all(&scratch_root)?;
    validate_sources_unchanged(&sources)?;
    let first = &sources[0].manifest;
    let mut attributions = sources
        .iter()
        .flat_map(|source| source.manifest.capture.attributions.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    attributions.sort();
    let source_records = sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            Ok(AtlasSourceRecord {
                source_index: u16::try_from(index).map_err(|_| {
                    ReferenceError::Invalid("atlas source index exceeds u16".into())
                })?,
                bundle_id: source.manifest.bundle_id.clone(),
                manifest_sha256: source.manifest_sha256.clone(),
                column: source.manifest.tile.column,
                row: source.manifest.tile.row,
                core_coverage_basis_points: source.manifest.core_coverage_basis_points,
            })
        })
        .collect::<Result<Vec<_>, ReferenceError>>()?;
    let ownership_map_sha256 = ownership_digest(&ownership_tiles);
    let manifest = ReferenceAtlasManifest {
        schema: ATLAS_MANIFEST_SCHEMA.into(),
        atlas_id: request.atlas_id.clone(),
        grid,
        camera: first.camera.clone(),
        provider: first.capture.provider.clone(),
        renderer: first.capture.renderer.clone(),
        renderer_version: first.capture.renderer_version.clone(),
        source_epoch: first.capture.source_epoch.clone(),
        source_session: request.source_session.clone(),
        attributions,
        sources: source_records,
        layer_tiles,
        ownership_tiles,
        ownership_map_sha256,
    };
    let manifest_json = canonical_atlas_manifest_json(&manifest)?;
    write_new_file(
        &staging.join(ATLAS_MANIFEST_FILENAME),
        manifest_json.as_bytes(),
    )?;
    let mut report = validate_atlas(&staging, &manifest)?;
    report.peak_row_buffer_bytes = peak_row_buffer_bytes;
    fs::rename(&staging, output_root)?;
    partial.keep = true;
    Ok(report)
}

/// Read an atlas manifest without trusting any referenced artifact.
///
/// # Errors
///
/// Returns an error when the file cannot be read or decoded.
pub fn read_atlas_manifest(path: &Path) -> Result<ReferenceAtlasManifest, ReferenceError> {
    Ok(serde_json::from_reader(BufReader::new(File::open(path)?))?)
}

/// Serialize a validated atlas manifest into canonical JSON.
///
/// # Errors
///
/// Returns an error when the manifest is invalid or cannot be encoded.
pub fn canonical_atlas_manifest_json(
    manifest: &ReferenceAtlasManifest,
) -> Result<String, ReferenceError> {
    validate_atlas_manifest(manifest)?;
    let mut encoded = serde_json::to_string_pretty(manifest)?;
    encoded.push('\n');
    Ok(encoded)
}

/// Validate one complete canonical `ReferenceAtlas` and its hash chain.
///
/// # Errors
///
/// Returns an error for invalid schema, grid, source ownership, paths, headers,
/// lengths, hashes, missing tiles, duplicate tiles, or unsafe artifacts.
pub fn validate_atlas(
    root: &Path,
    manifest: &ReferenceAtlasManifest,
) -> Result<AtlasReport, ReferenceError> {
    validate_atlas_manifest(manifest)?;
    let mut total_bytes = 0_u64;
    for tile in &manifest.layer_tiles {
        let path = root.join(&tile.path);
        validate_regular_hashed_file(&path, tile.byte_length, &tile.sha256)?;
        let (width, height) = match tile.kind {
            LayerKind::LinearDepth => read_depth_dimensions(&path)?,
            _ => read_png_dimensions(&path, tile.kind)?,
        };
        if width != tile.width_px || height != tile.height_px {
            return Err(ReferenceError::Invalid(
                "atlas layer tile header contradicts its manifest".into(),
            ));
        }
        total_bytes = total_bytes
            .checked_add(tile.byte_length)
            .ok_or_else(|| ReferenceError::Invalid("atlas byte total overflowed".into()))?;
    }
    for tile in &manifest.ownership_tiles {
        let path = root.join(&tile.path);
        validate_regular_hashed_file(&path, tile.byte_length, &tile.sha256)?;
        validate_ownership_file(&path, tile, manifest)?;
        total_bytes = total_bytes
            .checked_add(tile.byte_length)
            .ok_or_else(|| ReferenceError::Invalid("atlas byte total overflowed".into()))?;
    }
    let manifest_json = canonical_atlas_manifest_json(manifest)?;
    Ok(AtlasReport {
        manifest_sha256: sha256_bytes(manifest_json.as_bytes()),
        tile_count: manifest.ownership_tiles.len(),
        total_bytes,
        peak_row_buffer_bytes: 0,
    })
}

fn validate_request(request: &AtlasCompileRequest) -> Result<(), ReferenceError> {
    if request.schema != ATLAS_REQUEST_SCHEMA
        || !is_identifier(&request.atlas_id)
        || request.bundle_directories.is_empty()
        || request.bundle_directories.len() > MAX_ATLAS_CELLS
        || request
            .bundle_directories
            .iter()
            .any(|path| path.is_empty() || path.len() > 4_096 || path.chars().any(char::is_control))
    {
        return Err(ReferenceError::Invalid(
            "atlas request violates its bounded contract".into(),
        ));
    }
    validate_session(&request.source_session)
}

fn validate_session(session: &SessionRecord) -> Result<(), ReferenceError> {
    if !is_identifier(&session.session_id)
        || !is_sha256(&session.root_tileset_sha256)
        || session.started_at.is_empty()
        || session.expires_at.is_empty()
        || session.started_at.len() > MAX_SESSION_TEXT
        || session.expires_at.len() > MAX_SESSION_TEXT
        || session.started_at.chars().any(char::is_control)
        || session.expires_at.chars().any(char::is_control)
    {
        return Err(ReferenceError::Invalid(
            "atlas source session contract is invalid".into(),
        ));
    }
    Ok(())
}

fn load_sources(
    request_root: &Path,
    request: &AtlasCompileRequest,
) -> Result<Vec<SourceBundle>, ReferenceError> {
    request
        .bundle_directories
        .iter()
        .map(|directory| {
            let requested = Path::new(directory);
            let root = if requested.is_absolute() {
                requested.to_path_buf()
            } else {
                request_root.join(requested)
            };
            let metadata = root.symlink_metadata()?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(ReferenceError::Invalid(
                    "atlas input bundle is not a regular directory".into(),
                ));
            }
            let manifest = read_manifest(&root.join(crate::MANIFEST_FILENAME))?;
            let report = validate_bundle(&root, &manifest)?;
            Ok(SourceBundle {
                root,
                manifest,
                manifest_sha256: report.manifest_sha256,
            })
        })
        .collect()
}

fn validate_source_set(sources: &[SourceBundle]) -> Result<(), ReferenceError> {
    if sources.is_empty()
        || sources.len() > MAX_ATLAS_CELLS
        || sources.len() > usize::from(u16::MAX)
    {
        return Err(ReferenceError::Invalid(
            "atlas source count violates its bound".into(),
        ));
    }
    let first = &sources[0].manifest;
    let mut cells = BTreeSet::new();
    for source in sources {
        let manifest = &source.manifest;
        if manifest.camera != first.camera
            || manifest.lighting != first.lighting
            || manifest.capture.provider != "google-photorealistic-3d-tiles"
            || manifest.capture.provider != first.capture.provider
            || manifest.capture.renderer != first.capture.renderer
            || manifest.capture.renderer_version != first.capture.renderer_version
            || manifest.capture.source_epoch != first.capture.source_epoch
            || manifest.tile.region_id != first.tile.region_id
            || manifest.tile.core_width_px != first.tile.core_width_px
            || manifest.tile.core_height_px != first.tile.core_height_px
            || manifest.tile.guard_px != first.tile.guard_px
            || manifest.tile.millimeters_per_pixel != first.tile.millimeters_per_pixel
        {
            return Err(ReferenceError::Invalid(
                "atlas sources do not share one Google grid and camera contract".into(),
            ));
        }
        if !cells.insert((manifest.tile.column, manifest.tile.row)) {
            return Err(ReferenceError::Invalid(
                "atlas source grid contains a duplicate cell".into(),
            ));
        }
    }
    let minimum_column = cells
        .iter()
        .map(|(column, _)| *column)
        .min()
        .expect("not empty");
    let maximum_column = cells
        .iter()
        .map(|(column, _)| *column)
        .max()
        .expect("not empty");
    let minimum_row = cells.iter().map(|(_, row)| *row).min().expect("not empty");
    let maximum_row = cells.iter().map(|(_, row)| *row).max().expect("not empty");
    for row in minimum_row..=maximum_row {
        for column in minimum_column..=maximum_column {
            if !cells.contains(&(column, row)) {
                return Err(ReferenceError::Invalid(
                    "atlas source grid contains a missing cell".into(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_sources_unchanged(sources: &[SourceBundle]) -> Result<(), ReferenceError> {
    for source in sources {
        let manifest = read_manifest(&source.root.join(crate::MANIFEST_FILENAME))?;
        let report = validate_bundle(&source.root, &manifest)?;
        if manifest != source.manifest || report.manifest_sha256 != source.manifest_sha256 {
            return Err(ReferenceError::Invalid(
                "atlas source changed while it was being compiled".into(),
            ));
        }
    }
    Ok(())
}

fn source_sort_key(source: &SourceBundle) -> (i32, i32, &str, &str) {
    (
        source.manifest.tile.row,
        source.manifest.tile.column,
        &source.manifest.bundle_id,
        &source.manifest_sha256,
    )
}

fn atlas_grid(sources: &[SourceBundle]) -> Result<AtlasGrid, ReferenceError> {
    let first = &sources[0].manifest.tile;
    let minimum_column = sources
        .iter()
        .map(|source| source.manifest.tile.column)
        .min()
        .expect("validated nonempty sources");
    let maximum_column = sources
        .iter()
        .map(|source| source.manifest.tile.column)
        .max()
        .expect("validated nonempty sources");
    let minimum_row = sources
        .iter()
        .map(|source| source.manifest.tile.row)
        .min()
        .expect("validated nonempty sources");
    let maximum_row = sources
        .iter()
        .map(|source| source.manifest.tile.row)
        .max()
        .expect("validated nonempty sources");
    let columns = u16::try_from(maximum_column - minimum_column + 1)
        .map_err(|_| ReferenceError::Invalid("atlas column count overflowed".into()))?;
    let rows = u16::try_from(maximum_row - minimum_row + 1)
        .map_err(|_| ReferenceError::Invalid("atlas row count overflowed".into()))?;
    let width_px = u32::from(columns)
        .checked_mul(u32::from(first.core_width_px))
        .ok_or_else(|| ReferenceError::Invalid("atlas width overflowed".into()))?;
    let height_px = u32::from(rows)
        .checked_mul(u32::from(first.core_height_px))
        .ok_or_else(|| ReferenceError::Invalid("atlas height overflowed".into()))?;
    Ok(AtlasGrid {
        minimum_column,
        minimum_row,
        columns,
        rows,
        core_width_px: first.core_width_px,
        core_height_px: first.core_height_px,
        source_guard_px: first.guard_px,
        millimeters_per_pixel: first.millimeters_per_pixel,
        width_px,
        height_px,
        registration_error_micropixels: 0,
    })
}

fn decode_sources(
    sources: &[SourceBundle],
    decoded_root: &Path,
) -> Result<Vec<DecodedSource>, ReferenceError> {
    sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            let root = decoded_root.join(format!("source-{index:04}"));
            fs::create_dir(&root)?;
            let mut layers = BTreeMap::new();
            for kind in required_layer_kinds() {
                let input = source.root.join(kind.filename());
                if kind == LayerKind::LinearDepth {
                    layers.insert(
                        kind,
                        DecodedLayer {
                            file: RefCell::new(File::open(input)?),
                            width: source.manifest.tile.total_width_px(),
                            height: source.manifest.tile.total_height_px(),
                            bytes_per_pixel: 4,
                            data_offset: 16,
                        },
                    );
                    continue;
                }
                let output = root.join(format!("{}.raw", kind.filename()));
                let (width, height, bytes_per_pixel) = decode_png_to_raw(&input, &output, kind)?;
                layers.insert(
                    kind,
                    DecodedLayer {
                        file: RefCell::new(File::open(output)?),
                        width,
                        height,
                        bytes_per_pixel,
                        data_offset: 0,
                    },
                );
            }
            Ok(DecodedSource { layers })
        })
        .collect()
}

fn decode_png_to_raw(
    input: &Path,
    output: &Path,
    kind: LayerKind,
) -> Result<(u32, u32, u8), ReferenceError> {
    let mut decoder = png::Decoder::new(BufReader::new(File::open(input)?));
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder
        .read_info()
        .map_err(|error| ReferenceError::Invalid(format!("atlas PNG decode failed: {error}")))?;
    let info = reader.info();
    let expected_color = match kind {
        LayerKind::Color | LayerKind::Whitebox | LayerKind::ViewNormal => png::ColorType::Rgba,
        LayerKind::FixedShadow | LayerKind::Coverage => png::ColorType::Grayscale,
        LayerKind::LinearDepth => {
            return Err(ReferenceError::Invalid(
                "depth cannot enter the PNG decoder".into(),
            ));
        }
    };
    if info.bit_depth != png::BitDepth::Eight
        || info.color_type != expected_color
        || info.interlaced
    {
        return Err(ReferenceError::Invalid(
            "atlas PNG color or interlace contract changed during decoding".into(),
        ));
    }
    let bytes_per_pixel = if expected_color == png::ColorType::Rgba {
        4
    } else {
        1
    };
    let width = info.width;
    let height = info.height;
    let expected_row_bytes = usize::try_from(u64::from(width) * u64::from(bytes_per_pixel))
        .map_err(|_| ReferenceError::Invalid("decoded PNG row does not fit memory".into()))?;
    let output_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output)?;
    let mut writer = BufWriter::new(output_file);
    let mut rows = 0_u32;
    while let Some(row) = reader
        .next_row()
        .map_err(|error| ReferenceError::Invalid(format!("atlas PNG row failed: {error}")))?
    {
        if row.data().len() != expected_row_bytes {
            return Err(ReferenceError::Invalid(
                "decoded PNG row violates the registered width".into(),
            ));
        }
        writer.write_all(row.data())?;
        rows = rows
            .checked_add(1)
            .ok_or_else(|| ReferenceError::Invalid("decoded PNG row count overflowed".into()))?;
    }
    if rows != height {
        return Err(ReferenceError::Invalid(
            "decoded PNG row count violates the registered height".into(),
        ));
    }
    writer.flush()?;
    writer.get_ref().sync_all()?;
    Ok((width, height, bytes_per_pixel))
}

fn build_ownership_tile(
    sources: &[SourceBundle],
    decoded: &[DecodedSource],
    grid: &AtlasGrid,
    column: i32,
    row: i32,
) -> Result<(Vec<u16>, u64), ReferenceError> {
    let width = u32::from(grid.core_width_px);
    let height = u32::from(grid.core_height_px);
    let pixel_count = usize::try_from(u64::from(width) * u64::from(height))
        .map_err(|_| ReferenceError::Invalid("ownership tile does not fit memory".into()))?;
    let mut ownership = vec![u16::MAX; pixel_count];
    let tile_origin_x = i64::from(column - grid.minimum_column) * i64::from(width);
    let tile_origin_y = i64::from(row - grid.minimum_row) * i64::from(height);
    let mut peak_row_bytes = 0_u64;

    for output_y in 0..height {
        let global_y = tile_origin_y + i64::from(output_y);
        let mut rows = Vec::with_capacity(sources.len());
        let mut current_row_bytes = 0_u64;
        for (index, source) in sources.iter().enumerate() {
            let source_y = source_local_y(source, grid, global_y);
            if let Some(local_y) = source_y {
                let coverage = decoded[index].layers[&LayerKind::Coverage].read_row(local_y)?;
                let depth = decoded[index].layers[&LayerKind::LinearDepth].read_row(local_y)?;
                let normal = decoded[index].layers[&LayerKind::ViewNormal].read_row(local_y)?;
                current_row_bytes = current_row_bytes
                    .checked_add(
                        u64::try_from(coverage.len() + depth.len() + normal.len()).map_err(
                            |_| ReferenceError::Invalid("atlas row memory overflowed".into()),
                        )?,
                    )
                    .ok_or_else(|| ReferenceError::Invalid("atlas row memory overflowed".into()))?;
                rows.push((index, local_y, coverage, depth, normal));
            }
        }
        peak_row_bytes = peak_row_bytes.max(current_row_bytes);
        for output_x in 0..width {
            let global_x = tile_origin_x + i64::from(output_x);
            let mut best: Option<(OwnershipScore, u16)> = None;
            for (index, local_y, coverage, depth, normal) in &rows {
                let Some(local_x) = source_local_x(&sources[*index], grid, global_x) else {
                    continue;
                };
                let x = usize::try_from(local_x).map_err(|_| {
                    ReferenceError::Invalid("ownership x does not fit memory".into())
                })?;
                if coverage[x] == 0 {
                    continue;
                }
                let depth_offset = x.checked_mul(4).ok_or_else(|| {
                    ReferenceError::Invalid("ownership depth offset overflowed".into())
                })?;
                let normal_offset = depth_offset;
                let depth_value = u32::from_le_bytes(
                    depth[depth_offset..depth_offset + 4]
                        .try_into()
                        .expect("validated depth row slice"),
                );
                let structural_stability =
                    u8::from(depth_value != 0) + u8::from(normal[normal_offset + 3] != 0);
                let source_width = sources[*index].manifest.tile.total_width_px();
                let source_height = sources[*index].manifest.tile.total_height_px();
                let edge_distance = local_x
                    .min(source_width - 1 - local_x)
                    .min(*local_y)
                    .min(source_height - 1 - *local_y);
                let source_index = u16::try_from(*index)
                    .map_err(|_| ReferenceError::Invalid("source index exceeds u16".into()))?;
                let score = OwnershipScore {
                    edge_distance,
                    structural_stability,
                    sampling_density: sources[*index].manifest.tile.millimeters_per_pixel,
                    source_index,
                };
                if best.is_none_or(|(current, _)| score > current) {
                    best = Some((score, source_index));
                }
            }
            let owner = best.map(|(_, owner)| owner).ok_or_else(|| {
                ReferenceError::Invalid("atlas ownership contains a coverage gap".into())
            })?;
            let offset =
                usize::try_from(u64::from(output_y) * u64::from(width) + u64::from(output_x))
                    .map_err(|_| {
                        ReferenceError::Invalid("ownership offset does not fit memory".into())
                    })?;
            ownership[offset] = owner;
        }
    }
    Ok((ownership, peak_row_bytes))
}

fn write_ownership_tile(
    staging: &Path,
    tile_directory: &str,
    column: i32,
    row: i32,
    grid: &AtlasGrid,
    ownership: &[u16],
) -> Result<OwnershipTileRecord, ReferenceError> {
    let relative = format!("{tile_directory}/ownership.bin");
    let path = staging.join(&relative);
    let output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)?;
    let mut writer = BufWriter::new(output);
    writer.write_all(OWNERSHIP_MAGIC)?;
    writer.write_all(&u32::from(grid.core_width_px).to_le_bytes())?;
    writer.write_all(&u32::from(grid.core_height_px).to_le_bytes())?;
    for owner in ownership {
        writer.write_all(&owner.to_le_bytes())?;
    }
    writer.flush()?;
    writer.get_ref().sync_all()?;
    drop(writer);
    let byte_length = path.metadata()?.len();
    Ok(OwnershipTileRecord {
        column,
        row,
        path: relative,
        width_px: u32::from(grid.core_width_px),
        height_px: u32::from(grid.core_height_px),
        byte_length,
        sha256: sha256_file(&path)?,
    })
}

#[allow(clippy::too_many_arguments)]
#[expect(
    clippy::too_many_lines,
    reason = "the row-streaming layer writer keeps allocation and accounting adjacent"
)]
fn write_layer_tile(
    staging: &Path,
    scratch_root: &Path,
    tile_directory: &str,
    sources: &[SourceBundle],
    decoded: &[DecodedSource],
    grid: &AtlasGrid,
    column: i32,
    row: i32,
    kind: LayerKind,
    ownership: &[u16],
) -> Result<(AtlasLayerTileRecord, u64), ReferenceError> {
    let width = u32::from(grid.core_width_px);
    let height = u32::from(grid.core_height_px);
    let bytes_per_pixel = decoded[0].layers[&kind].bytes_per_pixel;
    let raw_path = scratch_root.join(format!(
        "{}-{}-{}.raw",
        coordinate_component(row),
        coordinate_component(column),
        kind.filename()
    ));
    let output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&raw_path)?;
    let mut raw = BufWriter::new(output);
    let tile_origin_x = i64::from(column - grid.minimum_column) * i64::from(width);
    let tile_origin_y = i64::from(row - grid.minimum_row) * i64::from(height);
    let mut peak_row_bytes = 0_u64;
    let ownership_bytes = u64::try_from(ownership.len())
        .map_err(|_| ReferenceError::Invalid("ownership memory does not fit u64".into()))?
        .checked_mul(2)
        .ok_or_else(|| ReferenceError::Invalid("ownership memory overflowed".into()))?;
    for output_y in 0..height {
        let global_y = tile_origin_y + i64::from(output_y);
        let mut rows = Vec::with_capacity(sources.len());
        let mut current_row_bytes = ownership_bytes;
        for (index, source) in sources.iter().enumerate() {
            if let Some(local_y) = source_local_y(source, grid, global_y) {
                let bytes = decoded[index].layers[&kind].read_row(local_y)?;
                current_row_bytes = current_row_bytes
                    .checked_add(u64::try_from(bytes.len()).map_err(|_| {
                        ReferenceError::Invalid("atlas layer row memory overflowed".into())
                    })?)
                    .ok_or_else(|| {
                        ReferenceError::Invalid("atlas layer row memory overflowed".into())
                    })?;
                rows.push((index, bytes));
            }
        }
        let row_bytes = usize::try_from(u64::from(width) * u64::from(bytes_per_pixel))
            .map_err(|_| ReferenceError::Invalid("atlas output row does not fit memory".into()))?;
        let mut output_row = vec![0_u8; row_bytes];
        current_row_bytes = current_row_bytes
            .checked_add(u64::try_from(row_bytes).map_err(|_| {
                ReferenceError::Invalid("atlas output row memory overflowed".into())
            })?)
            .ok_or_else(|| ReferenceError::Invalid("atlas output row memory overflowed".into()))?;
        peak_row_bytes = peak_row_bytes.max(current_row_bytes);
        for output_x in 0..width {
            let ownership_offset =
                usize::try_from(u64::from(output_y) * u64::from(width) + u64::from(output_x))
                    .map_err(|_| {
                        ReferenceError::Invalid("atlas ownership offset overflowed".into())
                    })?;
            let owner = usize::from(ownership[ownership_offset]);
            let (_, source_row) =
                rows.iter()
                    .find(|(index, _)| *index == owner)
                    .ok_or_else(|| {
                        ReferenceError::Invalid("atlas owner does not cover output row".into())
                    })?;
            let global_x = tile_origin_x + i64::from(output_x);
            let local_x = source_local_x(&sources[owner], grid, global_x).ok_or_else(|| {
                ReferenceError::Invalid("atlas owner does not cover output pixel".into())
            })?;
            let source_offset = usize::try_from(local_x)
                .map_err(|_| ReferenceError::Invalid("atlas source x does not fit memory".into()))?
                .checked_mul(usize::from(bytes_per_pixel))
                .ok_or_else(|| ReferenceError::Invalid("atlas source offset overflowed".into()))?;
            let output_offset = usize::try_from(output_x)
                .map_err(|_| ReferenceError::Invalid("atlas output x does not fit memory".into()))?
                .checked_mul(usize::from(bytes_per_pixel))
                .ok_or_else(|| ReferenceError::Invalid("atlas output offset overflowed".into()))?;
            output_row[output_offset..output_offset + usize::from(bytes_per_pixel)]
                .copy_from_slice(
                    &source_row[source_offset..source_offset + usize::from(bytes_per_pixel)],
                );
        }
        raw.write_all(&output_row)?;
    }
    raw.flush()?;
    raw.get_ref().sync_all()?;
    drop(raw);

    let relative = format!("{tile_directory}/{}", kind.filename());
    let path = staging.join(&relative);
    if kind == LayerKind::LinearDepth {
        write_depth_from_raw(&raw_path, &path, width, height)?;
    } else {
        let color_type = match kind {
            LayerKind::Color | LayerKind::Whitebox | LayerKind::ViewNormal => {
                crate::PngColorType::Rgba
            }
            LayerKind::FixedShadow | LayerKind::Coverage => crate::PngColorType::Grayscale,
            LayerKind::LinearDepth => unreachable!("handled above"),
        };
        encode_raw_png(&raw_path, &path, width, height, color_type)?;
    }
    fs::remove_file(&raw_path)?;
    let byte_length = path.metadata()?.len();
    Ok((
        AtlasLayerTileRecord {
            column,
            row,
            kind,
            path: relative,
            encoding: kind.encoding().into(),
            width_px: width,
            height_px: height,
            byte_length,
            sha256: sha256_file(&path)?,
        },
        peak_row_bytes,
    ))
}

fn write_depth_from_raw(
    raw_path: &Path,
    output_path: &Path,
    width: u32,
    height: u32,
) -> Result<(), ReferenceError> {
    let expected = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| ReferenceError::Invalid("atlas depth length overflowed".into()))?;
    if raw_path.metadata()?.len() != expected {
        return Err(ReferenceError::Invalid(
            "atlas raw depth length is invalid".into(),
        ));
    }
    let output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output_path)?;
    let mut writer = BufWriter::new(output);
    writer.write_all(DEPTH_MAGIC)?;
    writer.write_all(&width.to_le_bytes())?;
    writer.write_all(&height.to_le_bytes())?;
    let mut reader = BufReader::new(File::open(raw_path)?);
    std::io::copy(&mut reader, &mut writer)?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    Ok(())
}

fn source_local_x(source: &SourceBundle, grid: &AtlasGrid, global_x: i64) -> Option<u32> {
    let core_origin = i64::from(source.manifest.tile.column - grid.minimum_column)
        * i64::from(grid.core_width_px);
    let total_origin = core_origin - i64::from(grid.source_guard_px);
    let local = global_x - total_origin;
    u32::try_from(local)
        .ok()
        .filter(|value| *value < source.manifest.tile.total_width_px())
}

fn source_local_y(source: &SourceBundle, grid: &AtlasGrid, global_y: i64) -> Option<u32> {
    let core_origin =
        i64::from(source.manifest.tile.row - grid.minimum_row) * i64::from(grid.core_height_px);
    let total_origin = core_origin - i64::from(grid.source_guard_px);
    let local = global_y - total_origin;
    u32::try_from(local)
        .ok()
        .filter(|value| *value < source.manifest.tile.total_height_px())
}

#[expect(
    clippy::too_many_lines,
    reason = "the canonical manifest invariant is audited as one fail-closed validator"
)]
fn validate_atlas_manifest(manifest: &ReferenceAtlasManifest) -> Result<(), ReferenceError> {
    validate_session(&manifest.source_session)?;
    if manifest.schema != ATLAS_MANIFEST_SCHEMA
        || !is_identifier(&manifest.atlas_id)
        || manifest.provider != "google-photorealistic-3d-tiles"
        || manifest.renderer.is_empty()
        || manifest.renderer_version.is_empty()
        || manifest.source_epoch.is_empty()
        || manifest.attributions.is_empty()
        || manifest.grid.columns == 0
        || manifest.grid.rows == 0
        || manifest.grid.core_width_px == 0
        || manifest.grid.core_height_px == 0
        || manifest.grid.source_guard_px == 0
        || manifest.grid.millimeters_per_pixel == 0
        || manifest.grid.registration_error_micropixels > 500_000
        || !is_sha256(&manifest.ownership_map_sha256)
    {
        return Err(ReferenceError::Invalid(
            "atlas manifest identity or grid is invalid".into(),
        ));
    }
    let cell_count = usize::from(manifest.grid.columns)
        .checked_mul(usize::from(manifest.grid.rows))
        .ok_or_else(|| ReferenceError::Invalid("atlas cell count overflowed".into()))?;
    if cell_count == 0
        || cell_count > MAX_ATLAS_CELLS
        || manifest.sources.len() != cell_count
        || manifest.ownership_tiles.len() != cell_count
        || manifest.layer_tiles.len() != cell_count * required_layer_kinds().len()
    {
        return Err(ReferenceError::Invalid(
            "atlas manifest does not contain one complete rectangular tile set".into(),
        ));
    }
    let expected_width = u32::from(manifest.grid.columns)
        .checked_mul(u32::from(manifest.grid.core_width_px))
        .ok_or_else(|| ReferenceError::Invalid("atlas width overflowed".into()))?;
    let expected_height = u32::from(manifest.grid.rows)
        .checked_mul(u32::from(manifest.grid.core_height_px))
        .ok_or_else(|| ReferenceError::Invalid("atlas height overflowed".into()))?;
    if manifest.grid.width_px != expected_width || manifest.grid.height_px != expected_height {
        return Err(ReferenceError::Invalid(
            "atlas dimensions contradict the rectangular grid".into(),
        ));
    }

    let expected_cells = expected_cells(&manifest.grid);
    let mut source_cells = BTreeSet::new();
    for (index, source) in manifest.sources.iter().enumerate() {
        let expected_index = u16::try_from(index)
            .map_err(|_| ReferenceError::Invalid("atlas source index exceeds u16".into()))?;
        if source.source_index != expected_index
            || !is_identifier(&source.bundle_id)
            || !is_sha256(&source.manifest_sha256)
            || !(crate::MIN_CORE_COVERAGE_BASIS_POINTS..=10_000)
                .contains(&source.core_coverage_basis_points)
            || !source_cells.insert((source.column, source.row))
        {
            return Err(ReferenceError::Invalid(
                "atlas source record is invalid or duplicated".into(),
            ));
        }
    }
    if source_cells != expected_cells {
        return Err(ReferenceError::Invalid(
            "atlas sources do not cover the declared grid".into(),
        ));
    }

    let mut ownership_cells = BTreeSet::new();
    for tile in &manifest.ownership_tiles {
        let expected_path = format!("{}/ownership.bin", tile_directory(tile.column, tile.row));
        if tile.path != expected_path
            || tile.width_px != u32::from(manifest.grid.core_width_px)
            || tile.height_px != u32::from(manifest.grid.core_height_px)
            || tile.byte_length == 0
            || !is_sha256(&tile.sha256)
            || !ownership_cells.insert((tile.column, tile.row))
        {
            return Err(ReferenceError::Invalid(
                "atlas ownership record is invalid or duplicated".into(),
            ));
        }
    }
    if ownership_cells != expected_cells
        || ownership_digest(&manifest.ownership_tiles) != manifest.ownership_map_sha256
    {
        return Err(ReferenceError::Invalid(
            "atlas ownership map is incomplete or has the wrong digest".into(),
        ));
    }

    let mut layer_cells = BTreeSet::new();
    for tile in &manifest.layer_tiles {
        let expected_path = format!(
            "{}/{}",
            tile_directory(tile.column, tile.row),
            tile.kind.filename()
        );
        if tile.path != expected_path
            || tile.encoding != tile.kind.encoding()
            || tile.width_px != u32::from(manifest.grid.core_width_px)
            || tile.height_px != u32::from(manifest.grid.core_height_px)
            || tile.byte_length == 0
            || !is_sha256(&tile.sha256)
            || !layer_cells.insert((tile.column, tile.row, tile.kind))
        {
            return Err(ReferenceError::Invalid(
                "atlas layer record is invalid or duplicated".into(),
            ));
        }
    }
    let expected_layer_cells = expected_cells
        .iter()
        .flat_map(|(column, row)| {
            required_layer_kinds()
                .into_iter()
                .map(move |kind| (*column, *row, kind))
        })
        .collect::<BTreeSet<_>>();
    if layer_cells != expected_layer_cells {
        return Err(ReferenceError::Invalid(
            "atlas layer tiles do not cover every declared cell".into(),
        ));
    }
    Ok(())
}

fn validate_ownership_file(
    path: &Path,
    tile: &OwnershipTileRecord,
    manifest: &ReferenceAtlasManifest,
) -> Result<(), ReferenceError> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut header = [0_u8; 16];
    reader.read_exact(&mut header)?;
    if &header[..8] != OWNERSHIP_MAGIC
        || u32::from_le_bytes(header[8..12].try_into().expect("fixed width")) != tile.width_px
        || u32::from_le_bytes(header[12..16].try_into().expect("fixed height")) != tile.height_px
    {
        return Err(ReferenceError::Invalid(
            "atlas ownership header is invalid".into(),
        ));
    }
    let expected_length = 16_u64
        .checked_add(
            u64::from(tile.width_px)
                .checked_mul(u64::from(tile.height_px))
                .and_then(|pixels| pixels.checked_mul(2))
                .ok_or_else(|| ReferenceError::Invalid("ownership length overflowed".into()))?,
        )
        .ok_or_else(|| ReferenceError::Invalid("ownership length overflowed".into()))?;
    if tile.byte_length != expected_length {
        return Err(ReferenceError::Invalid(
            "atlas ownership length is invalid".into(),
        ));
    }
    let tile_origin_x =
        i64::from(tile.column - manifest.grid.minimum_column) * i64::from(tile.width_px);
    let tile_origin_y = i64::from(tile.row - manifest.grid.minimum_row) * i64::from(tile.height_px);
    let mut owner_bytes = [0_u8; 2];
    for y in 0..tile.height_px {
        for x in 0..tile.width_px {
            reader.read_exact(&mut owner_bytes)?;
            let owner = usize::from(u16::from_le_bytes(owner_bytes));
            let source = manifest.sources.get(owner).ok_or_else(|| {
                ReferenceError::Invalid("ownership references an unknown source".into())
            })?;
            let global_x = tile_origin_x + i64::from(x);
            let global_y = tile_origin_y + i64::from(y);
            if !source_record_covers(source, &manifest.grid, global_x, global_y) {
                return Err(ReferenceError::Invalid(
                    "ownership assigns a pixel outside its source guard".into(),
                ));
            }
        }
    }
    Ok(())
}

fn source_record_covers(
    source: &AtlasSourceRecord,
    grid: &AtlasGrid,
    global_x: i64,
    global_y: i64,
) -> bool {
    let core_x = i64::from(source.column - grid.minimum_column) * i64::from(grid.core_width_px);
    let core_y = i64::from(source.row - grid.minimum_row) * i64::from(grid.core_height_px);
    let guard = i64::from(grid.source_guard_px);
    global_x >= core_x - guard
        && global_x < core_x + i64::from(grid.core_width_px) + guard
        && global_y >= core_y - guard
        && global_y < core_y + i64::from(grid.core_height_px) + guard
}

fn validate_regular_hashed_file(
    path: &Path,
    expected_length: u64,
    expected_sha256: &str,
) -> Result<(), ReferenceError> {
    let metadata = path.symlink_metadata()?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() != expected_length
        || sha256_file(path)? != expected_sha256
    {
        return Err(ReferenceError::Invalid(format!(
            "atlas artifact {} fails its hash or file contract",
            path.display()
        )));
    }
    Ok(())
}

fn read_png_dimensions(path: &Path, kind: LayerKind) -> Result<(u32, u32), ReferenceError> {
    let decoder = png::Decoder::new(BufReader::new(File::open(path)?));
    let reader = decoder
        .read_info()
        .map_err(|error| ReferenceError::Invalid(format!("atlas PNG header failed: {error}")))?;
    let info = reader.info();
    let expected_color = match kind {
        LayerKind::Color | LayerKind::Whitebox | LayerKind::ViewNormal => png::ColorType::Rgba,
        LayerKind::FixedShadow | LayerKind::Coverage => png::ColorType::Grayscale,
        LayerKind::LinearDepth => {
            return Err(ReferenceError::Invalid(
                "depth cannot use a PNG atlas record".into(),
            ));
        }
    };
    if info.bit_depth != png::BitDepth::Eight || info.color_type != expected_color {
        return Err(ReferenceError::Invalid(
            "atlas PNG header has the wrong color contract".into(),
        ));
    }
    Ok((info.width, info.height))
}

fn read_depth_dimensions(path: &Path) -> Result<(u32, u32), ReferenceError> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut header = [0_u8; 16];
    reader.read_exact(&mut header)?;
    if &header[..8] != DEPTH_MAGIC {
        return Err(ReferenceError::Invalid(
            "atlas depth header has the wrong magic".into(),
        ));
    }
    Ok((
        u32::from_le_bytes(header[8..12].try_into().expect("fixed depth width")),
        u32::from_le_bytes(header[12..16].try_into().expect("fixed depth height")),
    ))
}

fn ownership_digest(tiles: &[OwnershipTileRecord]) -> String {
    let mut digest = Sha256::new();
    for tile in tiles {
        digest.update(tile.row.to_le_bytes());
        digest.update(tile.column.to_le_bytes());
        digest.update(tile.sha256.as_bytes());
    }
    hex_digest(digest.finalize())
}

fn expected_cells(grid: &AtlasGrid) -> BTreeSet<(i32, i32)> {
    (0..grid.rows)
        .flat_map(|row| {
            (0..grid.columns).map(move |column| {
                (
                    grid.minimum_column + i32::from(column),
                    grid.minimum_row + i32::from(row),
                )
            })
        })
        .collect()
}

fn required_layer_kinds() -> [LayerKind; 6] {
    [
        LayerKind::Color,
        LayerKind::Whitebox,
        LayerKind::LinearDepth,
        LayerKind::ViewNormal,
        LayerKind::FixedShadow,
        LayerKind::Coverage,
    ]
}

fn tile_directory(column: i32, row: i32) -> String {
    format!(
        "tiles/r{}-c{}",
        coordinate_component(row),
        coordinate_component(column)
    )
}

fn coordinate_component(value: i32) -> String {
    if value < 0 {
        format!("n{:08}", value.unsigned_abs())
    } else {
        format!("p{:08}", value.unsigned_abs())
    }
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), ReferenceError> {
    let output = OpenOptions::new().create_new(true).write(true).open(path)?;
    let mut writer = BufWriter::new(output);
    writer.write_all(bytes)?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, ReferenceError> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
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
    let mut output = String::with_capacity(bytes.as_ref().len() * 2);
    for byte in bytes.as_ref() {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
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
    use crate::{
        CaptureSpec, LayerRecord, LightingSpec, PngColorType, TileSpec, canonical_manifest_json,
    };

    fn fixture_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "isometric-atlas-{name}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create atlas fixture root");
        root
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the synthetic bundle fixture keeps all registered layers in one constructor"
    )]
    fn write_bundle(root: &Path, column: i32, row: i32) -> PathBuf {
        let directory = root.join(format!("bundle-{column}-{row}"));
        fs::create_dir(&directory).expect("create bundle directory");
        let tile = TileSpec {
            region_id: "hoover-atlas".into(),
            column,
            row,
            core_width_px: 4,
            core_height_px: 4,
            guard_px: 1,
            millimeters_per_pixel: 125,
            center_longitude_e7: -1_221_700_000 + column * 10,
            center_latitude_e7: 374_280_000 + row * 10,
        };
        let width = tile.total_width_px();
        let height = tile.total_height_px();
        let mut layers = Vec::new();
        for kind in required_layer_kinds() {
            let path = directory.join(kind.filename());
            if kind == LayerKind::LinearDepth {
                let raw = directory.join("depth.raw");
                let mut pixels = Vec::new();
                for _ in 0..width * height {
                    pixels.extend_from_slice(&1_000_u32.to_le_bytes());
                }
                fs::write(&raw, pixels).expect("write fixture depth raw");
                write_depth_from_raw(&raw, &path, width, height).expect("write fixture depth");
                fs::remove_file(raw).expect("remove fixture depth raw");
            } else {
                let color_type = match kind {
                    LayerKind::Color | LayerKind::Whitebox | LayerKind::ViewNormal => {
                        PngColorType::Rgba
                    }
                    LayerKind::FixedShadow | LayerKind::Coverage => PngColorType::Grayscale,
                    LayerKind::LinearDepth => unreachable!(),
                };
                let channels = match color_type {
                    PngColorType::Grayscale => 1,
                    PngColorType::Rgba => 4,
                };
                let value = if kind == LayerKind::Coverage {
                    255
                } else if kind == LayerKind::ViewNormal {
                    127
                } else {
                    u8::try_from((column + row * 2 + 4) * 20).expect("fixture value")
                };
                let mut pixels = vec![value; usize::try_from(width * height).unwrap() * channels];
                if kind == LayerKind::ViewNormal {
                    for pixel in pixels.chunks_exact_mut(4) {
                        pixel[3] = 255;
                    }
                }
                let raw = directory.join(format!("{}.raw", kind.filename()));
                fs::write(&raw, pixels).expect("write fixture PNG raw");
                encode_raw_png(&raw, &path, width, height, color_type).expect("encode fixture PNG");
                fs::remove_file(raw).expect("remove fixture PNG raw");
            }
            let bytes = fs::read(&path).expect("read fixture layer");
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
        let manifest = ReferenceManifest {
            schema: crate::MANIFEST_SCHEMA.into(),
            bundle_id: format!("hoover-{column}-{row}").replace('-', "n"),
            tile,
            camera: CameraSpec {
                projection: "orthographic".into(),
                azimuth_millidegrees: 330_000,
                elevation_millidegrees: 42_000,
                target_altitude_mm: 20_000,
                near_mm: 1_000,
                far_mm: 5_000_000,
                orthographic_width_mm: u64::from(width) * 125,
                orthographic_height_mm: u64::from(height) * 125,
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
                source_epoch: "2026-08-30T00:00:00Z".into(),
                complete: true,
                attributions: vec!["copyright:fixture-google".into()],
            },
            core_coverage_basis_points: 10_000,
            layers,
        };
        fs::write(
            directory.join(crate::MANIFEST_FILENAME),
            canonical_manifest_json(&manifest).expect("canonical fixture manifest"),
        )
        .expect("write fixture manifest");
        directory
    }

    fn request(directories: &[PathBuf]) -> AtlasCompileRequest {
        AtlasCompileRequest {
            schema: ATLAS_REQUEST_SCHEMA.into(),
            atlas_id: "hoover-reference-atlas".into(),
            source_session: SessionRecord {
                session_id: "fixture-session".into(),
                root_tileset_sha256: "a".repeat(64),
                started_at: "2026-08-30T00:00:00Z".into(),
                expires_at: "2026-08-30T03:00:00Z".into(),
            },
            bundle_directories: directories
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
        }
    }

    #[test]
    fn two_by_two_atlas_is_exact_order_independent_and_complete() {
        let root = fixture_root("exact");
        let mut directories = vec![
            write_bundle(&root, 0, 0),
            write_bundle(&root, 1, 0),
            write_bundle(&root, 0, 1),
            write_bundle(&root, 1, 1),
        ];
        let first_output = root.join("atlas-first");
        let first = compile_atlas(&root, &request(&directories), &first_output)
            .expect("compile first atlas");
        directories.reverse();
        let second_output = root.join("atlas-second");
        let second = compile_atlas(&root, &request(&directories), &second_output)
            .expect("compile permuted atlas");
        let third_output = root.join("atlas-third");
        let third = compile_atlas(&root, &request(&directories), &third_output)
            .expect("compile exact atlas rerun");
        assert_eq!(first.manifest_sha256, second.manifest_sha256);
        assert_eq!(second.manifest_sha256, third.manifest_sha256);
        assert_eq!(first.total_bytes, third.total_bytes);
        assert_eq!(first.tile_count, 4);
        assert!(first.peak_row_buffer_bytes > 0);
        let manifest = read_atlas_manifest(&first_output.join(ATLAS_MANIFEST_FILENAME))
            .expect("read atlas manifest");
        assert_eq!(manifest.grid.width_px, 8);
        assert_eq!(manifest.grid.height_px, 8);
        assert_eq!(manifest.grid.registration_error_micropixels, 0);
        assert_eq!(manifest.layer_tiles.len(), 24);
        assert_eq!(manifest.ownership_tiles.len(), 4);
        let inspected = validate_atlas(&first_output, &manifest).expect("inspect atlas");
        assert_eq!(inspected.manifest_sha256, first.manifest_sha256);
        fs::remove_dir_all(root).expect("remove atlas fixture root");
    }

    #[test]
    fn rejects_duplicate_gap_camera_and_corrupt_ownership() {
        let root = fixture_root("invalid");
        let zero = write_bundle(&root, 0, 0);
        let one = write_bundle(&root, 1, 0);
        let duplicate = request(&[zero.clone(), zero.clone()]);
        assert!(compile_atlas(&root, &duplicate, &root.join("duplicate")).is_err());

        let mut invalid_session = request(std::slice::from_ref(&zero));
        invalid_session.source_session.session_id = "unsafe/session".into();
        assert!(compile_atlas(&root, &invalid_session, &root.join("session")).is_err());

        let gap = write_bundle(&root, 2, 0);
        assert!(compile_atlas(&root, &request(&[zero.clone(), gap]), &root.join("gap")).is_err());

        let manifest_path = one.join(crate::MANIFEST_FILENAME);
        let mut manifest = read_manifest(&manifest_path).expect("read second fixture manifest");
        manifest.camera.elevation_millidegrees = 35_264;
        fs::write(
            &manifest_path,
            canonical_manifest_json(&manifest).expect("encode changed camera"),
        )
        .expect("write changed camera");
        assert!(
            compile_atlas(
                &root,
                &request(&[zero.clone(), one.clone()]),
                &root.join("camera")
            )
            .is_err()
        );

        manifest.camera.elevation_millidegrees = 42_000;
        fs::write(
            &manifest_path,
            canonical_manifest_json(&manifest).expect("encode restored camera"),
        )
        .expect("restore camera manifest");
        let valid = one;
        let output = root.join("valid-atlas");
        compile_atlas(&root, &request(&[zero, valid]), &output).expect("compile valid atlas");
        let manifest = read_atlas_manifest(&output.join(ATLAS_MANIFEST_FILENAME))
            .expect("read valid atlas manifest");
        let ownership = output.join(&manifest.ownership_tiles[0].path);
        let mut bytes = fs::read(&ownership).expect("read ownership");
        bytes[16..18].copy_from_slice(&u16::MAX.to_le_bytes());
        fs::write(&ownership, bytes).expect("corrupt ownership");
        assert!(validate_atlas(&output, &manifest).is_err());
        fs::remove_dir_all(root).expect("remove atlas fixture root");
    }
}

//! Immutable, registered semantic-mask artifacts.
//!
//! Model inference may propose masks, but this crate owns the portable pixel
//! ontology, exact reference registration, bounded streaming validation, and
//! the boundary that prevents transient evidence from becoming persistent
//! world content.

use std::{
    error::Error,
    fmt::{Display, Formatter, Write as _},
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Portable semantic-mask manifest schema.
pub const MANIFEST_SCHEMA: &str = "isometric-mask-manifest/v1";
/// Stable semantic ontology identifier.
pub const ONTOLOGY_ID: &str = "isometric-stanford-semantics/v1";
/// Canonical manifest filename inside a mask artifact.
pub const MANIFEST_FILENAME: &str = "mask.manifest.json";
/// Canonical binary mask filename inside a mask artifact.
pub const MASK_FILENAME: &str = "semantics.mask";
/// Portable fixed-width mask encoding.
pub const MASK_ENCODING: &str = "isometric-mask-pixel-u8-u8-u16le-u32le/v1";

const MASK_MAGIC: &[u8; 8] = b"ISOMSKV1";
const HEADER_BYTES: usize = 20;
const HEADER_BYTES_U64: u64 = 20;
const RECORD_BYTES: u16 = 8;
const HASH_BUFFER_BYTES: usize = 64 * 1_024;
const MAX_MANIFEST_BYTES: u64 = 1_024 * 1_024;
const MAX_MASK_DIMENSION: u32 = 4_096;
const MAX_INSTANCE_ID: u32 = 262_144;
const CLASS_COUNT: usize = 24;
const UNSEEN_INSTANCE: u8 = u8::MAX;

/// Pixel-level semantic class used by reference masking.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticClass {
    /// Unresolved evidence. Qualification decides whether it is acceptable.
    Unknown = 0,
    /// Pixels outside usable scene geometry.
    SkyOrVoid = 1,
    /// Persistent bare or general terrain.
    Terrain = 2,
    /// Persistent lawn or grass.
    Grass = 3,
    /// Persistent tree crown or canopy.
    TreeCanopy = 4,
    /// Persistent water.
    Water = 5,
    /// Persistent vehicle road.
    Road = 6,
    /// Persistent parking hardscape.
    Parking = 7,
    /// Persistent pedestrian circulation.
    PedestrianPath = 8,
    /// Persistent crosswalk marking.
    CrosswalkMarking = 9,
    /// Persistent non-crosswalk road marking.
    RoadMarking = 10,
    /// Persistent building roof.
    BuildingRoof = 11,
    /// Persistent building facade.
    BuildingFacade = 12,
    /// Persistent window, door, or architectural opening.
    WindowOrOpening = 13,
    /// Persistent landmark-specific detail.
    LandmarkDetail = 14,
    /// Persistent lamppost visible at the accepted scale.
    Lamppost = 15,
    /// Persistent traffic signal visible at the accepted scale.
    TrafficSignal = 16,
    /// Persistent sign or bollard visible at the accepted scale.
    SignOrBollard = 17,
    /// Transient person obstruction.
    Person = 18,
    /// Transient bicycle obstruction.
    Bicycle = 19,
    /// Transient car obstruction.
    Car = 20,
    /// Transient bus or truck obstruction.
    BusOrTruck = 21,
    /// Transient construction equipment obstruction.
    ConstructionEquipment = 22,
    /// Broken or missing reference-source content.
    SourceArtifact = 23,
}

impl SemanticClass {
    /// Every class in stable binary order.
    pub const ALL: [Self; CLASS_COUNT] = [
        Self::Unknown,
        Self::SkyOrVoid,
        Self::Terrain,
        Self::Grass,
        Self::TreeCanopy,
        Self::Water,
        Self::Road,
        Self::Parking,
        Self::PedestrianPath,
        Self::CrosswalkMarking,
        Self::RoadMarking,
        Self::BuildingRoof,
        Self::BuildingFacade,
        Self::WindowOrOpening,
        Self::LandmarkDetail,
        Self::Lamppost,
        Self::TrafficSignal,
        Self::SignOrBollard,
        Self::Person,
        Self::Bicycle,
        Self::Car,
        Self::BusOrTruck,
        Self::ConstructionEquipment,
        Self::SourceArtifact,
    ];

    /// Whether the class represents a removable captured obstruction.
    #[must_use]
    pub const fn is_transient(self) -> bool {
        matches!(
            self,
            Self::Person
                | Self::Bicycle
                | Self::Car
                | Self::BusOrTruck
                | Self::ConstructionEquipment
        )
    }

    /// Whether the class may survive into a persistent mask artifact.
    #[must_use]
    pub const fn is_persistent_compatible(self) -> bool {
        !self.is_transient() && !matches!(self, Self::SourceArtifact)
    }

    const fn index(self) -> usize {
        self as usize
    }
}

impl TryFrom<u8> for SemanticClass {
    type Error = MaskError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::ALL.get(usize::from(value)).copied().ok_or_else(|| {
            MaskError::Invalid(format!("mask pixel uses unknown semantic class {value}"))
        })
    }
}

/// Validated provenance flags for one mask pixel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidenceFlags(u16);

impl EvidenceFlags {
    /// No contributing evidence, normally used only for unresolved pixels.
    pub const NONE: Self = Self(0);
    /// Depth or normal geometry evidence.
    pub const GEOMETRY: Self = Self(1 << 0);
    /// Dense semantic-model evidence.
    pub const DENSE_MODEL: Self = Self(1 << 1);
    /// Object-detector evidence.
    pub const DETECTOR: Self = Self(1 << 2);
    /// Prompted instance-segmentation evidence.
    pub const PROMPT_SEGMENTATION: Self = Self(1 << 3);
    /// Projected geographic prior.
    pub const GEOGRAPHIC_PRIOR: Self = Self(1 << 4);
    /// Deterministic structural or material prior.
    pub const STRUCTURAL_PRIOR: Self = Self(1 << 5);
    /// Reviewed human correction to a semantic mask.
    pub const HUMAN_CORRECTION: Self = Self(1 << 6);
    /// Evidence derived by accepted obstruction repair.
    pub const INFILL_DERIVATION: Self = Self(1 << 7);

    const VALID_BITS: u16 = (1 << 8) - 1;

    /// Construct flags only when every bit belongs to the v1 evidence schema.
    ///
    /// # Errors
    ///
    /// Returns an error when any reserved bit is set.
    pub fn from_bits(bits: u16) -> Result<Self, MaskError> {
        if bits & !Self::VALID_BITS != 0 {
            return Err(MaskError::Invalid(format!(
                "mask evidence contains reserved bits 0x{bits:04x}"
            )));
        }
        Ok(Self(bits))
    }

    /// Return the stable portable bit representation.
    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Combine two validated evidence sources.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether all supplied evidence bits are present.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// One fixed-width semantic mask pixel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaskPixel {
    /// Fused semantic classification.
    pub class: SemanticClass,
    /// Quantized confidence from 0 through 255.
    pub confidence: u8,
    /// Stable zero-based absence or positive instance identity.
    pub instance_id: u32,
    /// Contributing evidence sources.
    pub evidence: EvidenceFlags,
}

/// Intended stage for one immutable mask artifact.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactRole {
    /// Raw accepted evidence. Transient and source-artifact classes are legal.
    Evidence,
    /// Fused input to repair. Transient and source-artifact classes are legal.
    RepairInput,
    /// Repaired persistent content. Transient and artifact classes are illegal.
    Persistent,
}

impl ArtifactRole {
    /// Stable human-readable role identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Evidence => "evidence",
            Self::RepairInput => "repair-input",
            Self::Persistent => "persistent",
        }
    }
}

/// Exact registration with one validated reference bundle.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReferenceRegistration {
    /// Reference bundle identity.
    pub bundle_id: String,
    /// SHA-256 of the canonical registered reference manifest.
    pub manifest_sha256: String,
    /// Registered region identity.
    pub region_id: String,
    /// Registered full guarded width.
    pub width_px: u32,
    /// Registered full guarded height.
    pub height_px: u32,
    /// SHA-256 of the canonical grid and camera identity record.
    pub grid_sha256: String,
}

impl ReferenceRegistration {
    /// Derive exact mask registration from one canonical reference manifest.
    ///
    /// The grid digest covers the complete tile and orthographic camera
    /// contracts. Lighting and layer hashes remain covered by the parent
    /// reference-manifest digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the reference manifest is invalid or cannot be
    /// encoded canonically.
    pub fn from_reference_manifest(
        manifest: &isometric_reference::ReferenceManifest,
    ) -> Result<Self, MaskError> {
        let manifest_json =
            isometric_reference::canonical_manifest_json(manifest).map_err(|error| {
                MaskError::Invalid(format!("reference registration failed: {error}"))
            })?;
        let grid_json = serde_json::to_vec(&(&manifest.tile, &manifest.camera))?;
        Ok(Self {
            bundle_id: manifest.bundle_id.clone(),
            manifest_sha256: sha256_bytes(manifest_json.as_bytes()),
            region_id: manifest.tile.region_id.clone(),
            width_px: manifest.tile.total_width_px(),
            height_px: manifest.tile.total_height_px(),
            grid_sha256: sha256_bytes(&grid_json),
        })
    }
}

/// Identity of the deterministic compiler or frozen model stage.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProducerIdentity {
    /// Stable producer name.
    pub name: String,
    /// Pinned implementation, model, or weights version.
    pub version: String,
}

/// One binary mask file record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MaskLayerRecord {
    /// Canonical artifact-relative path.
    pub path: String,
    /// Portable binary encoding identity.
    pub encoding: String,
    /// Registered pixel width.
    pub width_px: u32,
    /// Registered pixel height.
    pub height_px: u32,
    /// Fixed bytes per pixel record.
    pub record_bytes: u16,
    /// Exact file length including its header.
    pub byte_length: u64,
    /// SHA-256 over exact file bytes.
    pub sha256: String,
}

/// Count for one class in stable ontology order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClassCount {
    /// Semantic class.
    pub class: SemanticClass,
    /// Exact pixel count.
    pub pixels: u64,
}

/// Complete immutable mask artifact contract.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MaskManifest {
    /// Manifest schema identity.
    pub schema: String,
    /// Stable artifact identity.
    pub artifact_id: String,
    /// Intended stage and transient policy.
    pub role: ArtifactRole,
    /// Stable ontology identity.
    pub ontology: String,
    /// Immutable promotion marker.
    pub frozen: bool,
    /// Exact upstream reference registration.
    pub reference: ReferenceRegistration,
    /// Compiler or inference-stage identity.
    pub producer: ProducerIdentity,
    /// Binary mask file.
    pub mask: MaskLayerRecord,
    /// Counts for all classes in ontology order.
    pub class_counts: Vec<ClassCount>,
    /// Exact number of unresolved pixels.
    pub unknown_pixels: u64,
    /// Exact number of transient pixels.
    pub transient_pixels: u64,
    /// Number of distinct nonzero instance identities.
    pub instance_count: u32,
    /// Largest nonzero instance identity, or zero when none exist.
    pub max_instance_id: u32,
}

/// Inputs that do not depend on pixel content when creating an artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactDescriptor {
    /// Stable artifact identity.
    pub artifact_id: String,
    /// Intended stage and transient policy.
    pub role: ArtifactRole,
    /// Exact upstream reference registration.
    pub reference: ReferenceRegistration,
    /// Compiler or inference-stage identity.
    pub producer: ProducerIdentity,
}

/// Verified mask evidence returned without retaining the mask raster.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactReport {
    /// SHA-256 over canonical manifest JSON.
    pub manifest_sha256: String,
    /// Verified exact mask SHA-256.
    pub mask_sha256: String,
    /// Number of streamed pixels.
    pub pixel_count: u64,
    /// Number of unresolved pixels.
    pub unknown_pixels: u64,
    /// Number of transient pixels.
    pub transient_pixels: u64,
    /// Number of distinct instances.
    pub instance_count: u32,
}

/// Fail-closed mask artifact error.
#[derive(Debug)]
pub enum MaskError {
    /// A manifest, pixel, or artifact violates a contract invariant.
    Invalid(String),
    /// Local I/O failed.
    Io(std::io::Error),
    /// JSON decoding or encoding failed.
    Json(serde_json::Error),
}

impl Display for MaskError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::Io(error) => write!(formatter, "mask I/O failed: {error}"),
            Self::Json(error) => write!(formatter, "mask JSON failed: {error}"),
        }
    }
}

impl Error for MaskError {}

impl From<std::io::Error> for MaskError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for MaskError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

/// Read a bounded mask manifest without trusting its binary artifact.
///
/// # Errors
///
/// Returns an error when the path is not a regular non-symlink file, exceeds
/// the manifest ceiling, or does not contain valid JSON.
pub fn read_manifest(path: &Path) -> Result<MaskManifest, MaskError> {
    let metadata = path.symlink_metadata()?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_MANIFEST_BYTES
    {
        return Err(MaskError::Invalid(
            "mask manifest is not a bounded regular file".into(),
        ));
    }
    Ok(serde_json::from_reader(BufReader::new(File::open(path)?))?)
}

/// Serialize a manifest into its stable repository representation.
///
/// # Errors
///
/// Returns an error when the contract is invalid or JSON encoding fails.
pub fn canonical_manifest_json(manifest: &MaskManifest) -> Result<String, MaskError> {
    validate_manifest(manifest)?;
    let mut encoded = serde_json::to_string_pretty(manifest)?;
    encoded.push('\n');
    Ok(encoded)
}

/// Stream, hash, and validate one complete mask artifact.
///
/// No mask-sized raster is allocated. Instance-class consistency uses a fixed
/// table capped at 262,144 instance identities.
///
/// # Errors
///
/// Returns an error for any invalid manifest, unsafe file, malformed record,
/// incorrect digest or count, inconsistent instance, or transient pixel in a
/// persistent artifact.
pub fn validate_artifact(
    root: &Path,
    manifest: &MaskManifest,
) -> Result<ArtifactReport, MaskError> {
    validate_manifest(manifest)?;
    let path = root.join(&manifest.mask.path);
    let metadata = path.symlink_metadata()?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() != manifest.mask.byte_length
    {
        return Err(MaskError::Invalid(
            "mask payload byte length does not match".into(),
        ));
    }

    let mut reader = BufReader::with_capacity(HASH_BUFFER_BYTES, File::open(path)?);
    let mut digest = Sha256::new();
    let mut header = [0_u8; HEADER_BYTES];
    reader.read_exact(&mut header)?;
    digest.update(header);
    validate_header(&header, &manifest.mask)?;

    let mut class_counts = [0_u64; CLASS_COUNT];
    let mut instance_classes = vec![UNSEEN_INSTANCE; instance_table_len(manifest)?];
    let mut distinct_instances = 0_u32;
    let mut observed_max_instance = 0_u32;
    let pixel_count = checked_pixel_count(manifest.mask.width_px, manifest.mask.height_px)?;

    for _ in 0..pixel_count {
        let mut encoded = [0_u8; RECORD_BYTES as usize];
        reader.read_exact(&mut encoded)?;
        digest.update(encoded);
        let pixel = decode_pixel(encoded)?;
        validate_pixel(
            pixel,
            manifest.role,
            &mut instance_classes,
            &mut distinct_instances,
            &mut observed_max_instance,
        )?;
        class_counts[pixel.class.index()] += 1;
    }

    let mask_sha256 = hex_digest(digest.finalize());
    if mask_sha256 != manifest.mask.sha256 {
        return Err(MaskError::Invalid(
            "mask payload SHA-256 does not match".into(),
        ));
    }
    validate_observed_counts(
        manifest,
        &class_counts,
        distinct_instances,
        observed_max_instance,
    )?;

    let manifest_json = canonical_manifest_json(manifest)?;
    Ok(ArtifactReport {
        manifest_sha256: sha256_bytes(manifest_json.as_bytes()),
        mask_sha256,
        pixel_count,
        unknown_pixels: class_counts[SemanticClass::Unknown.index()],
        transient_pixels: transient_count(&class_counts),
        instance_count: distinct_instances,
    })
}

/// Atomically create one immutable artifact from a bounded exact-size iterator.
///
/// Pixels are written directly to disk. The iterator is never collected into
/// a mask-sized allocation.
///
/// # Errors
///
/// Returns an error when the destination already exists, dimensions are
/// unsafe, the iterator length differs from the registered grid, pixel
/// invariants fail, or the artifact cannot be written and verified.
pub fn write_artifact<I>(
    root: &Path,
    descriptor: ArtifactDescriptor,
    pixels: I,
) -> Result<ArtifactReport, MaskError>
where
    I: IntoIterator<Item = MaskPixel>,
    I::IntoIter: ExactSizeIterator,
{
    if root.exists() {
        return Err(MaskError::Invalid(
            "mask artifact destination already exists".into(),
        ));
    }
    validate_registration(&descriptor.reference)?;
    validate_identity(&descriptor.artifact_id, "mask artifact")?;
    validate_producer(&descriptor.producer)?;

    let expected_pixels = checked_pixel_count(
        descriptor.reference.width_px,
        descriptor.reference.height_px,
    )?;
    let iterator = pixels.into_iter();
    let iterator_len = u64::try_from(iterator.len())
        .map_err(|_| MaskError::Invalid("mask iterator length overflowed".into()))?;
    if iterator_len != expected_pixels {
        return Err(MaskError::Invalid(format!(
            "mask iterator has {iterator_len} pixels; expected {expected_pixels}"
        )));
    }

    let staging = staging_path(root)?;
    fs::create_dir(&staging)?;
    let result = write_staged_artifact(&staging, descriptor, iterator, expected_pixels).and_then(
        |manifest| {
            let report = validate_artifact(&staging, &manifest)?;
            fs::rename(&staging, root)?;
            Ok(report)
        },
    );
    if result.is_err() && staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    result
}

fn write_staged_artifact<I>(
    staging: &Path,
    descriptor: ArtifactDescriptor,
    pixels: I,
    expected_pixels: u64,
) -> Result<MaskManifest, MaskError>
where
    I: Iterator<Item = MaskPixel>,
{
    let mask_path = staging.join(MASK_FILENAME);
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&mask_path)?;
    let mut writer = BufWriter::with_capacity(HASH_BUFFER_BYTES, file);
    let header = encode_header(
        descriptor.reference.width_px,
        descriptor.reference.height_px,
    );
    writer.write_all(&header)?;
    let mut digest = Sha256::new();
    digest.update(header);
    let mut class_counts = [0_u64; CLASS_COUNT];
    let mut instance_classes = vec![
        UNSEEN_INSTANCE;
        usize::try_from(MAX_INSTANCE_ID + 1)
            .expect("instance ceiling fits usize")
    ];
    let mut distinct_instances = 0_u32;
    let mut observed_max_instance = 0_u32;
    let mut written_pixels = 0_u64;

    for pixel in pixels {
        validate_pixel(
            pixel,
            descriptor.role,
            &mut instance_classes,
            &mut distinct_instances,
            &mut observed_max_instance,
        )?;
        let encoded = encode_pixel(pixel);
        writer.write_all(&encoded)?;
        digest.update(encoded);
        class_counts[pixel.class.index()] += 1;
        written_pixels += 1;
    }
    if written_pixels != expected_pixels {
        return Err(MaskError::Invalid(
            "mask iterator changed length while writing".into(),
        ));
    }
    writer.flush()?;
    drop(writer);

    let mask_sha256 = hex_digest(digest.finalize());
    let byte_length = expected_mask_length(
        descriptor.reference.width_px,
        descriptor.reference.height_px,
    )?;
    let manifest = MaskManifest {
        schema: MANIFEST_SCHEMA.into(),
        artifact_id: descriptor.artifact_id,
        role: descriptor.role,
        ontology: ONTOLOGY_ID.into(),
        frozen: true,
        reference: descriptor.reference.clone(),
        producer: descriptor.producer,
        mask: MaskLayerRecord {
            path: MASK_FILENAME.into(),
            encoding: MASK_ENCODING.into(),
            width_px: descriptor.reference.width_px,
            height_px: descriptor.reference.height_px,
            record_bytes: RECORD_BYTES,
            byte_length,
            sha256: mask_sha256.clone(),
        },
        class_counts: SemanticClass::ALL
            .into_iter()
            .map(|class| ClassCount {
                class,
                pixels: class_counts[class.index()],
            })
            .collect(),
        unknown_pixels: class_counts[SemanticClass::Unknown.index()],
        transient_pixels: transient_count(&class_counts),
        instance_count: distinct_instances,
        max_instance_id: observed_max_instance,
    };
    let manifest_json = canonical_manifest_json(&manifest)?;
    let manifest_path = staging.join(MANIFEST_FILENAME);
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(manifest_path)?
        .write_all(manifest_json.as_bytes())?;
    Ok(manifest)
}

fn validate_manifest(manifest: &MaskManifest) -> Result<(), MaskError> {
    if manifest.schema != MANIFEST_SCHEMA || manifest.ontology != ONTOLOGY_ID || !manifest.frozen {
        return Err(MaskError::Invalid(
            "mask schema, ontology, or frozen state is invalid".into(),
        ));
    }
    validate_identity(&manifest.artifact_id, "mask artifact")?;
    validate_registration(&manifest.reference)?;
    validate_producer(&manifest.producer)?;
    let expected_pixels = checked_pixel_count(manifest.mask.width_px, manifest.mask.height_px)?;
    if manifest.mask.path != MASK_FILENAME
        || manifest.mask.encoding != MASK_ENCODING
        || manifest.mask.width_px != manifest.reference.width_px
        || manifest.mask.height_px != manifest.reference.height_px
        || manifest.mask.record_bytes != RECORD_BYTES
        || manifest.mask.byte_length
            != expected_mask_length(manifest.mask.width_px, manifest.mask.height_px)?
        || !is_sha256(&manifest.mask.sha256)
    {
        return Err(MaskError::Invalid(
            "mask layer violates its portable registered contract".into(),
        ));
    }
    if manifest.class_counts.len() != CLASS_COUNT
        || manifest
            .class_counts
            .iter()
            .map(|count| count.class)
            .ne(SemanticClass::ALL)
    {
        return Err(MaskError::Invalid(
            "mask class counts must contain the complete ontology in order".into(),
        ));
    }
    let counted_pixels = manifest
        .class_counts
        .iter()
        .try_fold(0_u64, |total, count| {
            total
                .checked_add(count.pixels)
                .ok_or_else(|| MaskError::Invalid("mask class counts overflowed".into()))
        })?;
    if counted_pixels != expected_pixels
        || manifest.unknown_pixels != manifest.class_counts[SemanticClass::Unknown.index()].pixels
        || manifest.transient_pixels
            != manifest
                .class_counts
                .iter()
                .filter(|count| count.class.is_transient())
                .map(|count| count.pixels)
                .sum::<u64>()
        || manifest.instance_count > manifest.max_instance_id
        || manifest.max_instance_id > MAX_INSTANCE_ID
        || (manifest.instance_count == 0) != (manifest.max_instance_id == 0)
    {
        return Err(MaskError::Invalid(
            "mask summary counts or instance bounds are invalid".into(),
        ));
    }
    if manifest.role == ArtifactRole::Persistent
        && (manifest.transient_pixels != 0
            || manifest.class_counts[SemanticClass::SourceArtifact.index()].pixels != 0)
    {
        return Err(MaskError::Invalid(
            "persistent mask contains transient or source-artifact pixels".into(),
        ));
    }
    Ok(())
}

fn validate_registration(reference: &ReferenceRegistration) -> Result<(), MaskError> {
    validate_identity(&reference.bundle_id, "reference bundle")?;
    validate_identity(&reference.region_id, "reference region")?;
    checked_pixel_count(reference.width_px, reference.height_px)?;
    if !is_sha256(&reference.manifest_sha256) || !is_sha256(&reference.grid_sha256) {
        return Err(MaskError::Invalid(
            "mask reference registration lacks canonical SHA-256 identities".into(),
        ));
    }
    Ok(())
}

fn validate_producer(producer: &ProducerIdentity) -> Result<(), MaskError> {
    validate_text(&producer.name, "mask producer name", 128)?;
    validate_text(&producer.version, "mask producer version", 256)
}

fn validate_identity(value: &str, name: &str) -> Result<(), MaskError> {
    if value.is_empty()
        || value.len() > 96
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(MaskError::Invalid(format!(
            "{name} must use a lowercase safe identifier"
        )));
    }
    Ok(())
}

fn validate_text(value: &str, name: &str, max_length: usize) -> Result<(), MaskError> {
    if value.is_empty()
        || value.len() > max_length
        || value.chars().any(char::is_control)
        || value.contains("../")
    {
        return Err(MaskError::Invalid(format!("{name} is invalid")));
    }
    Ok(())
}

fn validate_header(header: &[u8; HEADER_BYTES], mask: &MaskLayerRecord) -> Result<(), MaskError> {
    if &header[..8] != MASK_MAGIC
        || u32::from_le_bytes(header[8..12].try_into().expect("fixed width slice")) != mask.width_px
        || u32::from_le_bytes(header[12..16].try_into().expect("fixed height slice"))
            != mask.height_px
        || u16::from_le_bytes(header[16..18].try_into().expect("fixed record slice"))
            != RECORD_BYTES
        || header[18..20] != [0, 0]
    {
        return Err(MaskError::Invalid("mask binary header is invalid".into()));
    }
    Ok(())
}

fn validate_pixel(
    pixel: MaskPixel,
    role: ArtifactRole,
    instance_classes: &mut [u8],
    distinct_instances: &mut u32,
    observed_max_instance: &mut u32,
) -> Result<(), MaskError> {
    if role == ArtifactRole::Persistent && !pixel.class.is_persistent_compatible() {
        return Err(MaskError::Invalid(format!(
            "persistent mask contains forbidden {:?} pixel",
            pixel.class
        )));
    }
    if pixel.instance_id == 0 {
        return Ok(());
    }
    if matches!(
        pixel.class,
        SemanticClass::Unknown | SemanticClass::SkyOrVoid
    ) {
        return Err(MaskError::Invalid(
            "unknown or void mask pixel cannot carry an instance identity".into(),
        ));
    }
    let index = usize::try_from(pixel.instance_id)
        .map_err(|_| MaskError::Invalid("mask instance identity overflowed".into()))?;
    let instance_class = instance_classes
        .get_mut(index)
        .ok_or_else(|| MaskError::Invalid("mask instance identity exceeds its bound".into()))?;
    let class = pixel.class as u8;
    if *instance_class == UNSEEN_INSTANCE {
        *instance_class = class;
        *distinct_instances = distinct_instances
            .checked_add(1)
            .ok_or_else(|| MaskError::Invalid("mask instance count overflowed".into()))?;
    } else if *instance_class != class {
        return Err(MaskError::Invalid(format!(
            "mask instance {} spans multiple semantic classes",
            pixel.instance_id
        )));
    }
    *observed_max_instance = (*observed_max_instance).max(pixel.instance_id);
    Ok(())
}

fn validate_observed_counts(
    manifest: &MaskManifest,
    observed: &[u64; CLASS_COUNT],
    instance_count: u32,
    max_instance_id: u32,
) -> Result<(), MaskError> {
    if manifest
        .class_counts
        .iter()
        .zip(observed)
        .any(|(expected, actual)| expected.pixels != *actual)
        || manifest.unknown_pixels != observed[SemanticClass::Unknown.index()]
        || manifest.transient_pixels != transient_count(observed)
        || manifest.instance_count != instance_count
        || manifest.max_instance_id != max_instance_id
    {
        return Err(MaskError::Invalid(
            "mask binary content does not match manifest summaries".into(),
        ));
    }
    Ok(())
}

fn checked_pixel_count(width: u32, height: u32) -> Result<u64, MaskError> {
    if width == 0 || height == 0 || width > MAX_MASK_DIMENSION || height > MAX_MASK_DIMENSION {
        return Err(MaskError::Invalid(
            "mask dimensions are zero or exceed the registered ceiling".into(),
        ));
    }
    u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| MaskError::Invalid("mask pixel count overflowed".into()))
}

fn expected_mask_length(width: u32, height: u32) -> Result<u64, MaskError> {
    checked_pixel_count(width, height)?
        .checked_mul(u64::from(RECORD_BYTES))
        .and_then(|payload| payload.checked_add(HEADER_BYTES_U64))
        .ok_or_else(|| MaskError::Invalid("mask byte length overflowed".into()))
}

fn instance_table_len(manifest: &MaskManifest) -> Result<usize, MaskError> {
    let entries = manifest
        .max_instance_id
        .checked_add(1)
        .ok_or_else(|| MaskError::Invalid("mask instance table overflowed".into()))?;
    usize::try_from(entries)
        .map_err(|_| MaskError::Invalid("mask instance table does not fit memory".into()))
}

fn transient_count(class_counts: &[u64; CLASS_COUNT]) -> u64 {
    SemanticClass::ALL
        .into_iter()
        .filter(|class| class.is_transient())
        .map(|class| class_counts[class.index()])
        .sum()
}

fn encode_header(width: u32, height: u32) -> [u8; HEADER_BYTES] {
    let mut header = [0_u8; HEADER_BYTES];
    header[..8].copy_from_slice(MASK_MAGIC);
    header[8..12].copy_from_slice(&width.to_le_bytes());
    header[12..16].copy_from_slice(&height.to_le_bytes());
    header[16..18].copy_from_slice(&RECORD_BYTES.to_le_bytes());
    header
}

fn encode_pixel(pixel: MaskPixel) -> [u8; RECORD_BYTES as usize] {
    let mut encoded = [0_u8; RECORD_BYTES as usize];
    encoded[0] = pixel.class as u8;
    encoded[1] = pixel.confidence;
    encoded[2..4].copy_from_slice(&pixel.evidence.bits().to_le_bytes());
    encoded[4..8].copy_from_slice(&pixel.instance_id.to_le_bytes());
    encoded
}

fn decode_pixel(encoded: [u8; RECORD_BYTES as usize]) -> Result<MaskPixel, MaskError> {
    Ok(MaskPixel {
        class: SemanticClass::try_from(encoded[0])?,
        confidence: encoded[1],
        evidence: EvidenceFlags::from_bits(u16::from_le_bytes([encoded[2], encoded[3]]))?,
        instance_id: u32::from_le_bytes(encoded[4..8].try_into().expect("fixed instance slice")),
    })
}

fn staging_path(root: &Path) -> Result<PathBuf, MaskError> {
    let parent = root.parent().ok_or_else(|| {
        MaskError::Invalid("mask artifact destination requires a parent directory".into())
    })?;
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            MaskError::Invalid("mask artifact destination has an invalid name".into())
        })?;
    let staging = parent.join(format!(".{name}.partial-{}", std::process::id()));
    if staging.exists() {
        return Err(MaskError::Invalid(
            "mask artifact staging destination already exists".into(),
        ));
    }
    Ok(staging)
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
            "isometric-mask-{name}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        let _ = fs::remove_dir_all(&root);
        root
    }

    fn descriptor(role: ArtifactRole) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_id: "synthetic-mask".into(),
            role,
            reference: ReferenceRegistration {
                bundle_id: "synthetic-reference".into(),
                manifest_sha256: "1".repeat(64),
                region_id: "synthetic-region".into(),
                width_px: 3,
                height_px: 2,
                grid_sha256: "2".repeat(64),
            },
            producer: ProducerIdentity {
                name: "isometric-mask-test".into(),
                version: "fixture-v1".into(),
            },
        }
    }

    fn pixel(class: SemanticClass, instance_id: u32) -> MaskPixel {
        MaskPixel {
            class,
            confidence: 240,
            instance_id,
            evidence: EvidenceFlags::GEOMETRY.union(EvidenceFlags::GEOGRAPHIC_PRIOR),
        }
    }

    fn evidence_pixels() -> Vec<MaskPixel> {
        vec![
            pixel(SemanticClass::BuildingRoof, 1),
            pixel(SemanticClass::BuildingRoof, 1),
            pixel(SemanticClass::Road, 2),
            pixel(SemanticClass::Car, 3),
            pixel(SemanticClass::SourceArtifact, 4),
            MaskPixel {
                class: SemanticClass::Unknown,
                confidence: 0,
                instance_id: 0,
                evidence: EvidenceFlags::NONE,
            },
        ]
    }

    fn create_fixture(name: &str, role: ArtifactRole, pixels: Vec<MaskPixel>) -> PathBuf {
        let root = fixture_root(name);
        write_artifact(&root, descriptor(role), pixels).expect("write mask fixture");
        root
    }

    fn load_manifest(root: &Path) -> MaskManifest {
        read_manifest(&root.join(MANIFEST_FILENAME)).expect("read mask manifest")
    }

    fn rewrite_manifest(root: &Path, manifest: &MaskManifest) {
        fs::write(
            root.join(MANIFEST_FILENAME),
            serde_json::to_vec_pretty(manifest).expect("serialize malformed fixture"),
        )
        .expect("rewrite manifest");
    }

    #[test]
    fn ontology_has_stable_complete_binary_order() {
        for (expected, class) in (0_u8..).zip(SemanticClass::ALL) {
            assert_eq!(class as u8, expected);
            assert_eq!(
                SemanticClass::try_from(expected).expect("known class"),
                class
            );
        }
        assert!(
            SemanticClass::try_from(u8::try_from(CLASS_COUNT).expect("class count fits")).is_err()
        );
    }

    #[test]
    fn evidence_flags_reject_reserved_bits() {
        let flags = EvidenceFlags::DETECTOR.union(EvidenceFlags::PROMPT_SEGMENTATION);
        assert!(flags.contains(EvidenceFlags::DETECTOR));
        assert_eq!(
            EvidenceFlags::from_bits(flags.bits()).expect("valid"),
            flags
        );
        assert!(EvidenceFlags::from_bits(1 << 15).is_err());
    }

    #[test]
    fn registration_is_derived_from_the_canonical_reference_camera_and_grid() {
        use isometric_reference::{
            CameraSpec, CaptureSpec, LayerKind, LayerRecord, LightingSpec, ReferenceManifest,
            TileSpec,
        };

        let tile = TileSpec {
            region_id: "synthetic-region".into(),
            column: 0,
            row: 0,
            core_width_px: 2,
            core_height_px: 2,
            guard_px: 1,
            millimeters_per_pixel: 250,
            center_longitude_e7: -1_221_700_000,
            center_latitude_e7: 374_280_000,
        };
        let layers = [
            LayerKind::Color,
            LayerKind::Whitebox,
            LayerKind::LinearDepth,
            LayerKind::ViewNormal,
            LayerKind::FixedShadow,
            LayerKind::Coverage,
        ]
        .into_iter()
        .map(|kind| LayerRecord {
            kind,
            path: kind.filename().into(),
            encoding: kind.encoding().into(),
            width_px: 4,
            height_px: 4,
            byte_length: 64,
            sha256: "3".repeat(64),
        })
        .collect();
        let mut manifest = ReferenceManifest {
            schema: isometric_reference::MANIFEST_SCHEMA.into(),
            bundle_id: "synthetic-reference".into(),
            tile,
            camera: CameraSpec {
                projection: "orthographic".into(),
                azimuth_millidegrees: 45_000,
                elevation_millidegrees: 35_000,
                target_altitude_mm: 0,
                near_mm: 1,
                far_mm: 10_000,
                orthographic_width_mm: 1_000,
                orthographic_height_mm: 1_000,
                camera_distance_mm: 5_000,
            },
            lighting: LightingSpec {
                sun_azimuth_millidegrees: 315_000,
                sun_elevation_millidegrees: 42_000,
            },
            capture: CaptureSpec {
                renderer: "synthetic".into(),
                renderer_version: "fixture-v1".into(),
                provider: "google-photorealistic-3d-tiles".into(),
                source_epoch: "2026-08-18".into(),
                complete: true,
                attributions: vec!["Synthetic test attribution".into()],
            },
            core_coverage_basis_points: 10_000,
            layers,
        };

        let first = ReferenceRegistration::from_reference_manifest(&manifest)
            .expect("derive first registration");
        manifest.camera.azimuth_millidegrees += 1;
        let second = ReferenceRegistration::from_reference_manifest(&manifest)
            .expect("derive changed registration");

        assert_eq!(first.width_px, 4);
        assert_eq!(first.height_px, 4);
        assert_ne!(first.manifest_sha256, second.manifest_sha256);
        assert_ne!(first.grid_sha256, second.grid_sha256);
    }

    #[test]
    fn evidence_artifact_round_trips_deterministically() {
        let first = create_fixture("round-trip-a", ArtifactRole::Evidence, evidence_pixels());
        let second = create_fixture("round-trip-b", ArtifactRole::Evidence, evidence_pixels());
        let first_manifest = load_manifest(&first);
        let second_manifest = load_manifest(&second);
        let first_report = validate_artifact(&first, &first_manifest).expect("validate first");
        let second_report = validate_artifact(&second, &second_manifest).expect("validate second");

        assert_eq!(first_manifest, second_manifest);
        assert_eq!(first_report, second_report);
        assert_eq!(first_report.pixel_count, 6);
        assert_eq!(first_report.unknown_pixels, 1);
        assert_eq!(first_report.transient_pixels, 1);
        assert_eq!(first_report.instance_count, 4);
        assert_eq!(
            fs::read(first.join(MASK_FILENAME)).expect("first bytes"),
            fs::read(second.join(MASK_FILENAME)).expect("second bytes")
        );
    }

    #[test]
    fn persistent_artifact_rejects_every_transient_and_source_artifact() {
        for class in SemanticClass::ALL
            .into_iter()
            .filter(|class| class.is_transient() || *class == SemanticClass::SourceArtifact)
        {
            let root = fixture_root(&format!("persistent-reject-{}", class as u8));
            let pixels = vec![pixel(class, 1); 6];
            let error = write_artifact(&root, descriptor(ArtifactRole::Persistent), pixels)
                .expect_err("persistent artifact must reject class");
            assert!(error.to_string().contains("forbidden"));
            assert!(!root.exists());
        }
    }

    #[test]
    fn persistent_artifact_accepts_explicit_unknown_without_an_instance() {
        let pixels = vec![
            MaskPixel {
                class: SemanticClass::Unknown,
                confidence: 0,
                instance_id: 0,
                evidence: EvidenceFlags::NONE,
            };
            6
        ];
        let root = create_fixture("persistent-unknown", ArtifactRole::Persistent, pixels);
        let report = validate_artifact(&root, &load_manifest(&root)).expect("validate persistent");
        assert_eq!(report.unknown_pixels, 6);
        assert_eq!(report.transient_pixels, 0);
    }

    #[test]
    fn validator_rejects_corrupt_header_hash_and_length() {
        let root = create_fixture("corrupt-payload", ArtifactRole::Evidence, evidence_pixels());
        let manifest = load_manifest(&root);
        let mask_path = root.join(MASK_FILENAME);
        let original = fs::read(&mask_path).expect("mask bytes");

        let mut corrupt = original.clone();
        corrupt[0] ^= 0xff;
        fs::write(&mask_path, &corrupt).expect("write corrupt header");
        assert!(validate_artifact(&root, &manifest).is_err());

        fs::write(&mask_path, &original).expect("restore mask");
        let mut wrong_hash = manifest.clone();
        wrong_hash.mask.sha256 = "0".repeat(64);
        assert!(validate_artifact(&root, &wrong_hash).is_err());

        fs::write(&mask_path, &original[..original.len() - 1]).expect("truncate mask");
        assert!(validate_artifact(&root, &manifest).is_err());
    }

    #[test]
    fn validator_rejects_class_order_counts_registration_and_bounds() {
        let root = create_fixture(
            "manifest-contract",
            ArtifactRole::Evidence,
            evidence_pixels(),
        );
        let manifest = load_manifest(&root);

        let mut wrong_order = manifest.clone();
        wrong_order.class_counts.swap(0, 1);
        assert!(validate_artifact(&root, &wrong_order).is_err());

        let mut wrong_count = manifest.clone();
        wrong_count.class_counts[0].pixels += 1;
        wrong_count.class_counts[SemanticClass::BuildingRoof.index()].pixels -= 1;
        assert!(validate_artifact(&root, &wrong_count).is_err());

        let mut wrong_registration = manifest.clone();
        wrong_registration.reference.width_px += 1;
        assert!(validate_artifact(&root, &wrong_registration).is_err());

        let mut excessive_instance = manifest.clone();
        excessive_instance.max_instance_id = MAX_INSTANCE_ID + 1;
        assert!(validate_artifact(&root, &excessive_instance).is_err());

        rewrite_manifest(&root, &wrong_order);
        assert!(canonical_manifest_json(&wrong_order).is_err());
    }

    #[test]
    fn validator_rejects_unknown_class_reserved_evidence_and_mixed_instance() {
        let root = create_fixture("corrupt-record", ArtifactRole::Evidence, evidence_pixels());
        let manifest = load_manifest(&root);
        let path = root.join(MASK_FILENAME);
        let original = fs::read(&path).expect("read mask");

        let mut unknown_class = original.clone();
        unknown_class[HEADER_BYTES] = u8::try_from(CLASS_COUNT).expect("class count fits");
        fs::write(&path, unknown_class).expect("write unknown class");
        assert!(validate_artifact(&root, &manifest).is_err());

        let mut reserved_evidence = original.clone();
        reserved_evidence[HEADER_BYTES + 3] = 0x80;
        fs::write(&path, reserved_evidence).expect("write reserved evidence");
        assert!(validate_artifact(&root, &manifest).is_err());

        let mut mixed_instance = original;
        let second_record = HEADER_BYTES + usize::from(RECORD_BYTES);
        mixed_instance[second_record] = SemanticClass::Road as u8;
        fs::write(&path, mixed_instance).expect("write mixed instance");
        assert!(validate_artifact(&root, &manifest).is_err());
    }

    #[test]
    fn writer_rejects_wrong_pixel_count_and_existing_destination() {
        let root = fixture_root("wrong-count");
        let error = write_artifact(
            &root,
            descriptor(ArtifactRole::Evidence),
            evidence_pixels().into_iter().take(5).collect::<Vec<_>>(),
        )
        .expect_err("wrong pixel count must fail");
        assert!(error.to_string().contains("expected 6"));

        let existing = create_fixture("existing", ArtifactRole::Evidence, evidence_pixels());
        assert!(
            write_artifact(
                &existing,
                descriptor(ArtifactRole::Evidence),
                evidence_pixels()
            )
            .is_err()
        );
    }

    #[test]
    fn manifest_reader_rejects_symlink() {
        let root = create_fixture(
            "manifest-symlink",
            ArtifactRole::Evidence,
            evidence_pixels(),
        );
        let link = root.join("linked-manifest.json");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join(MANIFEST_FILENAME), &link)
                .expect("create manifest symlink");
            assert!(read_manifest(&link).is_err());
        }
    }
}

//! Bounded, content-addressed acquisition of approved prototype sources.

use std::{
    error::Error,
    fmt::{Display, Formatter, Write as _},
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Deserialize;
use sha2::{Digest, Sha256};

const LOCK_SCHEMA: &str = "isometric-source-lock/v1";
const COPY_BUFFER_BYTES: usize = 64 * 1024;
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const HTTP_RESPONSE_TIMEOUT: Duration = Duration::from_secs(180);

/// A validated source lock used by the acquisition pipeline.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct SourceLock {
    /// Schema identifier.
    pub schema: String,
    /// Stable geographic region identifier.
    pub region_id: String,
    /// Locked source artifacts.
    pub sources: Vec<SourceRecord>,
    /// Whether Google-derived content is permitted.
    pub google_content_permitted: bool,
}

/// One approved immutable source artifact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct SourceRecord {
    /// Stable source identifier.
    pub id: String,
    /// Semantic source family.
    pub kind: String,
    /// Compiler role of this artifact.
    pub role: String,
    /// Upstream release, snapshot, or item identifier.
    pub release: String,
    /// Date or timestamp represented by the source.
    pub source_date: String,
    /// UTC date on which the artifact was acquired.
    pub acquired_at: String,
    /// Retrieval contract.
    pub acquisition: Acquisition,
    /// Exact expected byte length.
    pub size_bytes: u64,
    /// Exact expected SHA-256 in lowercase hexadecimal.
    pub sha256: String,
    /// License identifier or public-domain statement.
    pub license: String,
    /// Required public attribution.
    pub attribution: String,
    /// Human-readable upstream metadata URL.
    pub metadata_url: String,
    /// Whether the source-rights record approved acquisition.
    pub approved: bool,
    /// Whether untransformed source bytes may enter final render output.
    pub raw_content_in_final_output: bool,
}

/// Supported deterministic artifact acquisition methods.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "method", rename_all = "kebab-case")]
pub enum Acquisition {
    /// Retrieve an exact response over HTTPS.
    Https {
        /// Full request URL including its locked query.
        url: String,
        /// Stable output filename.
        filename: String,
    },
    /// Verify and import a repository-local artifact.
    Local {
        /// Path relative to the source lock.
        path: String,
        /// Stable output filename.
        filename: String,
    },
}

/// Result of synchronizing one artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncedArtifact {
    /// Source record identifier.
    pub id: String,
    /// Content-addressed cache path.
    pub path: PathBuf,
    /// Whether an already verified cache entry was reused.
    pub reused: bool,
}

/// Fail-closed source acquisition error.
#[derive(Debug)]
pub enum SourceError {
    /// The lock or a record violates the contract.
    Invalid(String),
    /// Local I/O failed.
    Io(io::Error),
    /// JSON decoding failed.
    Json(serde_json::Error),
    /// Network retrieval failed.
    Http(Box<ureq::Error>),
}

impl Display for SourceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::Io(error) => write!(formatter, "source I/O failed: {error}"),
            Self::Json(error) => write!(formatter, "source lock JSON failed: {error}"),
            Self::Http(error) => write!(formatter, "source HTTP request failed: {error}"),
        }
    }
}

impl Error for SourceError {}

impl From<io::Error> for SourceError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for SourceError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

/// Read and validate a source lock without acquiring data.
///
/// # Errors
///
/// Returns an error when the lock cannot be read, decoded, or validated.
pub fn read_lock(path: &Path) -> Result<SourceLock, SourceError> {
    let lock: SourceLock = serde_json::from_reader(BufReader::new(File::open(path)?))?;
    validate_lock(&lock)?;
    Ok(lock)
}

/// Synchronize every approved source into a bounded content-addressed cache.
///
/// # Errors
///
/// Returns an error when a record is invalid, acquisition fails, the byte
/// length differs, or the SHA-256 does not match.
pub fn sync(lock_path: &Path, cache_root: &Path) -> Result<Vec<SyncedArtifact>, SourceError> {
    let lock = read_lock(lock_path)?;
    let lock_root = lock_path.parent().unwrap_or_else(|| Path::new("."));
    let mut outputs = Vec::with_capacity(lock.sources.len());

    for source in &lock.sources {
        outputs.push(sync_one(source, lock_root, cache_root)?);
    }

    Ok(outputs)
}

fn validate_lock(lock: &SourceLock) -> Result<(), SourceError> {
    if lock.schema != LOCK_SCHEMA {
        return Err(SourceError::Invalid(format!(
            "source lock schema must be {LOCK_SCHEMA}"
        )));
    }
    if lock.region_id != "stanford-hero-v1" {
        return Err(SourceError::Invalid(
            "source lock region must be stanford-hero-v1".into(),
        ));
    }
    if lock.google_content_permitted {
        return Err(SourceError::Invalid(
            "Google-derived source acquisition is disabled".into(),
        ));
    }
    if lock.sources.is_empty() {
        return Err(SourceError::Invalid(
            "source lock must contain at least one approved artifact".into(),
        ));
    }

    let mut previous_id: Option<&str> = None;
    for source in &lock.sources {
        if !source.approved {
            return Err(SourceError::Invalid(format!(
                "source {} is not approved",
                source.id
            )));
        }
        if source.raw_content_in_final_output {
            return Err(SourceError::Invalid(format!(
                "source {} permits raw content in final output",
                source.id
            )));
        }
        if source.id.is_empty() || source.kind.is_empty() || source.role.is_empty() {
            return Err(SourceError::Invalid(
                "source id, kind, and role must be non-empty".into(),
            ));
        }
        if previous_id.is_some_and(|previous| previous >= source.id.as_str()) {
            return Err(SourceError::Invalid(
                "source records must be uniquely sorted by id".into(),
            ));
        }
        previous_id = Some(&source.id);
        if source.size_bytes == 0 {
            return Err(SourceError::Invalid(format!(
                "source {} has an invalid byte length",
                source.id
            )));
        }
        if !is_sha256(&source.sha256) {
            return Err(SourceError::Invalid(format!(
                "source {} has an invalid SHA-256",
                source.id
            )));
        }
        if source.release.is_empty()
            || source.source_date.is_empty()
            || source.acquired_at.is_empty()
            || source.license.is_empty()
            || source.attribution.is_empty()
            || source.metadata_url.is_empty()
        {
            return Err(SourceError::Invalid(format!(
                "source {} lacks release, date, license, attribution, or metadata",
                source.id
            )));
        }
        validate_acquisition(source)?;
    }
    Ok(())
}

fn validate_acquisition(source: &SourceRecord) -> Result<(), SourceError> {
    let filename = match &source.acquisition {
        Acquisition::Https { url, filename } => {
            if !url.starts_with("https://") {
                return Err(SourceError::Invalid(format!(
                    "source {} must use HTTPS",
                    source.id
                )));
            }
            if url.contains("google.") || url.contains("googleapis.") {
                return Err(SourceError::Invalid(format!(
                    "source {} attempts prohibited Google retrieval",
                    source.id
                )));
            }
            filename
        }
        Acquisition::Local { path, filename } => {
            let local = Path::new(path);
            if local.is_absolute() || path.split('/').any(|part| part == "..") {
                return Err(SourceError::Invalid(format!(
                    "source {} has an unsafe local path",
                    source.id
                )));
            }
            filename
        }
    };

    if filename.is_empty() || filename.contains('/') || filename.contains('\\') {
        return Err(SourceError::Invalid(format!(
            "source {} has an unsafe filename",
            source.id
        )));
    }
    Ok(())
}

fn sync_one(
    source: &SourceRecord,
    lock_root: &Path,
    cache_root: &Path,
) -> Result<SyncedArtifact, SourceError> {
    let filename = match &source.acquisition {
        Acquisition::Https { filename, .. } | Acquisition::Local { filename, .. } => filename,
    };
    let destination = cache_root
        .join("sha256")
        .join(&source.sha256[..2])
        .join(&source.sha256)
        .join(filename);

    if destination.exists() {
        verify_file(&destination, source)?;
        return Ok(SyncedArtifact {
            id: source.id.clone(),
            path: destination,
            reused: true,
        });
    }

    let parent = destination
        .parent()
        .ok_or_else(|| SourceError::Invalid("cache destination lacks a parent".into()))?;
    fs::create_dir_all(parent)?;
    let partial = parent.join(format!(".{filename}.partial-{}", std::process::id()));
    let result = acquire_to_partial(source, lock_root, &partial);
    if let Err(error) = result {
        let _ = fs::remove_file(&partial);
        return Err(error);
    }
    verify_file(&partial, source)?;
    fs::rename(&partial, &destination)?;

    Ok(SyncedArtifact {
        id: source.id.clone(),
        path: destination,
        reused: false,
    })
}

fn acquire_to_partial(
    source: &SourceRecord,
    lock_root: &Path,
    partial: &Path,
) -> Result<(), SourceError> {
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(partial)?;
    let mut output = BufWriter::new(file);

    match &source.acquisition {
        Acquisition::Https { url, .. } => {
            let config = ureq::Agent::config_builder()
                .timeout_connect(Some(HTTP_CONNECT_TIMEOUT))
                .timeout_recv_response(Some(HTTP_RESPONSE_TIMEOUT))
                .user_agent("isometric-stanford/0.1 source-sync")
                .build();
            let agent: ureq::Agent = config.into();
            let response = agent
                .get(url)
                .call()
                .map_err(|error| SourceError::Http(Box::new(error)))?;
            let mut reader = response.into_body().into_reader();
            copy_bounded(&mut reader, &mut output, source.size_bytes)?;
        }
        Acquisition::Local { path, .. } => {
            let mut input = BufReader::new(File::open(lock_root.join(path))?);
            copy_bounded(&mut input, &mut output, source.size_bytes)?;
        }
    }
    output.flush()?;
    Ok(())
}

fn copy_bounded(
    input: &mut impl Read,
    output: &mut impl Write,
    expected_bytes: u64,
) -> Result<(), SourceError> {
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut copied = 0_u64;
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        copied = copied
            .checked_add(u64::try_from(count).expect("buffer length fits u64"))
            .ok_or_else(|| SourceError::Invalid("source byte count overflowed".into()))?;
        if copied > expected_bytes {
            return Err(SourceError::Invalid(format!(
                "source exceeded locked length of {expected_bytes} bytes"
            )));
        }
        output.write_all(&buffer[..count])?;
    }
    if copied != expected_bytes {
        return Err(SourceError::Invalid(format!(
            "source length {copied} does not match locked length {expected_bytes}"
        )));
    }
    Ok(())
}

fn verify_file(path: &Path, source: &SourceRecord) -> Result<(), SourceError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() != source.size_bytes {
        return Err(SourceError::Invalid(format!(
            "cached source {} has length {}, expected {}",
            source.id,
            metadata.len(),
            source.size_bytes
        )));
    }

    let mut reader = BufReader::new(File::open(path)?);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let actual = encode_hex(&hasher.finalize());
    if actual != source.sha256 {
        return Err(SourceError::Invalid(format!(
            "cached source {} has SHA-256 {actual}, expected {}",
            source.id, source.sha256
        )));
    }
    Ok(())
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::{Acquisition, SourceLock, SourceRecord, read_lock, sync};
    use sha2::{Digest, Sha256};
    use std::fs;

    fn hash(bytes: &[u8]) -> String {
        bytes.iter().fold(String::new(), |mut output, byte| {
            use std::fmt::Write as _;
            write!(&mut output, "{byte:02x}").expect("String write");
            output
        })
    }

    #[test]
    fn rejects_google_and_unsorted_sources() {
        let mut lock = SourceLock {
            schema: "isometric-source-lock/v1".into(),
            region_id: "stanford-hero-v1".into(),
            sources: vec![record("b"), record("a")],
            google_content_permitted: false,
        };
        assert!(super::validate_lock(&lock).is_err());
        lock.sources.sort_by(|left, right| left.id.cmp(&right.id));
        lock.sources[0].acquisition = Acquisition::Https {
            url: "https://maps.googleapis.com/content".into(),
            filename: "fixture".into(),
        };
        assert!(super::validate_lock(&lock).is_err());
        lock.sources[0].acquisition = Acquisition::Local {
            path: "fixture".into(),
            filename: "fixture".into(),
        };
        lock.google_content_permitted = true;
        assert!(super::validate_lock(&lock).is_err());
    }

    #[test]
    fn imports_local_bytes_and_reuses_verified_cache() {
        let root = std::env::temp_dir().join(format!(
            "isometric-source-test-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("input")).expect("create fixture directory");
        let bytes = b"licensed fixture";
        fs::write(root.join("input/source.bin"), bytes).expect("write fixture");
        let digest = Sha256::digest(bytes);
        let lock = format!(
            "{{\"schema\":\"isometric-source-lock/v1\",\"region_id\":\"stanford-hero-v1\",\"sources\":[{{\"id\":\"fixture\",\"kind\":\"test\",\"role\":\"test\",\"release\":\"fixture-v1\",\"source_date\":\"2026-08-17\",\"acquired_at\":\"2026-08-17\",\"acquisition\":{{\"method\":\"local\",\"path\":\"input/source.bin\",\"filename\":\"source.bin\"}},\"size_bytes\":{},\"sha256\":\"{}\",\"license\":\"CC0-1.0\",\"attribution\":\"fixture\",\"metadata_url\":\"https://example.invalid/fixture\",\"approved\":true,\"raw_content_in_final_output\":false}}],\"google_content_permitted\":false}}",
            bytes.len(),
            hash(&digest)
        );
        fs::write(root.join("source.lock.json"), lock).expect("write lock");

        let parsed = read_lock(&root.join("source.lock.json")).expect("valid lock");
        assert_eq!(parsed.sources.len(), 1);
        let first = sync(&root.join("source.lock.json"), &root.join("cache")).expect("first sync");
        let second =
            sync(&root.join("source.lock.json"), &root.join("cache")).expect("second sync");
        assert!(!first[0].reused);
        assert!(second[0].reused);
        assert_eq!(fs::read(&second[0].path).expect("cached bytes"), bytes);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn rejects_wrong_length_without_accepting_partial_artifact() {
        let root = std::env::temp_dir().join(format!(
            "isometric-source-length-test-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("input")).expect("create fixture directory");
        let bytes = b"licensed fixture";
        fs::write(root.join("input/source.bin"), bytes).expect("write fixture");
        let digest = hash(&Sha256::digest(bytes));
        let lock = format!(
            "{{\"schema\":\"isometric-source-lock/v1\",\"region_id\":\"stanford-hero-v1\",\"sources\":[{{\"id\":\"fixture\",\"kind\":\"test\",\"role\":\"test\",\"release\":\"fixture-v1\",\"source_date\":\"2026-08-17\",\"acquired_at\":\"2026-08-17\",\"acquisition\":{{\"method\":\"local\",\"path\":\"input/source.bin\",\"filename\":\"source.bin\"}},\"size_bytes\":{},\"sha256\":\"{}\",\"license\":\"CC0-1.0\",\"attribution\":\"fixture\",\"metadata_url\":\"https://example.invalid/fixture\",\"approved\":true,\"raw_content_in_final_output\":false}}],\"google_content_permitted\":false}}",
            bytes.len() - 1,
            digest
        );
        fs::write(root.join("source.lock.json"), lock).expect("write lock");

        let error = sync(&root.join("source.lock.json"), &root.join("cache"))
            .expect_err("wrong length must fail");
        assert!(error.to_string().contains("exceeded locked length"));
        let accepted = root
            .join("cache/sha256")
            .join(&digest[..2])
            .join(&digest)
            .join("source.bin");
        assert!(!accepted.exists());
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn rejects_corrupted_existing_cache_entry() {
        let root = std::env::temp_dir().join(format!(
            "isometric-source-corruption-test-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("input")).expect("create fixture directory");
        let bytes = b"licensed fixture";
        fs::write(root.join("input/source.bin"), bytes).expect("write fixture");
        let digest = hash(&Sha256::digest(bytes));
        let lock = format!(
            "{{\"schema\":\"isometric-source-lock/v1\",\"region_id\":\"stanford-hero-v1\",\"sources\":[{{\"id\":\"fixture\",\"kind\":\"test\",\"role\":\"test\",\"release\":\"fixture-v1\",\"source_date\":\"2026-08-17\",\"acquired_at\":\"2026-08-17\",\"acquisition\":{{\"method\":\"local\",\"path\":\"input/source.bin\",\"filename\":\"source.bin\"}},\"size_bytes\":{},\"sha256\":\"{}\",\"license\":\"CC0-1.0\",\"attribution\":\"fixture\",\"metadata_url\":\"https://example.invalid/fixture\",\"approved\":true,\"raw_content_in_final_output\":false}}],\"google_content_permitted\":false}}",
            bytes.len(),
            digest
        );
        fs::write(root.join("source.lock.json"), lock).expect("write lock");

        let first = sync(&root.join("source.lock.json"), &root.join("cache")).expect("first sync");
        fs::write(&first[0].path, b"corruptd fixture").expect("corrupt cache");
        let error = sync(&root.join("source.lock.json"), &root.join("cache"))
            .expect_err("corrupt cache must fail");
        assert!(error.to_string().contains("SHA-256"));
        fs::remove_dir_all(root).expect("remove test directory");
    }

    fn record(id: &str) -> SourceRecord {
        SourceRecord {
            id: id.into(),
            kind: "test".into(),
            role: "test".into(),
            release: "fixture-v1".into(),
            source_date: "2026-08-17".into(),
            acquired_at: "2026-08-17".into(),
            acquisition: Acquisition::Local {
                path: "fixture".into(),
                filename: "fixture".into(),
            },
            size_bytes: 1,
            sha256: "0".repeat(64),
            license: "CC0-1.0".into(),
            attribution: "fixture".into(),
            metadata_url: "https://example.invalid/fixture".into(),
            approved: true,
            raw_content_in_final_output: false,
        }
    }
}

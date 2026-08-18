//! Bounded, content-addressed acquisition of approved prototype sources.

use std::{
    error::Error,
    fmt::{Display, Formatter, Write as _},
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use serde::Deserialize;
use sha2::{Digest, Sha256};

const LOCK_SCHEMA: &str = "isometric-source-lock/v1";
const COPY_BUFFER_BYTES: usize = 64 * 1024;
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
// ureq keeps the receive-response timer active while the body is read. Keep
// both receive limits equal so a large valid body receives the documented
// bounded window instead of inheriting a shorter header-only assumption.
const HTTP_RESPONSE_TIMEOUT: Duration = Duration::from_secs(300);
const HTTP_BODY_TIMEOUT: Duration = Duration::from_secs(300);
const HTTP_MAX_ATTEMPTS: u8 = 3;
const HTTP_RETRY_BACKOFF: Duration = Duration::from_millis(250);

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
        /// Optional immutable entity tag required for guarded range continuation.
        #[serde(default)]
        etag: Option<String>,
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
    /// Acquisition attempts made in this invocation, or zero for a cache hit.
    pub attempts: u8,
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
    /// One approved source could not be acquired safely.
    Acquisition {
        /// Stable source identifier, never its possibly sensitive URL.
        source_id: String,
        /// Bounded acquisition stage that failed.
        stage: &'static str,
        /// Attempts made before returning the failure.
        attempts: u8,
        /// Sanitized upstream or stream failure.
        detail: String,
    },
}

impl Display for SourceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::Io(error) => write!(formatter, "source I/O failed: {error}"),
            Self::Json(error) => write!(formatter, "source lock JSON failed: {error}"),
            Self::Acquisition {
                source_id,
                stage,
                attempts,
                detail,
            } => write!(
                formatter,
                "source {source_id} acquisition failed during {stage} after {attempts} attempt(s): {detail}"
            ),
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
    sync_selected(lock_path, cache_root, &[])
}

/// Synchronize selected approved artifacts, or every artifact when `ids` is empty.
///
/// The complete lock is validated before selection, so a malformed or
/// prohibited unselected record still fails closed.
///
/// # Errors
///
/// Returns an error for an invalid lock, an unknown requested ID, acquisition
/// failure, wrong byte length, or SHA-256 mismatch.
pub fn sync_selected(
    lock_path: &Path,
    cache_root: &Path,
    ids: &[&str],
) -> Result<Vec<SyncedArtifact>, SourceError> {
    let lock = read_lock(lock_path)?;
    let lock_root = lock_path.parent().unwrap_or_else(|| Path::new("."));
    let selected = if ids.is_empty() {
        lock.sources.iter().collect::<Vec<_>>()
    } else {
        let requested = ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if requested.len() != ids.len()
            || requested
                .iter()
                .any(|id| !lock.sources.iter().any(|source| source.id == *id))
        {
            return Err(SourceError::Invalid(
                "selected source IDs must be unique and present in the lock".into(),
            ));
        }
        lock.sources
            .iter()
            .filter(|source| requested.contains(source.id.as_str()))
            .collect()
    };
    let mut outputs = Vec::with_capacity(selected.len());

    for source in selected {
        outputs.push(sync_one(
            source,
            lock_root,
            cache_root,
            RetryPolicy::production(),
        )?);
    }

    Ok(outputs)
}

#[derive(Clone, Copy)]
struct RetryPolicy {
    max_attempts: u8,
    backoff: Duration,
    connect_timeout: Duration,
    response_timeout: Duration,
    body_timeout: Duration,
}

impl RetryPolicy {
    const fn production() -> Self {
        Self {
            max_attempts: HTTP_MAX_ATTEMPTS,
            backoff: HTTP_RETRY_BACKOFF,
            connect_timeout: HTTP_CONNECT_TIMEOUT,
            response_timeout: HTTP_RESPONSE_TIMEOUT,
            body_timeout: HTTP_BODY_TIMEOUT,
        }
    }
}

struct AttemptFailure {
    stage: &'static str,
    detail: String,
    retryable: bool,
}

enum CopyFailure {
    Io(io::Error),
    Invalid(String),
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
        Acquisition::Https {
            url,
            etag,
            filename,
        } => {
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
            if etag.as_ref().is_some_and(|etag| {
                etag.len() < 3
                    || !etag.starts_with('"')
                    || !etag.ends_with('"')
                    || !etag.bytes().all(|byte| byte.is_ascii_graphic())
            }) {
                return Err(SourceError::Invalid(format!(
                    "source {} has an invalid HTTPS entity tag",
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
    retry_policy: RetryPolicy,
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
            attempts: 0,
        });
    }

    let parent = destination
        .parent()
        .ok_or_else(|| SourceError::Invalid("cache destination lacks a parent".into()))?;
    fs::create_dir_all(parent)?;
    let partial = parent.join(format!(".{filename}.partial-{}", std::process::id()));
    let attempts = match acquire_to_partial(source, lock_root, &partial, retry_policy) {
        Ok(attempts) => attempts,
        Err(error) => {
            let _ = fs::remove_file(&partial);
            return Err(error);
        }
    };
    if let Err(error) = verify_file(&partial, source) {
        let _ = fs::remove_file(&partial);
        return Err(error);
    }
    fs::rename(&partial, &destination)?;

    Ok(SyncedArtifact {
        id: source.id.clone(),
        path: destination,
        reused: false,
        attempts,
    })
}

fn acquire_to_partial(
    source: &SourceRecord,
    lock_root: &Path,
    partial: &Path,
    retry_policy: RetryPolicy,
) -> Result<u8, SourceError> {
    match &source.acquisition {
        Acquisition::Https { url, etag, .. } => {
            acquire_https(source, url, etag.as_deref(), partial, retry_policy)
        }
        Acquisition::Local { path, .. } => {
            let file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(partial)
                .map_err(|error| acquisition_error(source, "partial file creation", 1, error))?;
            let mut output = BufWriter::new(file);
            let mut input = BufReader::new(
                File::open(lock_root.join(path))
                    .map_err(|error| acquisition_error(source, "local source open", 1, error))?,
            );
            copy_bounded(&mut input, &mut output, source.size_bytes)
                .map_err(|error| copy_failure_to_acquisition_error(source, error))?;
            output
                .flush()
                .map_err(|error| acquisition_error(source, "partial file flush", 1, error))?;
            Ok(1)
        }
    }
}

fn acquire_https(
    source: &SourceRecord,
    url: &str,
    etag: Option<&str>,
    partial: &Path,
    retry_policy: RetryPolicy,
) -> Result<u8, SourceError> {
    debug_assert!(retry_policy.max_attempts > 0);
    let _ = fs::remove_file(partial);
    for attempt in 1..=retry_policy.max_attempts {
        let offset = fs::metadata(partial).map_or(0, |metadata| metadata.len());
        if offset > source.size_bytes {
            return Err(acquisition_error(
                source,
                "partial file length",
                attempt,
                "partial source exceeds locked length",
            ));
        }
        match acquire_https_once(source, url, etag, partial, offset, retry_policy) {
            Ok(()) => return Ok(attempt),
            Err(failure) => {
                if fs::metadata(partial).is_ok_and(|metadata| metadata.len() == source.size_bytes) {
                    return Ok(attempt);
                }
                if !failure.retryable || attempt == retry_policy.max_attempts {
                    return Err(SourceError::Acquisition {
                        source_id: source.id.clone(),
                        stage: failure.stage,
                        attempts: attempt,
                        detail: failure.detail,
                    });
                }
                if etag.is_none() {
                    let _ = fs::remove_file(partial);
                }
                let multiplier = 1_u32 << u32::from(attempt - 1);
                thread::sleep(retry_policy.backoff.saturating_mul(multiplier));
            }
        }
    }
    unreachable!("a positive bounded attempt loop returns")
}

fn acquire_https_once(
    source: &SourceRecord,
    url: &str,
    etag: Option<&str>,
    partial: &Path,
    offset: u64,
    retry_policy: RetryPolicy,
) -> Result<(), AttemptFailure> {
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(retry_policy.connect_timeout))
        .timeout_recv_response(Some(retry_policy.response_timeout))
        .timeout_recv_body(Some(retry_policy.body_timeout))
        .user_agent("isometric-stanford/0.1 source-sync")
        .build();
    let agent: ureq::Agent = config.into();
    let mut request = agent.get(url);
    if offset > 0 {
        let locked_etag = etag.ok_or_else(|| AttemptFailure {
            stage: "range request",
            detail: "partial bytes cannot continue without a locked entity tag".into(),
            retryable: false,
        })?;
        request = request
            .header("Range", format!("bytes={offset}-"))
            .header("If-Range", locked_etag);
    }
    let response = request.call().map_err(|error| AttemptFailure {
        stage: "response headers",
        detail: http_error_detail(&error),
        retryable: retryable_http_error(&error),
    })?;
    validate_http_response(&response, source, etag, offset)?;
    let mut options = OpenOptions::new();
    options.write(true);
    if offset == 0 {
        options.create_new(true);
    } else {
        options.append(true);
    }
    let mut output = options.open(partial).map_err(|error| AttemptFailure {
        stage: "partial file creation",
        detail: error.to_string(),
        retryable: false,
    })?;
    let mut reader = response.into_body().into_reader();
    let remaining = source.size_bytes - offset;
    copy_bounded(&mut reader, &mut output, remaining).map_err(|error| match error {
        CopyFailure::Io(error) => AttemptFailure {
            stage: "response body",
            detail: io_error_detail(&error),
            retryable: retryable_io_error(&error),
        },
        CopyFailure::Invalid(detail) => AttemptFailure {
            stage: "response body length",
            detail,
            retryable: false,
        },
    })?;
    output.flush().map_err(|error| AttemptFailure {
        stage: "partial file flush",
        detail: error.to_string(),
        retryable: false,
    })?;
    Ok(())
}

fn validate_http_response(
    response: &ureq::http::Response<ureq::Body>,
    source: &SourceRecord,
    etag: Option<&str>,
    offset: u64,
) -> Result<(), AttemptFailure> {
    if let Some(expected) = etag {
        let actual = response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok());
        if actual != Some(expected) {
            return Err(AttemptFailure {
                stage: "response entity tag",
                detail: "response does not match the locked entity tag".into(),
                retryable: false,
            });
        }
    }

    let status = response.status().as_u16();
    if offset == 0 {
        if status != 200 {
            return Err(AttemptFailure {
                stage: "response status",
                detail: format!("expected HTTP 200, received {status}"),
                retryable: false,
            });
        }
        return Ok(());
    }

    if status != 206 {
        return Err(AttemptFailure {
            stage: "range response status",
            detail: format!("expected HTTP 206, received {status}"),
            retryable: false,
        });
    }
    let expected_range = format!(
        "bytes {offset}-{}/{}",
        source.size_bytes - 1,
        source.size_bytes
    );
    let actual_range = response
        .headers()
        .get("content-range")
        .and_then(|value| value.to_str().ok());
    if actual_range != Some(expected_range.as_str()) {
        return Err(AttemptFailure {
            stage: "range response bounds",
            detail: "response does not match the locked continuation bounds".into(),
            retryable: false,
        });
    }
    Ok(())
}

fn copy_bounded(
    input: &mut impl Read,
    output: &mut impl Write,
    expected_bytes: u64,
) -> Result<(), CopyFailure> {
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut copied = 0_u64;
    loop {
        let count = input.read(&mut buffer).map_err(CopyFailure::Io)?;
        if count == 0 {
            break;
        }
        copied = copied
            .checked_add(u64::try_from(count).expect("buffer length fits u64"))
            .ok_or_else(|| CopyFailure::Invalid("source byte count overflowed".into()))?;
        if copied > expected_bytes {
            return Err(CopyFailure::Invalid(format!(
                "source exceeded locked length of {expected_bytes} bytes"
            )));
        }
        output
            .write_all(&buffer[..count])
            .map_err(CopyFailure::Io)?;
    }
    if copied != expected_bytes {
        return Err(CopyFailure::Invalid(format!(
            "source length {copied} does not match locked length {expected_bytes}"
        )));
    }
    Ok(())
}

fn copy_failure_to_acquisition_error(source: &SourceRecord, error: CopyFailure) -> SourceError {
    match error {
        CopyFailure::Io(error) => acquisition_error(source, "local source stream", 1, error),
        CopyFailure::Invalid(detail) => acquisition_error(source, "local source length", 1, detail),
    }
}

fn acquisition_error(
    source: &SourceRecord,
    stage: &'static str,
    attempts: u8,
    detail: impl Display,
) -> SourceError {
    SourceError::Acquisition {
        source_id: source.id.clone(),
        stage,
        attempts,
        detail: detail.to_string(),
    }
}

fn retryable_http_error(error: &ureq::Error) -> bool {
    match error {
        ureq::Error::StatusCode(code) => {
            matches!(*code, 408 | 425 | 429 | 500 | 502 | 503 | 504)
        }
        ureq::Error::Io(error) => retryable_io_error(error),
        ureq::Error::Timeout(_)
        | ureq::Error::HostNotFound
        | ureq::Error::ConnectionFailed
        | ureq::Error::BodyStalled => true,
        _ => false,
    }
}

fn http_error_detail(error: &ureq::Error) -> String {
    match error {
        ureq::Error::StatusCode(code) => format!("HTTP status {code}"),
        ureq::Error::Timeout(timeout) => format!("timeout: {timeout}"),
        ureq::Error::HostNotFound => "host not found".into(),
        ureq::Error::ConnectionFailed => "connection failed".into(),
        ureq::Error::BodyStalled => "response body stalled".into(),
        ureq::Error::Io(error) => io_error_detail(error),
        _ => "HTTP request rejected".into(),
    }
}

fn io_error_detail(error: &io::Error) -> String {
    if let Some(inner) = error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<ureq::Error>())
    {
        return http_error_detail(inner);
    }
    format!("I/O failure ({:?})", error.kind())
}

fn retryable_io_error(error: &io::Error) -> bool {
    if let Some(inner) = error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<ureq::Error>())
    {
        return retryable_http_error(inner);
    }
    matches!(
        error.kind(),
        io::ErrorKind::TimedOut
            | io::ErrorKind::Interrupted
            | io::ErrorKind::WouldBlock
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::UnexpectedEof
    )
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
    use super::{
        Acquisition, RetryPolicy, SourceLock, SourceRecord, read_lock, sync, sync_one,
        sync_selected,
    };
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        io::{Read as _, Write as _},
        net::TcpListener,
        path::Path,
        thread,
        time::Duration,
    };

    struct ResponseSpec {
        status: u16,
        advertised_bytes: usize,
        body: Vec<u8>,
        body_delay: Duration,
    }

    impl ResponseSpec {
        fn status(status: u16) -> Self {
            Self {
                status,
                advertised_bytes: 0,
                body: Vec::new(),
                body_delay: Duration::ZERO,
            }
        }

        fn body(body: &[u8]) -> Self {
            Self {
                status: 200,
                advertised_bytes: body.len(),
                body: body.to_vec(),
                body_delay: Duration::ZERO,
            }
        }

        fn stalled_body(advertised_bytes: usize) -> Self {
            Self {
                status: 200,
                advertised_bytes,
                body: Vec::new(),
                body_delay: Duration::from_millis(75),
            }
        }
    }

    fn serve(responses: Vec<ResponseSpec>) -> (String, thread::JoinHandle<usize>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture server");
        let address = listener.local_addr().expect("fixture server address");
        let handle = thread::spawn(move || {
            let mut served = 0;
            for response in responses {
                let (mut stream, _) = listener.accept().expect("accept fixture request");
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .expect("set fixture read timeout");
                let mut request = [0_u8; 1024];
                let _ = stream.read(&mut request).expect("read fixture request");
                let reason = match response.status {
                    200 => "OK",
                    404 => "Not Found",
                    503 => "Service Unavailable",
                    _ => "Fixture",
                };
                write!(
                    stream,
                    "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    response.status, reason, response.advertised_bytes
                )
                .expect("write fixture headers");
                thread::sleep(response.body_delay);
                stream
                    .write_all(&response.body)
                    .expect("write fixture body");
                served += 1;
            }
            served
        });
        (format!("http://{address}/source.bin"), handle)
    }

    fn serve_range_resume(
        bytes: &[u8],
        prefix_bytes: usize,
        reported_offset: usize,
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind range fixture server");
        let address = listener.local_addr().expect("range fixture server address");
        let body = bytes.to_vec();
        let handle = thread::spawn(move || {
            let (mut first, _) = listener.accept().expect("accept initial request");
            let mut request = [0_u8; 2048];
            let count = first.read(&mut request).expect("read initial request");
            let request = String::from_utf8_lossy(&request[..count]).to_ascii_lowercase();
            assert!(!request.contains("range:"));
            write!(
                first,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"fixture-v1\"\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .expect("write initial headers");
            first
                .write_all(&body[..prefix_bytes])
                .expect("write initial prefix");
            first.flush().expect("flush initial prefix");
            thread::sleep(Duration::from_millis(75));
            let _ = first.write_all(&body[prefix_bytes..]);

            let (mut second, _) = listener.accept().expect("accept range request");
            let mut second_request = [0_u8; 2048];
            let count = second
                .read(&mut second_request)
                .expect("read range request");
            let request = String::from_utf8_lossy(&second_request[..count]).to_ascii_lowercase();
            assert!(request.contains(&format!("range: bytes={prefix_bytes}-")));
            assert!(request.contains("if-range: \"fixture-v1\""));
            write!(
                second,
                "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nETag: \"fixture-v1\"\r\nConnection: close\r\n\r\n",
                body.len() - prefix_bytes,
                reported_offset,
                body.len() - 1,
                body.len()
            )
            .expect("write range headers");
            let _ = second.write_all(&body[prefix_bytes..]);
        });
        (format!("http://{address}/source.bin"), handle)
    }

    fn test_retry_policy() -> RetryPolicy {
        RetryPolicy {
            max_attempts: 3,
            backoff: Duration::ZERO,
            connect_timeout: Duration::from_secs(1),
            response_timeout: Duration::from_secs(1),
            body_timeout: Duration::from_millis(20),
        }
    }

    fn remote_record(id: &str, url: String, expected: &[u8]) -> SourceRecord {
        SourceRecord {
            id: id.into(),
            kind: "test".into(),
            role: "test".into(),
            release: "fixture-v1".into(),
            source_date: "2026-08-17".into(),
            acquired_at: "2026-08-17".into(),
            acquisition: Acquisition::Https {
                url,
                etag: None,
                filename: "source.bin".into(),
            },
            size_bytes: u64::try_from(expected.len()).expect("fixture length fits u64"),
            sha256: hash(&Sha256::digest(expected)),
            license: "CC0-1.0".into(),
            attribution: "fixture".into(),
            metadata_url: "https://example.invalid/fixture".into(),
            approved: true,
            raw_content_in_final_output: false,
        }
    }

    fn partial_files(root: &Path) -> Vec<String> {
        let mut partials = Vec::new();
        if !root.exists() {
            return partials;
        }
        for entry in fs::read_dir(root).expect("read fixture cache") {
            let entry = entry.expect("fixture cache entry");
            if entry.path().is_dir() {
                partials.extend(partial_files(&entry.path()));
            } else if entry.file_name().to_string_lossy().contains(".partial-") {
                partials.push(entry.path().display().to_string());
            }
        }
        partials
    }

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
            etag: None,
            filename: "fixture".into(),
        };
        assert!(super::validate_lock(&lock).is_err());
        lock.sources[0].acquisition = Acquisition::Local {
            path: "fixture".into(),
            filename: "fixture".into(),
        };
        lock.google_content_permitted = true;
        assert!(super::validate_lock(&lock).is_ok());
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
        assert!(
            sync_selected(
                &root.join("source.lock.json"),
                &root.join("cache"),
                &["missing"]
            )
            .expect_err("unknown selection must fail")
            .to_string()
            .contains("selected source IDs")
        );
        let first = sync(&root.join("source.lock.json"), &root.join("cache")).expect("first sync");
        let second =
            sync(&root.join("source.lock.json"), &root.join("cache")).expect("second sync");
        assert!(!first[0].reused);
        assert_eq!(first[0].attempts, 1);
        assert!(second[0].reused);
        assert_eq!(second[0].attempts, 0);
        assert_eq!(fs::read(&second[0].path).expect("cached bytes"), bytes);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn retries_transient_status_and_records_attempts() {
        let root = std::env::temp_dir().join(format!(
            "isometric-source-retry-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let bytes = b"licensed remote fixture";
        let (url, server) = serve(vec![ResponseSpec::status(503), ResponseSpec::body(bytes)]);
        let source = remote_record("remote-retry", url, bytes);

        let artifact = sync_one(&source, Path::new("."), &root, test_retry_policy())
            .expect("transient status must recover");

        assert_eq!(artifact.attempts, 2);
        assert_eq!(
            fs::read(&artifact.path).expect("read acquired fixture"),
            bytes
        );
        assert_eq!(server.join().expect("join fixture server"), 2);
        assert!(partial_files(&root).is_empty());
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn production_receive_deadlines_share_one_large_artifact_window() {
        let policy = RetryPolicy::production();

        assert_eq!(policy.connect_timeout, Duration::from_secs(30));
        assert_eq!(policy.response_timeout, Duration::from_secs(300));
        assert_eq!(policy.body_timeout, Duration::from_secs(300));
    }

    #[test]
    fn retries_stalled_body_from_a_fresh_partial_file() {
        let root = std::env::temp_dir().join(format!(
            "isometric-source-body-retry-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let bytes = b"licensed remote fixture";
        let (url, server) = serve(vec![
            ResponseSpec::stalled_body(bytes.len()),
            ResponseSpec::body(bytes),
        ]);
        let source = remote_record("remote-body-retry", url, bytes);

        let artifact = sync_one(&source, Path::new("."), &root, test_retry_policy())
            .expect("stalled body must recover");

        assert_eq!(artifact.attempts, 2);
        assert_eq!(
            fs::read(&artifact.path).expect("read acquired fixture"),
            bytes
        );
        assert_eq!(server.join().expect("join fixture server"), 2);
        assert!(partial_files(&root).is_empty());
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn resumes_only_with_locked_etag_and_exact_range_bounds() {
        let root = std::env::temp_dir().join(format!(
            "isometric-source-range-retry-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let bytes = b"licensed immutable remote fixture";
        let prefix_bytes = 11;
        let (url, server) = serve_range_resume(bytes, prefix_bytes, prefix_bytes);
        let mut source = remote_record("remote-range-retry", url, bytes);
        if let Acquisition::Https { etag, .. } = &mut source.acquisition {
            *etag = Some("\"fixture-v1\"".into());
        }

        let artifact = sync_one(&source, Path::new("."), &root, test_retry_policy())
            .expect("locked range continuation must recover");

        assert_eq!(artifact.attempts, 2);
        assert_eq!(
            fs::read(&artifact.path).expect("read range-acquired fixture"),
            bytes
        );
        server.join().expect("join range fixture server");
        assert!(partial_files(&root).is_empty());
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn rejects_range_response_that_does_not_match_locked_offset() {
        let root = std::env::temp_dir().join(format!(
            "isometric-source-invalid-range-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let bytes = b"licensed immutable remote fixture";
        let prefix_bytes = 11;
        let (url, server) = serve_range_resume(bytes, prefix_bytes, prefix_bytes + 1);
        let mut source = remote_record("remote-invalid-range", url, bytes);
        if let Acquisition::Https { etag, .. } = &mut source.acquisition {
            *etag = Some("\"fixture-v1\"".into());
        }

        let error = sync_one(&source, Path::new("."), &root, test_retry_policy())
            .expect_err("mismatched continuation bounds must fail closed");
        let message = error.to_string();

        assert!(message.contains("range response bounds"));
        assert!(message.contains("after 2 attempt(s)"));
        server.join().expect("join range fixture server");
        assert!(partial_files(&root).is_empty());
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn exhausts_only_the_bounded_transient_attempts() {
        let root = std::env::temp_dir().join(format!(
            "isometric-source-exhaustion-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let bytes = b"licensed remote fixture";
        let (url, server) = serve(vec![
            ResponseSpec::status(503),
            ResponseSpec::status(503),
            ResponseSpec::status(503),
        ]);
        let source = remote_record("remote-exhausted", url.clone(), bytes);

        let error = sync_one(&source, Path::new("."), &root, test_retry_policy())
            .expect_err("bounded transient attempts must fail closed");
        let message = error.to_string();

        assert!(message.contains("source remote-exhausted"));
        assert!(message.contains("response headers"));
        assert!(message.contains("after 3 attempt(s)"));
        assert!(!message.contains(&url));
        assert_eq!(server.join().expect("join fixture server"), 3);
        assert!(partial_files(&root).is_empty());
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn permanent_status_and_corrupt_bytes_are_not_retried() {
        let status_root = std::env::temp_dir().join(format!(
            "isometric-source-permanent-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&status_root);
        let expected = b"licensed remote fixture";
        let (status_url, status_server) = serve(vec![ResponseSpec::status(404)]);
        let status_source = remote_record("remote-permanent", status_url, expected);
        let status_error = sync_one(
            &status_source,
            Path::new("."),
            &status_root,
            test_retry_policy(),
        )
        .expect_err("permanent status must fail closed");
        assert!(status_error.to_string().contains("after 1 attempt(s)"));
        assert_eq!(status_server.join().expect("join fixture server"), 1);
        assert!(partial_files(&status_root).is_empty());
        fs::remove_dir_all(status_root).expect("remove status test directory");

        let corrupt_root = std::env::temp_dir().join(format!(
            "isometric-source-corrupt-http-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&corrupt_root);
        let corrupt = b"corruptd remote fixture";
        assert_eq!(corrupt.len(), expected.len());
        let (corrupt_url, corrupt_server) = serve(vec![ResponseSpec::body(corrupt)]);
        let corrupt_source = remote_record("remote-corrupt", corrupt_url, expected);
        let corrupt_error = sync_one(
            &corrupt_source,
            Path::new("."),
            &corrupt_root,
            test_retry_policy(),
        )
        .expect_err("corrupt bytes must fail closed");
        assert!(corrupt_error.to_string().contains("SHA-256"));
        assert_eq!(corrupt_server.join().expect("join fixture server"), 1);
        assert!(partial_files(&corrupt_root).is_empty());
        fs::remove_dir_all(corrupt_root).expect("remove corrupt test directory");

        let length_root = std::env::temp_dir().join(format!(
            "isometric-source-short-http-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&length_root);
        let (length_url, length_server) = serve(vec![ResponseSpec::body(b"short")]);
        let length_source = remote_record("remote-short", length_url, expected);
        let length_error = sync_one(
            &length_source,
            Path::new("."),
            &length_root,
            test_retry_policy(),
        )
        .expect_err("wrong response length must fail without retry");
        let length_message = length_error.to_string();
        assert!(length_message.contains("response body length"));
        assert!(length_message.contains("after 1 attempt(s)"));
        assert_eq!(length_server.join().expect("join fixture server"), 1);
        assert!(partial_files(&length_root).is_empty());
        fs::remove_dir_all(length_root).expect("remove length test directory");
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

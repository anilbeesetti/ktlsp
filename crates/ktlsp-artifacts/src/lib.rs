//! Bounded exact Maven-layout lookup and caller-provided SHA-256 verification.
//! See the crate README for the filesystem and integrity trust boundaries.

mod integrity;
pub use integrity::{ExpectedDigestOrigin, IntegrityError, VerifiedArtifact, verify_sha256};

use std::collections::HashSet;
use std::fs::{self, File, Metadata};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

pub const MAX_ARTIFACT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_ROOTS: usize = 16;
pub const MAX_ROOT_BYTES: usize = 4096;

/// An exact coordinate; construction rejects unsupported dynamic/snapshot forms.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Coordinate {
    group: String,
    artifact: String,
    version: String,
    classifier: Option<String>,
    extension: String,
}

impl Coordinate {
    pub fn new(
        group: &str,
        artifact: &str,
        version: &str,
        classifier: Option<&str>,
        extension: &str,
    ) -> Result<Self, LookupError> {
        let invalid = |field, reason| LookupError::InvalidCoordinate { field, reason };
        if group.len() > 256 || group.split('.').count() > 32 {
            return Err(invalid("group", "exceeds 256 bytes or 32 segments"));
        }
        for segment in group.split('.') {
            validate_segment("group", segment)?;
        }
        for (field, value) in [
            ("artifact", artifact),
            ("version", version),
            ("extension", extension),
        ] {
            validate_segment(field, value)?;
        }
        if let Some(classifier) = classifier {
            validate_segment("classifier", classifier)?;
        }
        let upper = version.to_ascii_uppercase();
        if matches!(upper.as_str(), "LATEST" | "RELEASE") || is_snapshot(&upper) {
            return Err(invalid(
                "version",
                "dynamic and snapshot versions unsupported",
            ));
        }
        if group.len()
            + artifact.len()
            + version.len()
            + classifier.map_or(0, str::len)
            + extension.len()
            > 512
        {
            return Err(invalid("coordinate", "exceeds 512 bytes"));
        }
        let coordinate = Self {
            group: group.into(),
            artifact: artifact.into(),
            version: version.into(),
            classifier: classifier.map(String::from),
            extension: extension.into(),
        };
        if coordinate.filename().len() > 255 {
            return Err(invalid("filename", "exceeds 255 bytes"));
        }
        Ok(coordinate)
    }

    pub fn group(&self) -> &str {
        &self.group
    }
    pub fn artifact(&self) -> &str {
        &self.artifact
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn classifier(&self) -> Option<&str> {
        self.classifier.as_deref()
    }
    pub fn extension(&self) -> &str {
        &self.extension
    }

    fn filename(&self) -> String {
        let classifier = self
            .classifier
            .as_ref()
            .map_or(String::new(), |s| format!("-{s}"));
        format!(
            "{}-{}{classifier}.{}",
            self.artifact, self.version, self.extension
        )
    }

    pub fn relative_path(&self) -> PathBuf {
        let mut path: PathBuf = self.group.split('.').collect();
        path.push(&self.artifact);
        path.push(&self.version);
        path.push(self.filename());
        path
    }
}

fn is_snapshot(version: &str) -> bool {
    if version.ends_with("SNAPSHOT") {
        return true;
    }
    let Some((prefix, build)) = version.rsplit_once('-') else {
        return false;
    };
    let Some((_, timestamp)) = prefix.rsplit_once('-') else {
        return false;
    };
    !build.is_empty()
        && build.bytes().all(|b| b.is_ascii_digit())
        && timestamp.len() == 15
        && timestamp.as_bytes()[8] == b'.'
        && timestamp
            .bytes()
            .enumerate()
            .all(|(i, b)| i == 8 || b.is_ascii_digit())
}

fn validate_segment(field: &'static str, value: &str) -> Result<(), LookupError> {
    let bytes = value.as_bytes();
    let valid = !bytes.is_empty()
        && bytes.len() <= 128
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'));
    if !valid {
        return Err(LookupError::InvalidCoordinate {
            field,
            reason: "requires a portable ASCII segment of 1..=128 bytes",
        });
    }
    let base = value.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            base.strip_prefix(prefix).is_some_and(|suffix| {
                suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9')
            })
        });
    if reserved {
        return Err(LookupError::InvalidCoordinate {
            field,
            reason: "requires a portable ASCII segment of 1..=128 bytes",
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChecksumStatus {
    Unverified,
}

/// Observations for the returned bytes, not a guarantee about a future reopen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Provenance {
    pub coordinate: Coordinate,
    pub repository_root: PathBuf,
    pub artifact_path: PathBuf,
    pub byte_length: usize,
    pub checksum: ChecksumStatus,
}

#[derive(Debug)]
pub struct Artifact {
    pub bytes: Vec<u8>,
    pub provenance: Provenance,
}

#[derive(Debug)]
pub enum LookupError {
    InvalidCoordinate {
        field: &'static str,
        reason: &'static str,
    },
    InvalidRoot {
        path: PathBuf,
        reason: &'static str,
    },
    TooManyRoots,
    UnsafePath {
        path: PathBuf,
    },
    Missing {
        searched_roots: Vec<PathBuf>,
    },
    Ambiguous {
        paths: Vec<PathBuf>,
    },
    Corrupt {
        path: PathBuf,
        reason: &'static str,
    },
    ArtifactTooLarge {
        path: PathBuf,
        limit: usize,
    },
    Io {
        path: PathBuf,
        source: io::Error,
    },
}

impl std::fmt::Display for LookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCoordinate { field, reason } => write!(f, "invalid {field}: {reason}"),
            Self::InvalidRoot { path, reason } => {
                write!(f, "invalid root {}: {reason}", path.display())
            }
            Self::TooManyRoots => write!(f, "more than {MAX_ROOTS} repository roots"),
            Self::UnsafePath { path } => write!(f, "symlink rejected: {}", path.display()),
            Self::Missing { .. } => write!(f, "artifact missing in supplied roots"),
            Self::Ambiguous { paths } => write!(f, "artifact found at {} paths", paths.len()),
            Self::Corrupt { path, reason } => {
                write!(f, "corrupt local entry {}: {reason}", path.display())
            }
            Self::ArtifactTooLarge { path, limit } => {
                write!(f, "artifact {} exceeds {limit} bytes", path.display())
            }
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl std::error::Error for LookupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn io_error(path: &Path, source: io::Error) -> LookupError {
    LookupError::Io {
        path: path.into(),
        source,
    }
}

/// Check every existing component, including root ancestors, without following
/// symlinks. Missing entries return None; wrong kinds are controlled errors.
fn inspect(path: &Path, file: bool) -> Result<Option<Metadata>, LookupError> {
    let mut current = PathBuf::new();
    let mut components = path.components().peekable();
    while let Some(component) = components.next() {
        current.push(component);
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io_error(&current, error)),
        };
        if metadata.file_type().is_symlink() {
            return Err(LookupError::UnsafePath { path: current });
        }
        let leaf = components.peek().is_none();
        if !(if leaf && file {
            metadata.is_file()
        } else {
            metadata.is_dir()
        }) {
            return Err(LookupError::Corrupt {
                path: current,
                reason: "unexpected filesystem entry kind",
            });
        }
        if leaf {
            return Ok(Some(metadata));
        }
    }
    Err(LookupError::InvalidRoot {
        path: path.into(),
        reason: "missing root component",
    })
}

fn check_size(path: &Path, metadata: &Metadata) -> Result<(), LookupError> {
    if metadata.len() > MAX_ARTIFACT_BYTES as u64 {
        return Err(LookupError::ArtifactTooLarge {
            path: path.into(),
            limit: MAX_ARTIFACT_BYTES,
        });
    }
    if metadata.len() == 0 {
        return Err(LookupError::Corrupt {
            path: path.into(),
            reason: "empty artifact",
        });
    }
    Ok(())
}

/// Locate and read one exact artifact. Filesystem errors take precedence over
/// Missing/Ambiguous, in supplied-root order. The cache must be trusted and
/// quiescent for the duration of this call; this is not a filesystem sandbox.
pub fn lookup(coordinate: &Coordinate, roots: &[PathBuf]) -> Result<Artifact, LookupError> {
    if roots.len() > MAX_ROOTS {
        return Err(LookupError::TooManyRoots);
    }
    let relative = coordinate.relative_path();
    let mut searched_roots = Vec::new();
    let mut unique = HashSet::new();
    let mut candidates = Vec::new();
    for root in roots {
        if !root.is_absolute()
            || root.as_os_str().as_encoded_bytes().len() > MAX_ROOT_BYTES
            || root.components().count() > 256
            || root.components().any(|c| matches!(c, Component::ParentDir))
        {
            return Err(LookupError::InvalidRoot {
                path: root.clone(),
                reason: "requires an absolute path without parent traversal, at most 4096 bytes/256 components",
            });
        }
        let exists = inspect(root, false)?.is_some();
        let root = if exists {
            fs::canonicalize(root).map_err(|e| io_error(root, e))?
        } else {
            root.components().collect()
        };
        if !unique.insert(root.clone()) {
            return Err(LookupError::InvalidRoot {
                path: root,
                reason: "duplicate repository root",
            });
        }
        searched_roots.push(root.clone());
        if exists {
            let path = root.join(&relative);
            if let Some(metadata) = inspect(&path, true)? {
                check_size(&path, &metadata)?;
                candidates.push((root, path, metadata.len()));
            }
        }
    }
    if candidates.len() > 1 {
        return Err(LookupError::Ambiguous {
            paths: candidates.into_iter().map(|(_, path, _)| path).collect(),
        });
    }
    let Some((root, path, length)) = candidates.pop() else {
        return Err(LookupError::Missing { searched_roots });
    };
    // shortcut: pathname checks require a trusted quiescent cache; use handle-relative no-follow I/O before accepting untrusted writers.
    let file = File::open(&path).map_err(|e| io_error(&path, e))?;
    let opened = file.metadata().map_err(|e| io_error(&path, e))?;
    if !opened.is_file() || opened.len() != length {
        return Err(LookupError::Corrupt {
            path,
            reason: "entry changed before read",
        });
    }
    let mut bytes = Vec::new();
    (&file)
        .take(MAX_ARTIFACT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| io_error(&path, e))?;
    if bytes.len() > MAX_ARTIFACT_BYTES {
        return Err(LookupError::ArtifactTooLarge {
            path,
            limit: MAX_ARTIFACT_BYTES,
        });
    }
    if bytes.len() as u64 != length
        || file.metadata().map_err(|e| io_error(&path, e))?.len() != length
    {
        return Err(LookupError::Corrupt {
            path,
            reason: "artifact length changed during read",
        });
    }
    let provenance = Provenance {
        coordinate: coordinate.clone(),
        repository_root: root,
        artifact_path: path,
        byte_length: bytes.len(),
        checksum: ChecksumStatus::Unverified,
    };
    Ok(Artifact { bytes, provenance })
}

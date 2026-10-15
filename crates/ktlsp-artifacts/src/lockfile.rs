//! Ordered caller-digest bindings for exact offline artifact lookup.
//! Expected digests are caller claims, not authenticated artifact identities.

use crate::{
    Coordinate, ExpectedDigestOrigin, IntegrityError, LookupError, VerifiedArtifact, lookup,
    verify_sha256,
};
use serde::Deserialize;
use serde::de::{MapAccess, SeqAccess, Visitor};
use std::collections::HashSet;
use std::path::PathBuf;

pub const MAX_LOCKFILE_BYTES: usize = 1024 * 1024;
pub const MAX_ENTRIES: usize = 1024;
pub const MAX_RELATIVE_PATH_BYTES: usize = 1024;

#[derive(Debug)]
pub struct Lockfile {
    entries: Vec<LockedArtifact>,
}

impl Lockfile {
    pub fn entries(&self) -> &[LockedArtifact] {
        &self.entries
    }

    /// Read only the requested binding; absence never falls back to an unlocked lookup.
    pub fn lookup(
        &self,
        coordinate: &Coordinate,
        roots: &[PathBuf],
    ) -> Result<VerifiedArtifact, LockedLookupError> {
        self.entries
            .iter()
            .find(|entry| &entry.coordinate == coordinate)
            .ok_or(LockedLookupError::NotLocked)?
            .lookup(roots)
    }
}

#[derive(Debug)]
pub struct LockedArtifact {
    coordinate: Coordinate,
    digest: [u8; 32],
}

impl LockedArtifact {
    pub fn coordinate(&self) -> &Coordinate {
        &self.coordinate
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    pub fn expected_digest_origin(&self) -> ExpectedDigestOrigin {
        ExpectedDigestOrigin::CallerProvided
    }

    /// Roots must be trusted and quiescent, as required by `crate::lookup`.
    /// Retains lookup provenance unchanged; equality does not authenticate the digest.
    pub fn lookup(&self, roots: &[PathBuf]) -> Result<VerifiedArtifact, LockedLookupError> {
        let artifact = lookup(&self.coordinate, roots).map_err(LockedLookupError::Lookup)?;
        verify_sha256(artifact, self.digest).map_err(LockedLookupError::Integrity)
    }
}

#[derive(Debug)]
pub enum LockfileError {
    InputTooLarge,
    Json(serde_json::Error),
    UnsupportedSchema { version: u32 },
    Coordinate { entry: usize, source: LookupError },
    DuplicateCoordinate { entry: usize },
    RelativePathTooLong { entry: usize },
    InvalidDigest { entry: usize },
}

impl std::fmt::Display for LockfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InputTooLarge => write!(f, "lockfile exceeds {MAX_LOCKFILE_BYTES} bytes"),
            Self::Json(source) => write!(f, "invalid lockfile JSON: {source}"),
            Self::UnsupportedSchema { version } => {
                write!(f, "unsupported lockfile schema {version}")
            }
            Self::Coordinate { entry, source } => write!(f, "lockfile entry {entry}: {source}"),
            Self::DuplicateCoordinate { entry } => {
                write!(f, "duplicate lockfile coordinate at {entry}")
            }
            Self::RelativePathTooLong { entry } => {
                write!(f, "lockfile entry {entry} path too long")
            }
            Self::InvalidDigest { entry } => {
                write!(f, "lockfile entry {entry} requires 64 hex digits")
            }
        }
    }
}

impl std::error::Error for LockfileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(source) => Some(source),
            Self::Coordinate { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum LockedLookupError {
    NotLocked,
    Lookup(LookupError),
    Integrity(IntegrityError),
}

impl std::fmt::Display for LockedLookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotLocked => write!(f, "coordinate absent from lockfile"),
            Self::Lookup(source) => write!(f, "{source}"),
            Self::Integrity(source) => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for LockedLookupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotLocked => None,
            Self::Lookup(source) => Some(source),
            Self::Integrity(source) => Some(source),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawLockfile {
    schema_version: u32,
    #[serde(deserialize_with = "bounded_entries")]
    entries: Vec<RawEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawEntry {
    group: String,
    artifact: String,
    version: String,
    classifier: Option<String>,
    extension: String,
    sha256: String,
    digest_origin: DigestOrigin,
}

#[derive(Deserialize)]
#[serde(try_from = "String")]
enum DigestOrigin {
    CallerProvided,
}

impl TryFrom<String> for DigestOrigin {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "callerProvided" => Ok(Self::CallerProvided),
            _ => Err("digestOrigin must be callerProvided"),
        }
    }
}

// Derived structs also accept JSON arrays; require object syntax at both levels.
struct Object<T>(T);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
            type Value = Object<T>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a lockfile object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(serde::de::value::MapAccessDeserializer::new(map)).map(Object)
            }
        }
        deserializer.deserialize_map(ObjectVisitor(std::marker::PhantomData))
    }
}

fn bounded_entries<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<RawEntry>, D::Error> {
    struct Entries;
    impl<'de> Visitor<'de> for Entries {
        type Value = Vec<RawEntry>;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "at most {MAX_ENTRIES} lockfile entries")
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
            let mut entries = Vec::new();
            while entries.len() < MAX_ENTRIES {
                let Some(Object(entry)) = sequence.next_element()? else {
                    return Ok(entries);
                };
                entries.push(entry);
            }
            if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(serde::de::Error::custom("too many lockfile entries"));
            }
            Ok(entries)
        }
    }
    deserializer.deserialize_seq(Entries)
}

/// Parse at most 1 MiB, retaining at most 1024 ordered exact-coordinate bindings.
/// Unknown/duplicate fields and trailing JSON reject. Classifier absent/null are equal.
/// JSON decoding (including the entry limit) precedes schema and entry validation.
/// Entry checks are coordinate, path length, duplicate coordinate, then digest.
/// This reads no files and authenticates neither lockfile nor expected digest.
pub fn parse_lockfile(bytes: &[u8]) -> Result<Lockfile, LockfileError> {
    if bytes.len() > MAX_LOCKFILE_BYTES {
        return Err(LockfileError::InputTooLarge);
    }
    let Object(raw): Object<RawLockfile> =
        serde_json::from_slice(bytes).map_err(LockfileError::Json)?;
    if raw.schema_version != 1 {
        return Err(LockfileError::UnsupportedSchema {
            version: raw.schema_version,
        });
    }
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for (entry, raw) in raw.entries.into_iter().enumerate() {
        let coordinate = Coordinate::new(
            &raw.group,
            &raw.artifact,
            &raw.version,
            raw.classifier.as_deref(),
            &raw.extension,
        )
        .map_err(|source| LockfileError::Coordinate { entry, source })?;
        if coordinate
            .relative_path()
            .as_os_str()
            .as_encoded_bytes()
            .len()
            > MAX_RELATIVE_PATH_BYTES
        {
            return Err(LockfileError::RelativePathTooLong { entry });
        }
        if !seen.insert((
            raw.group,
            raw.artifact,
            raw.version,
            raw.classifier,
            raw.extension,
        )) {
            return Err(LockfileError::DuplicateCoordinate { entry });
        }
        let mut digest = [0; 32];
        if raw.sha256.len() != 64 || !raw.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(LockfileError::InvalidDigest { entry });
        }
        for (byte, pair) in digest
            .iter_mut()
            .zip(raw.sha256.as_bytes().as_chunks::<2>().0)
        {
            let hex = |b: u8| {
                if b <= b'9' {
                    b - b'0'
                } else {
                    b.to_ascii_lowercase() - b'a' + 10
                }
            };
            *byte = hex(pair[0]) * 16 + hex(pair[1]);
        }
        let DigestOrigin::CallerProvided = raw.digest_origin;
        entries.push(LockedArtifact { coordinate, digest });
    }
    Ok(Lockfile { entries })
}

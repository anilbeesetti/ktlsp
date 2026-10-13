use crate::{Artifact, MAX_ARTIFACT_BYTES, Provenance};
use sha2::{Digest, Sha256};

/// This API does not authenticate the expected digest or bind it to a coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedDigestOrigin {
    CallerProvided,
}

/// Owned bytes equal to a caller-provided SHA-256, with no mutable access.
/// Lookup provenance is retained as supplied, including its Unverified status.
/// Paths and coordinates are observations, not authenticated identity or a
/// guarantee about future path contents. ZIP/JAR validity is not checked.
///
/// ```compile_fail
/// use ktlsp_artifacts::VerifiedArtifact;
/// fn mutate(mut artifact: VerifiedArtifact) {
///     artifact.bytes()[0] = 0;
/// }
/// ```
/// ```compile_fail
/// use ktlsp_artifacts::VerifiedArtifact;
/// fn mutate(mut artifact: VerifiedArtifact) {
///     artifact.provenance().byte_length = 0;
/// }
/// ```
/// ```compile_fail
/// use ktlsp_artifacts::VerifiedArtifact;
/// fn mutate(mut artifact: VerifiedArtifact) {
///     artifact.digest()[0] = 0;
/// }
/// ```
/// ```compile_fail
/// use ktlsp_artifacts::{Artifact, VerifiedArtifact};
/// fn bypass(artifact: Artifact) -> VerifiedArtifact {
///     VerifiedArtifact { artifact, digest: [0; 32] }
/// }
/// ```
#[derive(Debug)]
pub struct VerifiedArtifact {
    artifact: Artifact,
    digest: [u8; 32],
}

impl VerifiedArtifact {
    pub fn bytes(&self) -> &[u8] {
        &self.artifact.bytes
    }

    pub fn provenance(&self) -> &Provenance {
        &self.artifact.provenance
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    pub fn expected_digest_origin(&self) -> ExpectedDigestOrigin {
        ExpectedDigestOrigin::CallerProvided
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IntegrityError {
    Empty,
    TooLarge {
        actual: usize,
        limit: usize,
    },
    InconsistentByteLength {
        recorded: usize,
        actual: usize,
    },
    Mismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
}

impl std::fmt::Display for IntegrityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "empty artifact bytes"),
            Self::TooLarge { actual, limit } => {
                write!(f, "artifact has {actual} bytes, exceeds {limit} bytes")
            }
            Self::InconsistentByteLength { recorded, actual } => {
                write!(
                    f,
                    "artifact records {recorded} bytes, contains {actual} bytes"
                )
            }
            Self::Mismatch { .. } => write!(f, "artifact SHA-256 mismatch"),
        }
    }
}

impl std::error::Error for IntegrityError {}

/// Verify the existing allocation without cloning bytes or accessing paths.
/// Validation order is actual empty, actual oversize, recorded-length mismatch,
/// then digest mismatch. Errors consume and drop the supplied artifact.
pub fn verify_sha256(
    artifact: Artifact,
    expected: [u8; 32],
) -> Result<VerifiedArtifact, IntegrityError> {
    let actual = artifact.bytes.len();
    if actual == 0 {
        return Err(IntegrityError::Empty);
    }
    if actual > MAX_ARTIFACT_BYTES {
        return Err(IntegrityError::TooLarge {
            actual,
            limit: MAX_ARTIFACT_BYTES,
        });
    }
    if artifact.provenance.byte_length != actual {
        return Err(IntegrityError::InconsistentByteLength {
            recorded: artifact.provenance.byte_length,
            actual,
        });
    }
    let digest: [u8; 32] = Sha256::digest(&artifact.bytes).into();
    if digest != expected {
        return Err(IntegrityError::Mismatch {
            expected,
            actual: digest,
        });
    }
    Ok(VerifiedArtifact { artifact, digest })
}

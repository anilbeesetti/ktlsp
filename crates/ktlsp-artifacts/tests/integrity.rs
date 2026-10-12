use ktlsp_artifacts::{
    Artifact, ChecksumStatus, Coordinate, ExpectedDigestOrigin, IntegrityError, MAX_ARTIFACT_BYTES,
    Provenance, lookup, verify_sha256,
};
use std::fs;

fn artifact(bytes: Vec<u8>) -> Artifact {
    Artifact {
        provenance: Provenance {
            coordinate: Coordinate::new("org.example", "lib", "1", None, "jar").unwrap(),
            repository_root: "never-opened-root".into(),
            artifact_path: "never-opened-path\0".into(),
            byte_length: bytes.len(),
            checksum: ChecksumStatus::Unverified,
        },
        bytes,
    }
}

fn digest(hex: &str) -> [u8; 32] {
    assert_eq!(hex.len(), 64);
    std::array::from_fn(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
}

const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn check(bytes: Vec<u8>, hex: &str) {
    let artifact = artifact(bytes);
    let pointer = artifact.bytes.as_ptr();
    let provenance = artifact.provenance.clone();
    let expected = digest(hex);
    let verified = verify_sha256(artifact, expected).unwrap();
    assert_eq!(verified.bytes().as_ptr(), pointer);
    assert_eq!(verified.provenance(), &provenance);
    assert_eq!(verified.digest(), &expected);
    assert_eq!(verified.provenance().checksum, ChecksumStatus::Unverified);
    assert_eq!(
        verified.expected_digest_origin(),
        ExpectedDigestOrigin::CallerProvided
    );
}

#[test]
fn published_known_answers() {
    // NIST SHA256 worked examples; see INTEGRITY.md for source/independent checks.
    check(b"abc".to_vec(), ABC);
    check(
        b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".to_vec(),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
    );
    check(
        vec![b'a'; 1_000_000],
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
    );
}

#[test]
fn padding_and_block_boundaries() {
    for (length, hex) in [
        (
            1,
            "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb",
        ),
        (
            55,
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318",
        ),
        (
            56,
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a",
        ),
        (
            63,
            "7d3e74a05d7db15bce4ad9ec0658ea98e3f06eeecf16b4c6fff2da457ddc2f34",
        ),
        (
            64,
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb",
        ),
        (
            65,
            "635361c48bb9eab14198e76ea8ab7f1a41685d6ad62aa9146d301d4f17eb0ae0",
        ),
        (
            119,
            "31eba51c313a5c08226adf18d4a359cfdfd8d2e816b13f4af952f7ea6584dcfb",
        ),
        (
            120,
            "2f3d335432c70b580af0e8e1b3674a7c020d683aa5f73aaaedfdc55af904c21c",
        ),
        (
            127,
            "c57e9278af78fa3cab38667bef4ce29d783787a2f731d4e12200270f0c32320a",
        ),
        (
            128,
            "6836cf13bac400e9105071cd6af47084dfacad4e5e302c94bfed24e013afb73e",
        ),
        (
            129,
            "c12cb024a2e5551cca0e08fce8f1c5e314555cc3fef6329ee994a3db752166ae",
        ),
    ] {
        check(vec![b'a'; length], hex);
    }
}

#[test]
fn binary_payloads() {
    check(
        vec![0; 257],
        "6c934d0cdf9dba94b474d6d1929f16739bd9a8ed31d0c3bcaf82c283fb7a3568",
    );
    check(
        vec![255; 257],
        "4212c75089aa25b5734dd67583cd3952fb35492d3c38285225e66a2d70403600",
    );
    let mut seed = 0x1234_5678u32;
    let bytes = (0..4097)
        .map(|_| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 24) as u8
        })
        .collect();
    check(
        bytes,
        "640936ce58f3170cda9fb6ab9b0c201172b1f81294561c75b10c8f7056c1241c",
    );
}

#[test]
fn first_and_last_bit_mismatches_report_both_digests() {
    let actual = digest(ABC);
    for (index, bit) in [(0, 0x80), (31, 1)] {
        let mut expected = actual;
        expected[index] ^= bit;
        assert_eq!(
            verify_sha256(artifact(b"abc".to_vec()), expected).unwrap_err(),
            IntegrityError::Mismatch { expected, actual }
        );
    }
    for index in [0, 2] {
        let mut bytes = b"abc".to_vec();
        bytes[index] ^= 1;
        assert!(
            matches!(verify_sha256(artifact(bytes), actual), Err(IntegrityError::Mismatch { expected, actual: observed }) if expected == actual && observed != actual)
        );
    }
}

#[test]
fn size_and_metadata_validation_precedes_hashing() {
    for recorded in [0, 1, usize::MAX] {
        let mut empty = artifact(Vec::new());
        empty.provenance.byte_length = recorded;
        assert_eq!(
            verify_sha256(empty, [0; 32]).unwrap_err(),
            IntegrityError::Empty
        );
        let mut oversized = artifact(vec![0; MAX_ARTIFACT_BYTES + 1]);
        oversized.provenance.byte_length = recorded;
        assert_eq!(
            verify_sha256(oversized, [0; 32]).unwrap_err(),
            IntegrityError::TooLarge {
                actual: MAX_ARTIFACT_BYTES + 1,
                limit: MAX_ARTIFACT_BYTES,
            }
        );
    }
    for recorded in [0, 2, 4, MAX_ARTIFACT_BYTES + 1, usize::MAX] {
        let mut inconsistent = artifact(b"abc".to_vec());
        inconsistent.provenance.byte_length = recorded;
        assert_eq!(
            verify_sha256(inconsistent, [0; 32]).unwrap_err(),
            IntegrityError::InconsistentByteLength {
                recorded,
                actual: 3
            }
        );
    }
}

#[test]
fn exact_size_cap() {
    check(
        vec![0; MAX_ARTIFACT_BYTES],
        "3b6a07d0d404fab4e23b6d34bc6696a6a312dd92821332385e5af7c01c421351",
    );
}

#[test]
fn verification_retains_loaded_bytes_across_path_replacement_and_deletion() {
    let root = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("ktlsp-integrity-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let coordinate = Coordinate::new("org.example", "lib", "1", None, "jar").unwrap();
    let path = root.join(coordinate.relative_path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"abc").unwrap();
    let loaded = lookup(&coordinate, std::slice::from_ref(&root)).unwrap();
    let pointer = loaded.bytes.as_ptr();
    assert_eq!(loaded.provenance.checksum, ChecksumStatus::Unverified);
    fs::remove_file(&path).unwrap();
    fs::write(&path, b"different replacement").unwrap();
    let verified = verify_sha256(loaded, digest(ABC)).unwrap();
    fs::remove_file(&path).unwrap();
    assert_eq!(verified.bytes(), b"abc");
    assert_eq!(verified.bytes().as_ptr(), pointer);
    assert_eq!(verified.provenance().artifact_path, path);
    assert_eq!(verified.digest(), &digest(ABC));
    fs::remove_dir_all(root).unwrap();
}

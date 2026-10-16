use ktlsp_artifacts::lockfile::{
    LockedLookupError, LockfileError, MAX_ENTRIES, MAX_LOCKFILE_BYTES, parse_lockfile,
};
use ktlsp_artifacts::{
    ChecksumStatus, Coordinate, ExpectedDigestOrigin, IntegrityError, LookupError,
};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const FIXTURE: &[u8] = include_bytes!("fixtures/lockfile/ordered.json");
const ABC: [u8; 32] = [
    0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22, 0x23,
    0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad,
];

fn fixture() -> Value {
    serde_json::from_slice(FIXTURE).unwrap()
}

fn parse(value: &Value) -> Result<ktlsp_artifacts::lockfile::Lockfile, LockfileError> {
    parse_lockfile(&serde_json::to_vec(value).unwrap())
}

struct Cache(PathBuf);
impl Cache {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "ktlsp-lockfile-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn put(&self, coordinate: &Coordinate, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(coordinate.relative_path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }
}
impl Drop for Cache {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn ordered_binary_sources_and_retained_caller_integrity() {
    let lock = parse_lockfile(FIXTURE).unwrap();
    let cache = Cache::new();
    assert_eq!(lock.entries().len(), 2);
    assert_eq!(lock.entries()[0].coordinate().classifier(), None);
    assert_eq!(lock.entries()[1].coordinate().classifier(), Some("sources"));
    for entry in lock.entries() {
        assert_eq!(entry.digest(), &ABC);
        assert_eq!(
            entry.expected_digest_origin(),
            ExpectedDigestOrigin::CallerProvided
        );
        let path = cache.put(entry.coordinate(), b"abc");
        let artifact = lock
            .lookup(entry.coordinate(), std::slice::from_ref(&cache.0))
            .unwrap();
        fs::write(&path, b"replaced").unwrap();
        assert_eq!(artifact.bytes(), b"abc");
        assert_eq!(artifact.digest(), &ABC);
        assert_eq!(artifact.provenance().coordinate, *entry.coordinate());
        assert_eq!(artifact.provenance().artifact_path, path);
        assert_eq!(artifact.provenance().repository_root, cache.0);
        assert_eq!(artifact.provenance().byte_length, 3);
        assert_eq!(artifact.provenance().checksum, ChecksumStatus::Unverified);
        assert_eq!(
            artifact.expected_digest_origin(),
            ExpectedDigestOrigin::CallerProvided
        );
    }
}

#[test]
fn not_locked_missing_integrity_and_invalid_root_are_distinct() {
    let lock = parse_lockfile(FIXTURE).unwrap();
    let entry = &lock.entries()[0];
    let other = Coordinate::new("org.example", "other", "1", None, "jar").unwrap();
    assert!(matches!(
        lock.lookup(&other, &[]),
        Err(LockedLookupError::NotLocked)
    ));
    assert!(
        matches!(entry.lookup(&[]), Err(LockedLookupError::Lookup(LookupError::Missing { searched_roots })) if searched_roots.is_empty())
    );
    assert!(matches!(
        entry.lookup(&[PathBuf::from("relative")]),
        Err(LockedLookupError::Lookup(LookupError::InvalidRoot { .. }))
    ));
    let cache = Cache::new();
    cache.put(entry.coordinate(), b"abd");
    assert!(
        matches!(entry.lookup(std::slice::from_ref(&cache.0)), Err(LockedLookupError::Integrity(IntegrityError::Mismatch { expected, actual })) if expected == ABC && actual != ABC)
    );
    cache.put(entry.coordinate(), b"");
    assert!(matches!(
        entry.lookup(std::slice::from_ref(&cache.0)),
        Err(LockedLookupError::Lookup(LookupError::Corrupt { .. }))
    ));
}

#[test]
fn ambiguity_is_not_filtered_by_digest() {
    let lock = parse_lockfile(FIXTURE).unwrap();
    let entry = &lock.entries()[0];
    let a = Cache::new();
    let b = Cache::new();
    a.put(entry.coordinate(), b"abc");
    b.put(entry.coordinate(), b"wrong digest");
    assert!(
        matches!(entry.lookup(&[a.0.clone(), b.0.clone()]), Err(LockedLookupError::Lookup(LookupError::Ambiguous { paths })) if paths.len() == 2)
    );
}

#[cfg(unix)]
#[test]
fn inherited_symlink_rejection() {
    let lock = parse_lockfile(FIXTURE).unwrap();
    let entry = &lock.entries()[0];
    let cache = Cache::new();
    let path = cache.put(entry.coordinate(), b"abc");
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink("missing", &path).unwrap();
    assert!(
        matches!(entry.lookup(std::slice::from_ref(&cache.0)), Err(LockedLookupError::Lookup(LookupError::UnsafePath { path: rejected })) if rejected == path)
    );
}

#[test]
fn strict_schema_required_unknown_and_duplicate_fields() {
    assert!(parse_lockfile(b"[1, []]").is_err());
    let mut array_entry = fixture();
    array_entry["entries"][0] = json!([
        "org.example",
        "lib",
        "1.2.3",
        null,
        "jar",
        "0".repeat(64),
        "callerProvided"
    ]);
    assert!(parse(&array_entry).is_err());
    for field in ["schemaVersion", "entries"] {
        let mut value = fixture();
        value.as_object_mut().unwrap().remove(field);
        assert!(
            matches!(parse(&value), Err(LockfileError::Json(_))),
            "{field}"
        );
    }
    for field in [
        "group",
        "artifact",
        "version",
        "extension",
        "sha256",
        "digestOrigin",
    ] {
        let mut value = fixture();
        value["entries"][0].as_object_mut().unwrap().remove(field);
        assert!(
            matches!(parse(&value), Err(LockfileError::Json(_))),
            "{field}"
        );
    }
    for (field, value) in [
        ("schemaVersion", json!(2)),
        ("schemaVersion", json!(null)),
        ("schemaVersion", json!(-1)),
        ("schemaVersion", json!(1.5)),
        ("schemaVersion", json!(4294967296u64)),
        ("schemaVersion", json!("1")),
        ("entries", json!(null)),
        ("entries", json!({})),
        ("extra", json!(true)),
    ] {
        let mut input = fixture();
        input[field] = value;
        assert!(parse(&input).is_err(), "{field}: {input}");
    }
    assert!(matches!(
        parse(&json!({"schemaVersion": 2, "entries": []})),
        Err(LockfileError::UnsupportedSchema { version: 2 })
    ));
    let mut value = fixture();
    value["entries"][0]["extra"] = json!(0);
    assert!(matches!(parse(&value), Err(LockfileError::Json(_))));
    let text = std::str::from_utf8(FIXTURE).unwrap();
    for field in [
        "schemaVersion",
        "entries",
        "group",
        "artifact",
        "version",
        "classifier",
        "extension",
        "sha256",
        "digestOrigin",
    ] {
        let value = fixture();
        let first = match field {
            "schemaVersion" | "entries" => &value[field],
            "classifier" => &value["entries"][1][field],
            _ => &value["entries"][0][field],
        };
        let duplicate = text.replacen(
            &format!("\"{field}\":"),
            &format!("\"{field}\":{first},\"{field}\":"),
            1,
        );
        assert!(
            matches!(parse_lockfile(duplicate.as_bytes()), Err(LockfileError::Json(error)) if error.to_string().contains("duplicate field")),
            "{field}"
        );
    }
    assert!(parse_lockfile(format!("{text} {{}}").as_bytes()).is_err());
    assert!(
        parse(&json!({"schemaVersion":1,"entries":[]}))
            .unwrap()
            .entries()
            .is_empty()
    );
}

#[test]
fn duplicate_and_conflicting_coordinate_bindings_reject() {
    for digest in [
        json!("0".repeat(64)),
        fixture()["entries"][0]["sha256"].clone(),
    ] {
        let mut value = fixture();
        let mut duplicate = value["entries"][0].clone();
        duplicate["sha256"] = digest;
        duplicate["classifier"] = Value::Null;
        value["entries"].as_array_mut().unwrap().push(duplicate);
        assert!(matches!(
            parse(&value),
            Err(LockfileError::DuplicateCoordinate { entry: 2 })
        ));
    }
}

#[test]
fn digest_and_origin_validation() {
    for digest in [
        "".to_string(),
        "a".repeat(63),
        "a".repeat(65),
        "g".repeat(64),
        "é".repeat(32),
        format!("{} ", "a".repeat(63)),
    ] {
        let mut value = fixture();
        value["entries"][0]["sha256"] = json!(digest);
        assert!(matches!(
            parse(&value),
            Err(LockfileError::InvalidDigest { entry: 0 })
        ));
    }
    for origin in [
        json!("authenticated"),
        json!({"callerProvided": null}),
        json!(null),
        json!({}),
        json!(1),
    ] {
        let mut value = fixture();
        value["entries"][0]["digestOrigin"] = origin;
        assert!(matches!(parse(&value), Err(LockfileError::Json(_))));
    }
}

#[test]
fn inherited_coordinate_validation() {
    for (field, invalid) in [
        ("group", "../org"),
        ("group", "org..example"),
        ("artifact", "a/b"),
        ("artifact", "CON"),
        ("version", "LATEST"),
        ("version", "1-SNAPSHOT"),
        ("version", "1-20260101.120000-1"),
        ("classifier", ".."),
        ("extension", "jar/../../x"),
    ] {
        let mut value = fixture();
        value["entries"][0][field] = json!(invalid);
        assert!(
            matches!(
                parse(&value),
                Err(LockfileError::Coordinate { entry: 0, .. })
            ),
            "{field}={invalid}"
        );
    }
    let mut value = fixture();
    value["entries"][0]["artifact"] = json!("a".repeat(128));
    assert!(parse(&value).is_ok());
    value["entries"][0]["artifact"] = json!("a".repeat(129));
    assert!(matches!(
        parse(&value),
        Err(LockfileError::Coordinate { entry: 0, .. })
    ));
}

#[test]
fn exact_and_over_input_and_entry_limits() {
    let mut bytes = FIXTURE.to_vec();
    bytes.resize(MAX_LOCKFILE_BYTES, b' ');
    assert!(parse_lockfile(&bytes).is_ok());
    bytes.push(b' ');
    assert!(matches!(
        parse_lockfile(&bytes),
        Err(LockfileError::InputTooLarge)
    ));
    let template = fixture()["entries"][0].clone();
    let entries: Vec<Value> = (0..MAX_ENTRIES)
        .map(|i| {
            let mut entry = template.clone();
            entry["artifact"] = json!(format!("lib{i}"));
            entry
        })
        .collect();
    let mut value = json!({"schemaVersion":1,"entries":entries});
    assert_eq!(parse(&value).unwrap().entries().len(), MAX_ENTRIES);
    value["entries"].as_array_mut().unwrap().push(template);
    assert!(matches!(parse(&value), Err(LockfileError::Json(_))));
}

#[test]
fn truncations_nesting_and_generated_bytes_are_controlled() {
    let closing = FIXTURE.iter().rposition(|b| *b == b'}').unwrap();
    for end in 0..=closing {
        assert!(parse_lockfile(&FIXTURE[..end]).is_err(), "prefix {end}");
    }
    let deep = format!("{}0{}", "[".repeat(256), "]".repeat(256));
    assert!(parse_lockfile(deep.as_bytes()).is_err());
    for n in 0..2048usize {
        let bytes: Vec<u8> = (0..n % 256)
            .map(|i| (n.wrapping_mul(97).wrapping_add(i * 31) % 256) as u8)
            .collect();
        assert!(parse_lockfile(&bytes).is_err());
    }
}

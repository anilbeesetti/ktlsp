use ktlsp_artifacts::{
    ChecksumStatus, Coordinate, LookupError, MAX_ARTIFACT_BYTES, MAX_ROOTS, lookup,
};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Cache(PathBuf);
impl Cache {
    fn new() -> Self {
        Self::under(&std::env::temp_dir())
    }
    fn under(parent: &Path) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = parent.canonicalize().unwrap().join(format!(
            "ktlsp-artifacts-{}-{}",
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
    fn roots(&self) -> Vec<PathBuf> {
        vec![self.0.clone()]
    }
}
impl Drop for Cache {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn binary() -> Coordinate {
    Coordinate::new("org.example", "lib", "1.2.3", None, "jar").unwrap()
}

#[test]
fn exact_binary_sources_and_provenance() {
    let cache = Cache::new();
    let binary = binary();
    let sources = Coordinate::new("org.example", "lib", "1.2.3", Some("sources"), "jar").unwrap();
    assert_eq!(
        binary.relative_path(),
        Path::new("org/example/lib/1.2.3/lib-1.2.3.jar")
    );
    assert_eq!(
        sources.relative_path(),
        Path::new("org/example/lib/1.2.3/lib-1.2.3-sources.jar")
    );
    let path = cache.put(&binary, b"opaque binary data\0\xff");
    cache.put(&sources, b"opaque sources data");
    // A sidecar alone must never imply verified integrity.
    fs::write(path.with_extension("jar.sha256"), b"not a checksum").unwrap();
    let artifact = lookup(&binary, &cache.roots()).unwrap();
    assert_eq!(artifact.bytes, b"opaque binary data\0\xff");
    assert_eq!(artifact.provenance.coordinate, binary);
    assert_eq!(artifact.provenance.repository_root, cache.0);
    assert_eq!(artifact.provenance.artifact_path, path);
    assert_eq!(artifact.provenance.byte_length, artifact.bytes.len());
    assert_eq!(artifact.provenance.checksum, ChecksumStatus::Unverified);
    assert_eq!(
        lookup(&sources, &cache.roots()).unwrap().bytes,
        b"opaque sources data"
    );
    assert_eq!(
        (
            sources.group(),
            sources.artifact(),
            sources.version(),
            sources.classifier(),
            sources.extension()
        ),
        ("org.example", "lib", "1.2.3", Some("sources"), "jar")
    );
}

#[test]
fn invalid_coordinates_are_unconstructible() {
    for value in [
        "", ".", "..", "/tmp", "a/b", "a\\b", "a:b", " x", "x ", "a\0b", "é", "%2e", "CON",
        "nul.txt", "LPT9", "COM1", "a.", "-a",
    ] {
        assert!(
            Coordinate::new("org.example", value, "1", None, "jar").is_err(),
            "{value:?}"
        );
        assert!(
            Coordinate::new("org.example", "lib", value, None, "jar").is_err(),
            "{value:?}"
        );
        assert!(
            Coordinate::new("org.example", "lib", "1", Some(value), "jar").is_err(),
            "{value:?}"
        );
        assert!(
            Coordinate::new("org.example", "lib", "1", None, value).is_err(),
            "{value:?}"
        );
    }
    for group in [
        "",
        ".org",
        "org.",
        "org..example",
        "../org",
        "org/evil",
        "org.CON",
    ] {
        assert!(Coordinate::new(group, "lib", "1", None, "jar").is_err());
    }
    for version in [
        "LATEST",
        "release",
        "1-SNAPSHOT",
        "1-snapshot",
        "SNAPSHOT",
        "1SNAPSHOT",
        "snapshot",
        "1.snapshot",
        "1.0-20261012.123456-1",
        "1.0-00000000.000000-01",
        "[1,2)",
        "1.+",
        "+",
    ] {
        assert!(Coordinate::new("g", "a", version, None, "jar").is_err());
    }
    let huge = "a".repeat(129);
    assert!(Coordinate::new("g", &huge, "1", None, "jar").is_err());
    assert!(Coordinate::new(&vec!["g"; 33].join("."), "a", "1", None, "jar").is_err());
    assert!(Coordinate::new(&"g".repeat(257), "a", "1", None, "jar").is_err());
    assert!(Coordinate::new("g", &"a".repeat(128), &"1".repeat(128), None, "jar").is_err());
    assert!(
        Coordinate::new(
            &vec!["groupgroup"; 20].join("."),
            &"a".repeat(100),
            &"1".repeat(100),
            Some(&"c".repeat(100)),
            "jar"
        )
        .is_err()
    );
    assert!(Coordinate::new("g", "COM10", "1.0-rc_1", Some("linux-x86_64"), "tar.gz").is_ok());
    for version in [
        "1.0-2026101.123456-1",
        "1.0-20261012.12345-1",
        "1.0-20261012x123456-1",
        "1.0-20261012.123456-x",
    ] {
        assert!(Coordinate::new("g", "a", version, None, "jar").is_ok());
    }
}

#[test]
fn missing_is_explicit_and_never_scans_neighbors() {
    let cache = Cache::new();
    cache.put(
        &Coordinate::new("org.example", "lib", "2", None, "jar").unwrap(),
        b"other version",
    );
    let roots = vec![cache.0.join("absent"), cache.0.clone()];
    match lookup(&binary(), &roots).unwrap_err() {
        LookupError::Missing { searched_roots } => assert_eq!(searched_roots, roots),
        e => panic!("{e}"),
    }
    assert!(matches!(
        lookup(&binary(), &[]),
        Err(LookupError::Missing { .. })
    ));
}

#[test]
fn duplicates_and_invalid_roots_are_rejected() {
    let cache = Cache::new();
    cache.put(&binary(), b"data");
    for roots in [
        vec![cache.0.clone(), cache.0.join(".")],
        vec![PathBuf::from("relative")],
        vec![cache.0.join("../escape")],
        vec![cache.0.join("a".repeat(4096))],
    ] {
        assert!(matches!(
            lookup(&binary(), &roots),
            Err(LookupError::InvalidRoot { .. })
        ));
    }
    assert!(matches!(
        lookup(&binary(), &vec![cache.0.clone(); MAX_ROOTS + 1]),
        Err(LookupError::TooManyRoots)
    ));
}

#[test]
fn multiple_candidates_are_ambiguous_even_when_identical() {
    let first = Cache::new();
    let second = Cache::new();
    let paths = vec![
        first.put(&binary(), b"same"),
        second.put(&binary(), b"same"),
    ];
    match lookup(&binary(), &[first.0.clone(), second.0.clone()]).unwrap_err() {
        LookupError::Ambiguous { paths: found } => assert_eq!(found, paths),
        e => panic!("{e}"),
    }
    fs::remove_file(&paths[1]).unwrap();
    assert_eq!(
        lookup(&binary(), &[second.0.clone(), first.0.clone()])
            .unwrap()
            .bytes,
        b"same"
    );
}

#[test]
fn empty_wrong_kind_and_mixed_root_errors_take_precedence() {
    let good = Cache::new();
    let bad = Cache::new();
    good.put(&binary(), b"data");
    let path = bad.put(&binary(), b"");
    for roots in [
        vec![good.0.clone(), bad.0.clone()],
        vec![bad.0.clone(), good.0.clone()],
    ] {
        assert!(matches!(
            lookup(&binary(), &roots),
            Err(LookupError::Corrupt { .. })
        ));
    }
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(matches!(
        lookup(&binary(), &bad.roots()),
        Err(LookupError::Corrupt { .. })
    ));
    fs::remove_dir_all(bad.0.join("org")).unwrap();
    fs::write(bad.0.join("org"), b"not directory").unwrap();
    assert!(matches!(
        lookup(&binary(), &bad.roots()),
        Err(LookupError::Corrupt { .. })
    ));
}

#[test]
fn exact_size_limit_and_overflow() {
    let cache = Cache::new();
    let path = cache.put(&binary(), b"x");
    let file = File::options().write(true).open(&path).unwrap();
    file.set_len(MAX_ARTIFACT_BYTES as u64).unwrap();
    let artifact = lookup(&binary(), &cache.roots()).unwrap();
    assert_eq!(artifact.bytes.len(), MAX_ARTIFACT_BYTES);
    assert_eq!(artifact.provenance.byte_length, MAX_ARTIFACT_BYTES);
    drop(artifact);
    file.set_len(MAX_ARTIFACT_BYTES as u64 + 1).unwrap();
    assert!(matches!(
        lookup(&binary(), &cache.roots()),
        Err(LookupError::ArtifactTooLarge { .. })
    ));
}

#[cfg(unix)]
#[test]
fn symlinks_at_root_ancestors_directories_and_leaf_are_rejected() {
    use std::os::unix::fs::symlink;
    let cache = Cache::new();
    let links = Cache::new();
    let path = cache.put(&binary(), b"data");
    symlink(&cache.0, links.0.join("root")).unwrap();
    for root in [links.0.join("root"), links.0.join("root/absent")] {
        assert!(matches!(
            lookup(&binary(), &[root]),
            Err(LookupError::UnsafePath { .. })
        ));
    }
    symlink(cache.0.join("org"), links.0.join("org")).unwrap();
    assert!(matches!(
        lookup(&binary(), &links.roots()),
        Err(LookupError::UnsafePath { .. })
    ));
    fs::remove_file(&path).unwrap();
    symlink(links.0.join("absent"), &path).unwrap();
    assert!(matches!(
        lookup(&binary(), &cache.roots()),
        Err(LookupError::UnsafePath { .. })
    ));
}

#[cfg(unix)]
#[test]
fn special_file_is_rejected_without_opening() {
    // Unix socket paths have a much smaller limit than ordinary filesystem paths.
    let cache = Cache::under(Path::new("/tmp"));
    let path = cache.put(&binary(), b"data");
    fs::remove_file(&path).unwrap();
    let _socket = std::os::unix::net::UnixListener::bind(path).unwrap();
    assert!(matches!(
        lookup(&binary(), &cache.roots()),
        Err(LookupError::Corrupt { .. })
    ));
}

#[test]
fn bounded_generated_coordinate_crash_corpus() {
    for n in 0..4096usize {
        let value: String = (0..n % 160)
            .map(|i| char::from_u32(((n * 97 + i * 31) % 0x110000) as u32).unwrap_or('\0'))
            .collect();
        if let Ok(c) = Coordinate::new("g", &value, "1", None, "jar") {
            assert!(
                c.relative_path()
                    .components()
                    .all(|p| matches!(p, std::path::Component::Normal(_)))
            );
        }
    }
}

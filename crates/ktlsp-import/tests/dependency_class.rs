use ktlsp_artifacts::{
    Artifact, ChecksumStatus, Coordinate, Provenance, VerifiedArtifact, index_zip_directory,
    verify_sha256,
};
use ktlsp_binary::{
    Constant,
    class_path::{ClassRank, ClasspathError},
    source_attachment,
};
use ktlsp_import::{
    dependency_class::{ArchiveKind, ArtifactBinding, Error, select_class},
    v2,
};
use ktlsp_project::SourceRootKind;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const JAR: &[u8] =
    include_bytes!("../../ktlsp-binary/fixtures/t017c1/commons-lang3-3.17.0.jar.bin");
const SOURCES: &[u8] =
    include_bytes!("../../ktlsp-binary/fixtures/t017c1/commons-lang3-3.17.0-sources.jar.bin");
const BUILDER: &str = "org/apache/commons/lang3/AppendableJoiner$Builder";
const BINARY_DIGEST: &str = "6ee731df5c8e5a2976a1ca023b6bb320ea8d3539fbe64c8a1d5cb765127c33b4";
const SOURCE_DIGEST: &str = "5fdcac21ad329766054a95367d7583dfcdca737d221d5e01a5f2a198c04c6b18";

fn project() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
}
fn fixture(source: bool) -> PathBuf {
    project()
        .join("crates/ktlsp-binary/fixtures/t017c1")
        .join(if source {
            "commons-lang3-3.17.0-sources.jar.bin"
        } else {
            "commons-lang3-3.17.0.jar.bin"
        })
        .components()
        .collect()
}
fn source() -> PathBuf {
    project()
        .join("docs/specs/reference-binding-acceptance/same-block.kt")
        .components()
        .collect()
}
fn genuine(source: bool) -> VerifiedArtifact {
    let bytes = if source { SOURCES } else { JAR };
    let expected = if source { SOURCE_DIGEST } else { BINARY_DIGEST };
    assert_eq!(format!("{:x}", Sha256::digest(bytes)), expected);
    // Retained unchanged Maven Central data, not authored example coordinates.
    verify_sha256(
        Artifact {
            bytes: bytes.to_vec(),
            provenance: Provenance {
                coordinate: Coordinate::new(
                    "org.apache.commons",
                    "commons-lang3",
                    "3.17.0",
                    source.then_some("sources"),
                    "jar",
                )
                .unwrap(),
                repository_root: project().join("crates/ktlsp-binary/fixtures/t017c1"),
                artifact_path: fixture(source),
                byte_length: bytes.len(),
                checksum: ChecksumStatus::Unverified,
            },
        },
        Sha256::digest(bytes).into(),
    )
    .unwrap()
}
fn import_path(path: &Path) -> String {
    format!(
        "<WORKSPACE>/{}",
        path.strip_prefix(project())
            .unwrap()
            .to_str()
            .unwrap()
            .replace(std::path::MAIN_SEPARATOR, "/")
    )
}
fn library(name: &str, binaries: &[PathBuf]) -> Value {
    let mut roots: Vec<_> = binaries
        .iter()
        .map(|path| json!({"path": import_path(path)}))
        .collect();
    roots.push(json!({"path": import_path(&fixture(true)), "type":"SOURCES"}));
    json!({"name":name,"type":null,"roots":roots})
}
fn edge(name: &str) -> Value {
    json!({"type":"library","name":name,"scope":"compile"})
}
fn model(edges: Vec<Value>, libraries: Vec<Value>) -> Value {
    json!({"modules":[
        {"name":"app","contentRoots":[{"path":"<WORKSPACE>/not-the-source-boundary","sourceRoots":[
            {"path":"<WORKSPACE>/docs/specs/reference-binding-acceptance","type":"kotlin-source"},
            {"path":"<WORKSPACE>/docs/specs/lowering-acceptance","type":"kotlin-test"}
        ]}],"dependencies":edges},
        {"name":"isolated","contentRoots":[{"path":"<WORKSPACE>/tools/oracle/fixtures/official-pr174-symbols","sourceRoots":[
            {"path":"<WORKSPACE>/tools/oracle/fixtures/official-pr174-symbols","type":"kotlin-source"}
        ]}]}
    ],"libraries":libraries})
}
fn import(value: Value) -> v2::ImportedWorkspace {
    v2::from_json(project(), &serde_json::to_vec(&value).unwrap()).unwrap()
}
fn binding<'a>(
    library: &'a str,
    binary_root: usize,
    artifact: &'a VerifiedArtifact,
) -> ArtifactBinding<'a> {
    ArtifactBinding {
        library,
        binary_root,
        artifact,
        kind: ArchiveKind::Jar,
    }
}

#[test]
fn genuine_class_owner_identity_and_owning_library_source_composition() {
    let binary = genuine(false);
    let sources = genuine(true);
    let mut workspace = import(model(
        vec![
            json!({"type":"moduleSource"}),
            json!({"type":"library","name":"commons","scope":"compile","isExported":true}),
        ],
        vec![library("commons", &[fixture(false)])],
    ));
    // Imported JSON uses admitted forward slashes, while the query and retained
    // provenance use normalized host-native PathBufs, including Windows '\\'.
    #[cfg(windows)]
    {
        assert!(source().to_str().unwrap().contains('\\'));
        assert!(
            binary
                .provenance()
                .artifact_path
                .to_str()
                .unwrap()
                .contains('\\')
        );
    }
    // Public mutable sidecars are not authority for ownership, order, or paths.
    workspace.modules.clear();
    workspace.libraries.clear();
    let bindings = [binding("commons", 0, &binary)];
    let mut scratch = Vec::new();
    let result = select_class(&workspace, &source(), BUILDER, 8, &bindings, &mut scratch).unwrap();
    assert_eq!(result.owner.module.id, "app");
    assert_eq!(result.owner.source_root.kind, SourceRootKind::Production);
    let selected = result.class.unwrap();
    assert_eq!(selected.library.id, "commons");
    assert_eq!(
        (selected.dependency, selected.binary_root, selected.exported),
        (0, 0, true)
    );
    assert_eq!(selected.import_path, fixture(false));
    assert!(std::ptr::eq(selected.artifact, &binary));
    assert_eq!(selected.artifact.digest(), binary.digest());
    assert_eq!(
        selected.artifact.provenance().coordinate.group(),
        "org.apache.commons"
    );
    assert_eq!(
        selected.artifact.provenance().coordinate.artifact(),
        "commons-lang3"
    );
    assert_eq!(
        selected.artifact.provenance().coordinate.version(),
        "3.17.0"
    );
    assert_eq!(selected.artifact.provenance().artifact_path, fixture(false));
    assert_eq!(selected.selected.origin.root, 0);
    assert_eq!(selected.selected.origin.rank, ClassRank::Base);
    let class = &selected.selected.class;
    assert_eq!(
        (
            class.major,
            class.access_flags,
            class.fields.len(),
            class.methods.len()
        ),
        (52, 49, 4, 7)
    );
    let Constant::Class(name) = class.constants[usize::from(class.this_class)] else {
        panic!("class identity")
    };
    assert!(
        class
            .utf16(name)
            .unwrap()
            .iter()
            .copied()
            .eq(BUILDER.encode_utf16())
    );
    let binary_index = index_zip_directory(selected.artifact).unwrap();
    assert_eq!(
        binary_index.entries()[selected.selected.origin.ordinal].raw_name(),
        format!("{BUILDER}.class").as_bytes()
    );
    // Use the selected library's source declaration, not a workspace-global JAR.
    assert_eq!(
        selected.library.sources,
        [sources.provenance().artifact_path.clone()]
    );
    let source_index = index_zip_directory(&sources).unwrap();
    let mut source_scratch = Vec::new();
    let attached = source_attachment::attach(class, &source_index, &mut source_scratch).unwrap();
    assert_eq!(
        attached.entry.raw_name(),
        b"org/apache/commons/lang3/AppendableJoiner.java"
    );
    assert_eq!((attached.ordinal, attached.bytes.len()), (27, 11_798));
    assert_eq!(
        format!("{:x}", Sha256::digest(attached.bytes)),
        "bb5a9762fa8614c2a49e5ccb770f1378c60d5fdb7b8679039c85bdabe7f57315"
    );
}

#[cfg(windows)]
#[test]
fn canonical_windows_artifact_keeps_provenance_and_binds_ordinary_import() {
    use std::path::{Component, Prefix};
    let original = genuine(false);
    let canonical = std::fs::canonicalize(fixture(false)).unwrap();
    assert!(
        matches!(canonical.components().next(), Some(Component::Prefix(prefix))
        if matches!(prefix.kind(), Prefix::VerbatimDisk(_) | Prefix::VerbatimUNC(_, _)))
    );
    let mut provenance = original.provenance().clone();
    provenance.artifact_path = canonical.clone();
    let artifact = verify_sha256(
        Artifact {
            bytes: original.bytes().to_vec(),
            provenance,
        },
        *original.digest(),
    )
    .unwrap();
    let workspace = import(model(
        vec![edge("commons")],
        vec![library("commons", &[fixture(false)])],
    ));
    let bindings = [binding("commons", 0, &artifact)];
    let mut scratch = Vec::new();
    let result = select_class(&workspace, &source(), BUILDER, 8, &bindings, &mut scratch).unwrap();
    assert_eq!(result.owner.module.id, "app");
    let selected = result.class.unwrap();
    assert_eq!(selected.selected.class.major, 52);
    assert_eq!(selected.import_path, fixture(false));
    assert_eq!(selected.artifact.provenance().artifact_path, canonical);
    assert_eq!(selected.artifact.digest(), original.digest());
}

#[test]
fn owner_and_dependency_order_control_visibility_precedence_and_complete_absence() {
    let binary = genuine(false);
    let bindings = [binding("first", 0, &binary), binding("second", 0, &binary)];
    for names in [["first", "second"], ["second", "first"]] {
        let workspace = import(model(
            names.iter().map(|n| edge(n)).collect(),
            names
                .iter()
                .map(|n| library(n, &[fixture(false)]))
                .collect(),
        ));
        let mut scratch = Vec::new();
        let found = select_class(&workspace, &source(), BUILDER, 8, &bindings, &mut scratch)
            .unwrap()
            .class
            .unwrap();
        assert_eq!(found.library.id, names[0]);
        assert_eq!(found.selected.origin.root, 0);
        assert_eq!(found.dependency, 0);
        let absent = select_class(
            &workspace,
            &source(),
            "missing/Type",
            23,
            &bindings,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(absent.owner.module.id, "app");
        assert!(absent.class.is_none());
        let isolated: PathBuf = project()
            .join("tools/oracle/fixtures/official-pr174-symbols/declarations.kt")
            .components()
            .collect();
        let absent =
            select_class(&workspace, &isolated, BUILDER, 8, &bindings, &mut scratch).unwrap();
        assert_eq!(absent.owner.module.id, "isolated");
        assert!(
            absent.class.is_none(),
            "unrelated bindings must not grant visibility"
        );
    }
}

#[test]
fn binding_completeness_is_checked_before_early_match_or_successful_miss() {
    let binary = genuine(false);
    let workspace = import(model(
        vec![edge("commons")],
        vec![library("commons", &[fixture(false), fixture(false)])],
    ));
    let early = binding("commons", 0, &binary);
    let late = binding("commons", 1, &binary);
    for name in [BUILDER, "missing/Type"] {
        let mut scratch = Vec::new();
        assert!(matches!(
            select_class(&workspace, &source(), name, 8, &[early], &mut scratch),
            Err(Error::MissingBinding { binary_root: 1, .. })
        ));
        assert!(matches!(
            select_class(
                &workspace,
                &source(),
                name,
                8,
                &[early, late, late],
                &mut scratch
            ),
            Err(Error::DuplicateBinding { binary_root: 1, .. })
        ));
        let wrong_ordinal = ArtifactBinding {
            binary_root: 2,
            ..late
        };
        assert!(matches!(
            select_class(
                &workspace,
                &source(),
                name,
                8,
                &[early, wrong_ordinal],
                &mut scratch
            ),
            Err(Error::UnboundBinding { binding: 1 })
        ));
        assert!(matches!(
            select_class(
                &workspace,
                &source(),
                name,
                8,
                &[early, late, wrong_ordinal],
                &mut scratch
            ),
            Err(Error::UnboundBinding { binding: 2 })
        ));
    }
    let sources = genuine(true);
    assert!(matches!(
        select_class(
            &workspace,
            &source(),
            BUILDER,
            8,
            &[early, binding("commons", 1, &sources)],
            &mut Vec::new()
        ),
        Err(Error::BindingPath { binary_root: 1, .. })
    ));
    let complete = [early, late];
    assert!(
        select_class(
            &workspace,
            &source(),
            BUILDER,
            8,
            &complete,
            &mut Vec::new()
        )
        .unwrap()
        .class
        .is_some()
    );
}

#[test]
fn owner_policy_rejects_test_unowned_and_every_unsupported_edge_before_match() {
    let binary = genuine(false);
    let bindings = [binding("commons", 0, &binary)];
    let base = model(
        vec![edge("commons")],
        vec![library("commons", &[fixture(false)])],
    );
    let workspace = import(base.clone());
    let test: PathBuf = project()
        .join("docs/specs/lowering-acceptance/declarations.kt")
        .components()
        .collect();
    assert!(matches!(
        select_class(&workspace, &test, BUILDER, 8, &bindings, &mut Vec::new()),
        Err(Error::TestSource)
    ));
    let outside: PathBuf = project()
        .join("docs/specs/reference-binding-acceptance-extra/No.kt")
        .components()
        .collect();
    assert!(matches!(
        select_class(&workspace, &outside, BUILDER, 8, &bindings, &mut Vec::new()),
        Err(Error::UnownedSource)
    ));
    let unsupported = [
        json!({"type":"module","name":"isolated","scope":"compile"}),
        json!({"type":"library","name":"commons","scope":"test"}),
        json!({"type":"library","name":"commons","scope":"runtime"}),
        json!({"type":"library","name":"commons","scope":"provided"}),
        json!({"type":"inheritedSdk"}),
        json!({"type":"sdk","name":"JDK","kind":"JavaSDK"}),
    ];
    for edge in unsupported {
        let mut input = base.clone();
        input["modules"][0]["dependencies"]
            .as_array_mut()
            .unwrap()
            .push(edge);
        input["sdks"] = json!([{"name":"JDK","type":"JavaSDK","version":"25","homePath":null,"additionalData":""}]);
        let workspace = import(input);
        assert!(matches!(
            select_class(
                &workspace,
                &source(),
                BUILDER,
                8,
                &bindings,
                &mut Vec::new()
            ),
            Err(Error::UnsupportedDependency { dependency: 1 })
        ));
    }
}

// Authored in-memory ZIPs below are data, not additional downloaded/JVM fixtures.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 == 0 { 0 } else { 0xedb8_8320 };
        }
    }
    !crc
}
fn zip(entries: &[(&[u8], &[u8])]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut directory = Vec::new();
    for &(name, payload) in entries {
        let offset = bytes.len() as u32;
        let size = payload.len() as u32;
        let crc = crc32(payload);
        bytes.extend(b"PK\x03\x04");
        for n in [20u16, 0x800, 0, 0, 0] {
            bytes.extend(n.to_le_bytes());
        }
        for n in [crc, size, size] {
            bytes.extend(n.to_le_bytes());
        }
        bytes.extend((name.len() as u16).to_le_bytes());
        bytes.extend(0u16.to_le_bytes());
        bytes.extend(name);
        bytes.extend(payload);
        directory.extend(b"PK\x01\x02");
        for n in [20u16, 20, 0x800, 0, 0, 0] {
            directory.extend(n.to_le_bytes());
        }
        for n in [crc, size, size] {
            directory.extend(n.to_le_bytes());
        }
        directory.extend((name.len() as u16).to_le_bytes());
        for _ in 0..4 {
            directory.extend(0u16.to_le_bytes());
        }
        directory.extend(0u32.to_le_bytes());
        directory.extend(offset.to_le_bytes());
        directory.extend(name);
    }
    let offset = bytes.len() as u32;
    let size = directory.len() as u32;
    bytes.extend(directory);
    bytes.extend(b"PK\x05\x06");
    bytes.extend([0; 4]);
    for _ in 0..2 {
        bytes.extend((entries.len() as u16).to_le_bytes());
    }
    bytes.extend(size.to_le_bytes());
    bytes.extend(offset.to_le_bytes());
    bytes.extend([0; 2]);
    bytes
}
fn authored(bytes: Vec<u8>, path: &Path) -> VerifiedArtifact {
    let digest = Sha256::digest(&bytes).into();
    verify_sha256(
        Artifact {
            provenance: Provenance {
                coordinate: Coordinate::new(
                    "test.independently.authored",
                    "archive",
                    "1",
                    None,
                    "jar",
                )
                .unwrap(),
                repository_root: project().into(),
                artifact_path: path.into(),
                byte_length: bytes.len(),
                checksum: ChecksumStatus::Unverified,
            },
            bytes,
        },
        digest,
    )
    .unwrap()
}

#[test]
fn root_order_and_explicit_archive_kind_retain_real_selected_origin() {
    let empty_path = project().join("authored-empty.data");
    let empty = authored(zip(&[]), &empty_path);
    let binary = genuine(false);
    let workspace = import(model(
        vec![edge("commons")],
        vec![library("commons", &[empty_path, fixture(false)])],
    ));
    let bindings = [
        binding("commons", 1, &binary),
        binding("commons", 0, &empty),
    ];
    let mut scratch = Vec::new();
    let found = select_class(&workspace, &source(), BUILDER, 8, &bindings, &mut scratch)
        .unwrap()
        .class
        .unwrap();
    assert_eq!((found.binary_root, found.selected.origin.root), (1, 1));
    assert!(std::ptr::eq(found.artifact, &binary));
    let wrong_kind = ArtifactBinding {
        kind: ArchiveKind::Jmod,
        ..bindings[1]
    };
    assert!(matches!(
        select_class(
            &workspace,
            &source(),
            BUILDER,
            8,
            &[wrong_kind, bindings[0]],
            &mut Vec::new()
        ),
        Err(Error::Jmod { root: 0, .. })
    ));
    let class_bytes =
        include_bytes!("../../ktlsp-binary/fixtures/t017c1/AppendableJoiner$Builder.class.bin");
    let mut jmod_bytes = b"JM\x01\0".to_vec();
    jmod_bytes.extend(zip(&[(
        format!("classes/{BUILDER}.class").as_bytes(),
        class_bytes,
    )]));
    let jmod_path = project().join("authored-jmod.data");
    let jmod = authored(jmod_bytes, &jmod_path);
    let workspace = import(model(
        vec![edge("jmod")],
        vec![library("jmod", &[jmod_path])],
    ));
    let bindings = [ArtifactBinding {
        kind: ArchiveKind::Jmod,
        ..binding("jmod", 0, &jmod)
    }];
    let mut scratch = Vec::new();
    let selected = select_class(&workspace, &source(), BUILDER, 8, &bindings, &mut scratch)
        .unwrap()
        .class
        .unwrap();
    assert_eq!(selected.selected.origin.rank, ClassRank::Jmod);
    assert_eq!(selected.selected.class.major, 52);
}

#[test]
fn malformed_winning_archive_and_late_nonarchive_are_never_skipped() {
    let path = project().join("authored-broken.jar");
    let broken = authored(
        zip(&[(format!("{BUILDER}.class").as_bytes(), b"not a class")]),
        &path,
    );
    let genuine = genuine(false);
    let workspace = import(model(
        vec![edge("broken"), edge("commons")],
        vec![
            library("broken", std::slice::from_ref(&path)),
            library("commons", &[fixture(false)]),
        ],
    ));
    let bindings = [
        binding("commons", 0, &genuine),
        binding("broken", 0, &broken),
    ];
    assert!(matches!(
        select_class(
            &workspace,
            &source(),
            BUILDER,
            8,
            &bindings,
            &mut Vec::new()
        ),
        Err(Error::Classpath(ClasspathError::ClassPayload { .. }))
    ));
    let nonarchive = authored(b"not an archive".to_vec(), &path);
    let workspace = import(model(
        vec![edge("commons"), edge("broken")],
        vec![
            library("broken", &[path]),
            library("commons", &[fixture(false)]),
        ],
    ));
    let bindings = [
        binding("commons", 0, &genuine),
        binding("broken", 0, &nonarchive),
    ];
    assert!(matches!(
        select_class(
            &workspace,
            &source(),
            BUILDER,
            8,
            &bindings,
            &mut Vec::new()
        ),
        Err(Error::Jar { root: 1, .. })
    ));
}

#[test]
fn query_and_resource_boundaries_fail_closed() {
    let binary = genuine(false);
    let workspace = import(model(
        vec![edge("commons")],
        vec![library("commons", &[fixture(false)])],
    ));
    let bindings = [binding("commons", 0, &binary)];
    for path in [
        PathBuf::from("relative.kt"),
        source().parent().unwrap().join("./same-block.kt"),
        source()
            .parent()
            .unwrap()
            .join("../reference-binding-acceptance/same-block.kt"),
        PathBuf::from(format!(
            "{}//same-block.kt",
            source().parent().unwrap().display()
        )),
        PathBuf::from("/bad\0.kt"),
    ] {
        let mut scratch = vec![42];
        assert!(matches!(
            select_class(&workspace, &path, BUILDER, 8, &bindings, &mut scratch),
            Err(Error::InvalidSourcePath)
        ));
        assert!(scratch.is_empty());
    }
    assert!(matches!(
        select_class(
            &workspace,
            &source(),
            "../Class",
            8,
            &bindings,
            &mut Vec::new()
        ),
        Err(Error::Classpath(ClasspathError::InvalidName))
    ));
    assert!(matches!(
        select_class(
            &workspace,
            &source(),
            BUILDER,
            0,
            &bindings,
            &mut Vec::new()
        ),
        Err(Error::Classpath(ClasspathError::InvalidRelease))
    ));
    assert!(matches!(
        select_class(
            &workspace,
            &source(),
            BUILDER,
            7,
            &bindings,
            &mut Vec::new()
        ),
        Err(Error::Classpath(ClasspathError::IncompatibleVersion {
            major: 52,
            ..
        }))
    ));
    let empty_path = project().join("authored-empty.data");
    let empty = authored(zip(&[]), &empty_path);
    let input = |count| {
        import(model(
            vec![edge("empty")],
            vec![library("empty", &vec![empty_path.clone(); count])],
        ))
    };
    let exact = input(64);
    let all: Vec<_> = (0..64).map(|i| binding("empty", i, &empty)).collect();
    assert!(
        select_class(&exact, &source(), BUILDER, 8, &all, &mut Vec::new())
            .unwrap()
            .class
            .is_none()
    );
    assert!(matches!(
        select_class(&input(65), &source(), BUILDER, 8, &[], &mut Vec::new()),
        Err(Error::Limit("roots"))
    ));
    let too_many = vec![binding("unrelated", 0, &binary); 65_537];
    assert!(matches!(
        select_class(
            &workspace,
            &source(),
            BUILDER,
            8,
            &too_many,
            &mut Vec::new()
        ),
        Err(Error::Limit("bindings"))
    ));
    let huge_key = "x".repeat(16 * 1024 * 1024 + 1);
    assert!(matches!(
        select_class(
            &workspace,
            &source(),
            BUILDER,
            8,
            &[binding(&huge_key, 0, &binary)],
            &mut Vec::new()
        ),
        Err(Error::Limit("binding key bytes"))
    ));
}

#[test]
fn aggregate_entry_and_name_work_caps_apply_before_early_match_or_absence() {
    for (entry_name, count, field) in [
        (vec![b'x'], 16_384, "entries"),
        (vec![b'x'; 4096], 1024, "scan bytes"),
    ] {
        let path = project().join("authored-budget.data");
        let entries = vec![(entry_name.as_slice(), b"".as_slice()); count];
        let artifact = authored(zip(&entries), &path);
        for roots in [4, 5] {
            let workspace = import(model(
                vec![edge("budget")],
                vec![library("budget", &vec![path.clone(); roots])],
            ));
            let bindings: Vec<_> = (0..roots)
                .map(|i| binding("budget", i, &artifact))
                .collect();
            let mut scratch = Vec::new();
            let result = select_class(&workspace, &source(), BUILDER, 8, &bindings, &mut scratch);
            if roots == 4 {
                assert!(result.unwrap().class.is_none());
            } else {
                assert!(matches!(result, Err(Error::Limit(actual)) if actual == field));
            }
        }
    }
}

#[test]
fn selected_source_only_library_refuses_unbound_archive_but_unrelated_libraries_do_not_leak() {
    let binary = genuine(false);
    let workspace = import(model(
        vec![edge("source-only")],
        vec![library("source-only", &[])],
    ));
    for name in [BUILDER, "missing/Type"] {
        assert!(matches!(
            select_class(
                &workspace,
                &source(),
                name,
                8,
                &[binding("source-only", 0, &binary)],
                &mut Vec::new()
            ),
            Err(Error::UnboundBinding { binding: 0 })
        ));
        assert!(
            select_class(
                &workspace,
                &source(),
                name,
                8,
                &[binding("unrelated", usize::MAX, &binary)],
                &mut Vec::new()
            )
            .unwrap()
            .class
            .is_none()
        );
    }
}

#[cfg(not(windows))]
#[test]
fn unix_foreign_backslash_source_path_is_not_admitted_as_a_native_separator() {
    let workspace = import(model(Vec::new(), Vec::new()));
    let foreign = project().join(r"docs\specs\reference-binding-acceptance\same-block.kt");
    assert!(matches!(
        select_class(&workspace, &foreign, BUILDER, 8, &[], &mut Vec::new()),
        Err(Error::InvalidSourcePath)
    ));
}

#[test]
fn genuine_minimal_jcl_source_owner_uses_explicit_java24_java25_release() {
    // Genuine Eclipse JDT test libraries, not stock SDKs or Maven artifacts.
    for (release, bytes, expected) in [
        (24u16, include_bytes!("../../ktlsp-binary/fixtures/modern-class-versions/jclMin24.jar.bin").as_slice(), "49335cdc8f8e0a2a7f529e8c5167181ee67acf334150e23a2f0dd3a03c7c57db"),
        (25, include_bytes!("../../ktlsp-binary/fixtures/modern-class-versions/jclMin25.jar.bin").as_slice(), "8eb40b1a47696cf92b4a83a70fd07c885bea858a5dd907edc962ed48b74585da"),
    ] {
        assert_eq!(format!("{:x}", Sha256::digest(bytes)), expected);
        let path: PathBuf = project().join(format!("crates/ktlsp-binary/fixtures/modern-class-versions/jclMin{release}.jar.bin")).components().collect();
        let artifact = verify_sha256(Artifact {
            bytes: bytes.to_vec(),
            provenance: Provenance {
                coordinate: Coordinate::new("fixture", "eclipse-jdt-minimal-jcl", &release.to_string(), None, "jar").unwrap(),
                repository_root: path.parent().unwrap().into(),
                artifact_path: path.clone(),
                byte_length: bytes.len(),
                checksum: ChecksumStatus::Unverified,
            },
        }, Sha256::digest(bytes).into()).unwrap();
        let workspace = import(model(vec![edge("minimal-jcl")], vec![library("minimal-jcl", std::slice::from_ref(&path))]));
        let bindings = [binding("minimal-jcl", 0, &artifact)];
        let mut scratch = Vec::new();
        {
            let result = select_class(&workspace, &source(), "java/io/Serializable", release, &bindings, &mut scratch).unwrap();
            assert_eq!(result.owner.module.id, "app");
            assert_eq!(result.owner.source_root.kind, SourceRootKind::Production);
            let found = result.class.unwrap();
            assert_eq!(found.library.id, "minimal-jcl");
            assert_eq!(found.binary_root, 0);
            assert_eq!(found.import_path, path);
            assert!(std::ptr::eq(found.artifact, &artifact));
            assert_eq!(found.selected.origin.rank, ClassRank::Base);
            assert_eq!((found.selected.class.major, found.selected.class.minor), (release + 44, 0));
        }
        assert!(matches!(select_class(&workspace, &source(), "java/io/Serializable", release - 1, &bindings, &mut scratch), Err(Error::Classpath(ClasspathError::IncompatibleVersion { maximum, .. })) if maximum == u32::from(release + 43)));
    }
}

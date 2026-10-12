use ktlsp_project::*;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn base() -> PathBuf {
    std::env::temp_dir().join("ktlsp-project-fixture")
}
fn parse(value: Value) -> Result<ProjectModel, ProjectError> {
    from_json(
        base().join("ktlsp.json"),
        &serde_json::to_vec(&value).unwrap(),
    )
}
fn minimal() -> Value {
    json!({"schemaVersion":1,"modules":[{"id":"app"}]})
}
fn with_roots(roots: Value) -> Value {
    json!({"schemaVersion":1,"modules":[{"id":"app","sourceRoots":roots}]})
}
fn dependency(module: &str) -> Value {
    json!({"module":module,"scope":"compile"})
}

#[test]
fn fixture_retains_semantics_and_missing_artifacts() {
    let model = from_json(
        base().join("ktlsp.json"),
        include_bytes!("fixtures/multimodule.json"),
    )
    .unwrap();
    assert_eq!(
        model.modules[0].source_roots[0].path,
        base().join("src/main/kotlin")
    );
    assert_eq!(model.modules[0].source_roots[1].kind, SourceRootKind::Test);
    assert!(model.modules[0].source_roots[2].generated);
    assert_eq!(
        model.modules[1].source_roots[0].path,
        base().parent().unwrap().join("core/src")
    );
    assert_eq!(
        model.modules[0].dependencies[0].scope,
        DependencyScope::Provided
    );
    assert!(model.modules[0].dependencies[0].exported);
    assert_eq!(
        model.modules[0].language_settings.api_version.as_deref(),
        Some("2.2")
    );
    assert_eq!(
        model.libraries[0].binaries[0],
        base().join("lib/missing.jar")
    );
    assert_eq!(
        model.libraries[0].sources[0],
        base().join("sources/missing.zip")
    );
    assert_eq!(model.sdk_archives[0], base().join("sdk/jdk-data.zip"));
    let serialized = serde_json::to_value(&model).unwrap();
    assert_eq!(
        serialized["modules"][0]["dependencies"][0]["exported"],
        true
    );
    assert_eq!(serialized["modules"][0]["sourceRoots"][1]["kind"], "test");
    assert!(serialized.get("schemaVersion").is_some());
    assert!(serialized["modules"][0].get("languageSettings").is_some());
}

#[test]
fn defaults_and_empty_project() {
    let model = parse(minimal()).unwrap();
    assert!(model.libraries.is_empty());
    assert!(model.sdk_archives.is_empty());
    assert_eq!(
        model.modules[0].language_settings,
        LanguageSettings::default()
    );
    let model = parse(json!({"schemaVersion":1,"modules":[]})).unwrap();
    assert!(model.modules.is_empty());
    let model = parse(with_roots(json!([{"path":"src","kind":"test"}]))).unwrap();
    assert!(!model.modules[0].source_roots[0].generated);
    let model = parse(json!({"schemaVersion":1,"modules":[{"id":"a","dependencies":[{"module":"b","scope":"test"}]},{"id":"b"}]})).unwrap();
    assert!(!model.modules[0].dependencies[0].exported);
}

#[test]
fn malformed_schema_unknown_fields_and_required_fields() {
    let invalid = [
        json!({"modules":[]}),
        json!({"schemaVersion":1}),
        json!({"schemaVersion":1,"modules":[{}]}),
        json!({"schemaVersion":1,"modules":[],"libraries":[{"id":"lib"}]}),
        with_roots(json!([{"kind":"test"}])),
        with_roots(json!([{"path":"src"}])),
        with_roots(json!([{"path":"src","kind":"unknown"}])),
        with_roots(json!([{"path":"src","kind":"test","unexpected":true}])),
        json!({"schemaVersion":1,"modules":[{"id":"app","unexpected":true}]}),
        json!({"schemaVersion":1,"modules":[],"unexpected":true}),
        json!({"schemaVersion":1,"modules":[{"id":"a","dependencies":[{"module":"b"}]}]}),
        json!({"schemaVersion":1,"modules":[{"id":"a","dependencies":[{"scope":"compile"}]}]}),
        json!({"schemaVersion":1,"modules":[{"id":"a","dependencies":[{"module":"b","scope":"invalid"}]}]}),
        json!({"schemaVersion":1,"modules":[{"id":"a","languageSettings":{"extra":"x"}}]}),
        json!({"schemaVersion":1,"modules":[],"libraries":[{"id":"lib","binaries":[],"extra":true}]}),
        json!({"schemaVersion":1,"modules":[{"id":"a","libraries":[{"library":"lib","scope":"test"}]}]}),
    ];
    for value in invalid {
        assert!(
            matches!(parse(value.clone()), Err(ProjectError::Json(_))),
            "{value}"
        );
    }
    for bytes in [
        b"{".as_slice(),
        b"null",
        b"[]",
        b"{} {}",
        b"{\"schemaVersion\":1,\"schemaVersion\":1,\"modules\":[]}",
        b"\xff",
    ] {
        assert!(matches!(
            from_json(base().join("ktlsp.json"), bytes),
            Err(ProjectError::Json(_))
        ));
    }
}

#[test]
fn versions_are_syntactic_numeric_and_api_is_ordered_numerically() {
    for version in [0, 2, u64::MAX] {
        assert!(
            matches!(parse(json!({"schemaVersion":version,"modules":[]})), Err(ProjectError::UnsupportedSchemaVersion(v)) if v == version)
        );
    }
    for (field, value) in [
        ("languageVersion", "2"),
        ("apiVersion", "2.3.4"),
        ("languageVersion", "-1.2"),
        ("languageVersion", "٢.٣"),
        ("jvmTarget", "0"),
        ("jvmTarget", "00"),
        ("jvmTarget", "1.9"),
        ("jvmTarget", "-1"),
        ("jvmTarget", ""),
    ] {
        let mut project = minimal();
        project["modules"][0]["languageSettings"] = json!({field:value});
        assert!(
            matches!(parse(project), Err(ProjectError::InvalidVersion { .. })),
            "{field} {value}"
        );
    }
    for (language, api, valid) in [
        ("2.10", "2.9", true),
        ("2.9", "2.10", false),
        ("03.000", "3.0", true),
        ("2.99", "3.0", false),
        ("999999999999999999999.1", "2.3", true),
    ] {
        let mut project = minimal();
        project["modules"][0]["languageSettings"] =
            json!({"languageVersion":language,"apiVersion":api,"jvmTarget":"1.8"});
        assert_eq!(parse(project).is_ok(), valid, "{language} {api}");
    }
    let mut project = minimal();
    project["modules"][0]["languageSettings"] = json!({"apiVersion":"999.999","jvmTarget":"00021"});
    assert!(parse(project).is_ok());
}

#[test]
fn ids_namespaces_and_references() {
    for id in ["", " \n\t"] {
        assert!(matches!(
            parse(json!({"schemaVersion":1,"modules":[{"id":id}]})),
            Err(ProjectError::InvalidId { kind: "module", .. })
        ));
        assert!(matches!(
            parse(json!({"schemaVersion":1,"modules":[],"libraries":[{"id":id,"binaries":[]}]})),
            Err(ProjectError::InvalidId {
                kind: "library",
                ..
            })
        ));
    }
    assert!(matches!(
        parse(json!({"schemaVersion":1,"modules":[{"id":"a"},{"id":"a"}]})),
        Err(ProjectError::DuplicateId { kind: "module", .. })
    ));
    assert!(matches!(
        parse(
            json!({"schemaVersion":1,"modules":[],"libraries":[{"id":"a","binaries":[]},{"id":"a","binaries":[]}]})
        ),
        Err(ProjectError::DuplicateId {
            kind: "library",
            ..
        })
    ));
    assert!(parse(json!({"schemaVersion":1,"modules":[{"id":"a","libraries":["a"]}],"libraries":[{"id":"a","binaries":[]}]})).is_ok());
    assert!(matches!(
        parse(json!({"schemaVersion":1,"modules":[{"id":"a","libraries":["missing"]}]})),
        Err(ProjectError::UnknownReference {
            kind: "library",
            ..
        })
    ));
    assert!(matches!(
        parse(
            json!({"schemaVersion":1,"modules":[{"id":"a","dependencies":[dependency("missing")]}]})
        ),
        Err(ProjectError::UnknownReference { kind: "module", .. })
    ));
}

#[test]
fn graph_rejects_self_duplicates_and_cycles_including_disconnected_cycles() {
    assert!(matches!(
        parse(json!({"schemaVersion":1,"modules":[{"id":"a","dependencies":[dependency("a")]}]})),
        Err(ProjectError::SelfDependency(_))
    ));
    assert!(matches!(
        parse(
            json!({"schemaVersion":1,"modules":[{"id":"a","dependencies":[dependency("b"),{"module":"b","scope":"test","exported":true}]},{"id":"b"}]})
        ),
        Err(ProjectError::DuplicateDependency { .. })
    ));
    for modules in [
        json!([{"id":"a","dependencies":[dependency("b")]},{"id":"b","dependencies":[dependency("a")]}]),
        json!([{"id":"free"},{"id":"a","dependencies":[dependency("b")]},{"id":"b","dependencies":[dependency("c")]},{"id":"c","dependencies":[dependency("a")]}]),
    ] {
        assert!(matches!(
            parse(json!({"schemaVersion":1,"modules":modules})),
            Err(ProjectError::DependencyCycle)
        ));
    }
}

#[test]
fn max_length_chain_is_iterative_and_accepts_all_scopes() {
    let modules: Vec<_> = (0..MAX_MODULES).map(|i| {
        let dependencies = if i + 1 < MAX_MODULES { vec![json!({"module":format!("m{}",i+1),"scope":(["compile","test","runtime","provided"][i%4])})] } else { vec![] };
        json!({"id":format!("m{i}"),"dependencies":dependencies})
    }).collect();
    assert_eq!(
        parse(json!({"schemaVersion":1,"modules":modules}))
            .unwrap()
            .modules
            .len(),
        MAX_MODULES
    );
}

#[test]
fn lexical_paths_are_absolute_missing_parent_relative_and_root_bounded() {
    let absolute = base().join("absolute/./../file");
    let model = parse(with_roots(json!([{"path":absolute,"kind":"production"},{"path":"../peer/../../missing","kind":"production"}]))).unwrap();
    assert_eq!(model.modules[0].source_roots[0].path, base().join("file"));
    assert!(
        model.modules[0]
            .source_roots
            .iter()
            .all(|r| r.path.is_absolute())
    );
    let relative = from_json(
        Path::new("somewhere/../ktlsp.json"),
        &serde_json::to_vec(&with_roots(json!([{"path":"src","kind":"production"}]))).unwrap(),
    )
    .unwrap();
    assert_eq!(
        relative.modules[0].source_roots[0].path,
        std::env::current_dir().unwrap().join("src")
    );
    for path in ["", "a\0b"] {
        assert!(matches!(
            parse(with_roots(json!([{"path":path,"kind":"test"}]))),
            Err(ProjectError::InvalidPath(_))
        ));
        assert!(matches!(
            parse(json!({"schemaVersion":1,"modules":[],"sdkArchives":[path]})),
            Err(ProjectError::InvalidPath(_))
        ));
        assert!(matches!(
            parse(
                json!({"schemaVersion":1,"modules":[],"libraries":[{"id":"a","binaries":[path]}]})
            ),
            Err(ProjectError::InvalidPath(_))
        ));
        assert!(matches!(
            parse(
                json!({"schemaVersion":1,"modules":[],"libraries":[{"id":"a","binaries":[],"sources":[path]}]})
            ),
            Err(ProjectError::InvalidPath(_))
        ));
    }
    assert!(matches!(
        from_json("", b"{\"schemaVersion\":1,\"modules\":[]}"),
        Err(ProjectError::InvalidPath(_))
    ));
}

#[test]
fn ownership_handles_component_boundaries_normalization_and_nested_ancestors() {
    for roots in [
        json!([{"path":"src","kind":"test"},{"path":"src/.","kind":"test"}]),
        json!([{"path":"src","kind":"test"},{"path":"src/nested","kind":"production"}]),
        json!([{"path":"src","kind":"production"},{"path":"src/nested","kind":"production","generated":true}]),
    ] {
        assert!(matches!(
            parse(with_roots(roots)),
            Err(ProjectError::SourceRootOverlap { .. })
        ));
    }
    assert!(
        parse(with_roots(
            json!([{"path":"src","kind":"test"},{"path":"src/nested","kind":"test"}])
        ))
        .is_ok()
    );
    for other in [
        "src",
        "src/nested",
        "src/nested/deeper",
        "./src/other/../nested",
    ] {
        assert!(matches!(
            parse(
                json!({"schemaVersion":1,"modules":[{"id":"a","sourceRoots":[{"path":"src","kind":"test"},{"path":"src/nested","kind":"test"}]},{"id":"b","sourceRoots":[{"path":other,"kind":"test"}]}]})
            ),
            Err(ProjectError::SourceRootOverlap { .. })
        ));
    }
    assert!(parse(json!({"schemaVersion":1,"modules":[{"id":"a","sourceRoots":[{"path":"src","kind":"test"}]},{"id":"b","sourceRoots":[{"path":"src-other","kind":"test"}]}]})).is_ok());
}

#[test]
fn bounded_manifest_read_and_io_errors() {
    assert!(matches!(
        from_json(
            base().join("ktlsp.json"),
            &vec![b' '; MAX_MANIFEST_BYTES + 1]
        ),
        Err(ProjectError::BudgetExceeded {
            budget: Budget::ManifestBytes,
            ..
        })
    ));
    assert!(matches!(
        load_manifest(base().join("does-not-exist.json")),
        Err(ProjectError::Io(_))
    ));
    let path = std::env::temp_dir().join(format!(
        "ktlsp-project-{}-oversized.json",
        std::process::id()
    ));
    std::fs::write(&path, vec![b' '; MAX_MANIFEST_BYTES + 1]).unwrap();
    let result = load_manifest(&path);
    std::fs::remove_file(path).unwrap();
    assert!(matches!(
        result,
        Err(ProjectError::BudgetExceeded {
            budget: Budget::ManifestBytes,
            ..
        })
    ));
}

#[test]
fn collection_budgets_run_before_typed_allocation_and_are_aggregate() {
    let cases = [
        (
            Budget::Modules,
            MAX_MODULES,
            json!({"schemaVersion":1,"modules":vec![json!(null);MAX_MODULES+1]}),
        ),
        (
            Budget::Libraries,
            MAX_LIBRARIES,
            json!({"schemaVersion":1,"modules":[],"libraries":vec![json!(null);MAX_LIBRARIES+1]}),
        ),
        (
            Budget::SourceRoots,
            MAX_SOURCE_ROOTS,
            json!({"schemaVersion":1,"modules":[{"id":"a","sourceRoots":vec![json!(null);MAX_SOURCE_ROOTS/2]},{"id":"b","sourceRoots":vec![json!(null);MAX_SOURCE_ROOTS/2+1]}]}),
        ),
        (
            Budget::Dependencies,
            MAX_DEPENDENCIES,
            json!({"schemaVersion":1,"modules":[{"id":"a","dependencies":vec![json!(null);MAX_DEPENDENCIES/2]},{"id":"b","libraries":vec![json!(null);MAX_DEPENDENCIES/2+1]}]}),
        ),
        (
            Budget::ArchivePaths,
            MAX_ARCHIVE_PATHS,
            json!({"schemaVersion":1,"modules":[],"libraries":[{"id":"a","binaries":vec![json!(null);MAX_ARCHIVE_PATHS/3],"sources":vec![json!(null);MAX_ARCHIVE_PATHS/3]}],"sdkArchives":vec![json!(null);MAX_ARCHIVE_PATHS-2*(MAX_ARCHIVE_PATHS/3)+1]}),
        ),
    ];
    for (budget, limit, value) in cases {
        assert!(
            matches!(parse(value), Err(ProjectError::BudgetExceeded { budget:b,limit:l }) if b == budget && l == limit),
            "{budget:?}"
        );
    }
    let model = parse(json!({"schemaVersion":1,"modules":[],"libraries":[{"id":"a","binaries":vec!["a";MAX_ARCHIVE_PATHS]}]})).unwrap();
    assert_eq!(model.libraries[0].binaries.len(), MAX_ARCHIVE_PATHS);
    let model = parse(json!({"schemaVersion":1,"modules":[],"libraries":(0..MAX_LIBRARIES).map(|i|json!({"id":format!("l{i}"),"binaries":[]})).collect::<Vec<_>>()})).unwrap();
    assert_eq!(model.libraries.len(), MAX_LIBRARIES);
}

#[test]
fn arbitrary_truncations_bytes_and_deep_json_return_without_panicking() {
    let fixture = include_bytes!("fixtures/multimodule.json");
    for end in 0..fixture
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .unwrap()
        + 1
    {
        assert!(from_json(base().join("ktlsp.json"), &fixture[..end]).is_err());
    }
    for byte in 0..=255 {
        let _ = from_json(base().join("ktlsp.json"), &[byte; 256]);
    }
    let deeply_nested = format!("{}0{}", "[".repeat(10_000), "]".repeat(10_000));
    assert!(matches!(
        from_json(base().join("ktlsp.json"), deeply_nested.as_bytes()),
        Err(ProjectError::Json(_))
    ));
}

#[test]
fn duplicate_library_edges_and_all_boundary_counts() {
    assert!(matches!(
        parse(
            json!({"schemaVersion":1,"modules":[{"id":"a","libraries":["lib","lib"]}],"libraries":[{"id":"lib","binaries":[]}]})
        ),
        Err(ProjectError::DuplicateLibraryReference { .. })
    ));
    let libraries: Vec<_> = (0..MAX_LIBRARIES)
        .map(|i| json!({"id":format!("l{i}"),"binaries":[]}))
        .collect();
    let references: Vec<_> = (0..MAX_LIBRARIES).map(|i| format!("l{i}")).collect();
    let modules: Vec<_> = (0..MAX_DEPENDENCIES / MAX_LIBRARIES)
        .map(|i| json!({"id":format!("m{i}"),"libraries":references}))
        .collect();
    let model = parse(json!({"schemaVersion":1,"modules":modules,"libraries":libraries})).unwrap();
    assert_eq!(
        model
            .modules
            .iter()
            .map(|m| m.libraries.len())
            .sum::<usize>(),
        MAX_DEPENDENCIES
    );
    let roots: Vec<_> = (0..MAX_SOURCE_ROOTS)
        .map(|i| json!({"path":format!("src/r{i}"),"kind":"production"}))
        .collect();
    assert_eq!(
        parse(with_roots(json!(roots))).unwrap().modules[0]
            .source_roots
            .len(),
        MAX_SOURCE_ROOTS
    );
    let mut exactly_bytes = serde_json::to_vec(&minimal()).unwrap();
    exactly_bytes.resize(MAX_MANIFEST_BYTES, b' ');
    assert!(from_json(base().join("ktlsp.json"), &exactly_bytes).is_ok());
}

#[cfg(unix)]
#[test]
fn symlink_aliases_are_retained_and_require_later_filesystem_validation() {
    use std::os::unix::fs::symlink;
    let directory =
        std::env::temp_dir().join(format!("ktlsp-project-{}-symlink", std::process::id()));
    std::fs::create_dir_all(directory.join("source")).unwrap();
    symlink("source", directory.join("alias")).unwrap();
    let bytes = serde_json::to_vec(&json!({"schemaVersion":1,"modules":[{"id":"a","sourceRoots":[{"path":"source","kind":"production"}]},{"id":"b","sourceRoots":[{"path":"alias","kind":"production"}]}]})).unwrap();
    let result = from_json(directory.join("ktlsp.json"), &bytes);
    std::fs::remove_dir_all(&directory).unwrap();
    let model = result.unwrap();
    assert_ne!(
        model.modules[0].source_roots[0].path,
        model.modules[1].source_roots[0].path
    );
    assert_eq!(
        model.modules[1].source_roots[0].path,
        directory.join("alias")
    );
}

#[test]
fn load_manifest_reads_fixture_from_its_own_parent() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multimodule.json");
    let model = load_manifest(&path).unwrap();
    assert_eq!(
        model.modules[0].source_roots[0].path,
        path.parent().unwrap().join("src/main/kotlin")
    );
}

#[cfg(unix)]
#[test]
fn unreadable_manifest_returns_io_error() {
    use std::os::unix::fs::PermissionsExt;
    let path = std::env::temp_dir().join(format!(
        "ktlsp-project-{}-unreadable.json",
        std::process::id()
    ));
    std::fs::write(&path, serde_json::to_vec(&minimal()).unwrap()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let result = load_manifest(&path);
    // Restore permissions before cleanup, even if running with elevated OS rights.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::remove_file(path).unwrap();
    // Unix root can read mode 000 files; success is allowed only when its OS
    // permissions actually permit the read. Normal users must get PermissionDenied.
    if let Err(error) = result {
        assert!(
            matches!(error, ProjectError::Io(e) if e.kind() == std::io::ErrorKind::PermissionDenied)
        );
    }
}

use ktlsp_project::{Budget, DependencyScope, ProjectError, v2, *};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const ORDERED: &[u8] = include_bytes!("../fixtures/t021b/ordered.json");

fn base() -> PathBuf {
    std::env::temp_dir().join("ktlsp-project-v2/ktlsp.json")
}
fn minimal() -> Value {
    json!({"schemaVersion":2,"modules":[{"id":"app"}]})
}
fn parse(value: Value) -> Result<v2::ProjectModel, v2::Error> {
    v2::from_json(base(), &serde_json::to_vec(&value).unwrap())
}
fn edge(kind: &str, target: &str, scope: &str, exported: bool) -> Value {
    json!({"type":kind, kind:target,"scope":scope,"exported":exported})
}

#[test]
fn ordered_fixture_roundtrips_all_scopes_exports_and_namespaces() {
    let model = v2::from_json(base(), ORDERED).unwrap();
    let input: Value = serde_json::from_slice(ORDERED).unwrap();
    let serialized = serde_json::to_value(&model).unwrap();
    assert_eq!(
        serialized["modules"][0]["dependencies"],
        input["modules"][0]["dependencies"]
    );
    let mut expected = Vec::new();
    for scope in [
        DependencyScope::Compile,
        DependencyScope::Test,
        DependencyScope::Runtime,
        DependencyScope::Provided,
    ] {
        for exported in [false, true] {
            expected.push(v2::Dependency::Library {
                library: "core".into(),
                scope,
                exported,
            });
        }
    }
    assert_eq!(
        model.modules[0]
            .dependencies
            .iter()
            .filter(|d| matches!(d, v2::Dependency::Library { .. }))
            .cloned()
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        model.modules[0].source_roots[0].path,
        base().parent().unwrap().join("src/main")
    );
    assert_eq!(
        model.modules[1].source_roots[0].path,
        base().parent().unwrap().parent().unwrap().join("core/src")
    );
    assert!(model.modules[1].source_roots[0].generated);
    assert_eq!(
        model.libraries[0].binaries[0],
        base().parent().unwrap().join("lib/stdlib.jar")
    );
    assert_eq!(
        model.libraries[0].sources[0],
        base().parent().unwrap().join("sources/stdlib.zip")
    );
    assert_eq!(
        model.sdk_archives[0],
        base().parent().unwrap().join("sdk/api.zip")
    );
    assert_eq!(
        v2::from_json(base(), &serde_json::to_vec(&model).unwrap()).unwrap(),
        model
    );
    assert!(
        v2::from_json(
            base(),
            include_bytes!("../fixtures/t021b/contract-example.json")
        )
        .is_ok()
    );
}

#[test]
fn v1_stays_strict_and_v2_does_not_accept_v1_fields_or_version() {
    assert!(matches!(
        from_json(base(), br#"{"schemaVersion":2,"modules":[]}"#),
        Err(ProjectError::UnsupportedSchemaVersion(2))
    ));
    assert!(from_json(base(), ORDERED).is_err());
    let fixture = include_bytes!("fixtures/multimodule.json");
    let v1 = from_json(base(), fixture).unwrap();
    assert_eq!(
        from_json(base(), &serde_json::to_vec(&v1).unwrap()).unwrap(),
        v1
    );
    assert!(v2::from_json(base(), fixture).is_err());
    for version in [0, 1, 3, u64::MAX] {
        let error = parse(json!({"schemaVersion":version,"modules":[]})).unwrap_err();
        assert!(matches!(error, v2::Error::UnsupportedSchemaVersion(v) if v == version));
        assert!(error.to_string().ends_with("expected 2"));
    }
    assert!(parse(json!({"schemaVersion":2,"modules":[{"id":"app","libraries":[]}]})).is_err());
    let model = parse(minimal()).unwrap();
    assert!(model.modules[0].dependencies.is_empty());
    let model = parse(json!({"schemaVersion":2,"modules":[{"id":"a","dependencies":[{"type":"library","library":"a","scope":"test"}]}],"libraries":[{"id":"a","binaries":[]}]})).unwrap();
    assert!(matches!(
        &model.modules[0].dependencies[0],
        v2::Dependency::Library {
            exported: false,
            ..
        }
    ));
}

#[test]
fn strict_json_and_tagged_variants_reject_malformed_unknown_and_duplicate_keys() {
    for value in [
        json!({"modules":[]}),
        json!({"schemaVersion":2}),
        json!({"schemaVersion":2,"modules":[],"unknown":1}),
        json!({"schemaVersion":2,"modules":[{"id":"a","unknown":1}]}),
        json!({"schemaVersion":2,"modules":[],"libraries":[{"id":"l","binaries":[],"unknown":1}]}),
    ] {
        assert!(matches!(
            parse(value),
            Err(v2::Error::Validation(ProjectError::Json(_)))
        ));
    }
    for dependency in [
        json!({"type":"sdk","library":"l","scope":"compile"}),
        json!({"type":"library","library":"l","scope":"compile","module":"a"}),
        json!({"type":"module","module":"a","scope":"compile","library":"l"}),
        json!({"type":"library","library":"l","scope":"compile","extra":1}),
        json!({"library":"l","scope":"compile"}),
        json!({"type":"library","scope":"compile"}),
        json!({"type":"library","library":"l"}),
        json!({"type":"library","library":"l","scope":"other"}),
        json!({"type":"library","library":"l","scope":"compile","exported":null}),
        json!({"type":"library","library":"l","scope":"compile","exported":"true"}),
    ] {
        assert!(matches!(
            parse(json!({"schemaVersion":2,"modules":[{"id":"a","dependencies":[dependency]}]})),
            Err(v2::Error::Validation(ProjectError::Json(_)))
        ));
    }
    for bytes in [
        br#"{"schemaVersion":2,"schemaVersion":2,"modules":[]}"#.as_slice(),
        br#"{"schemaVersion":2,"modules":[{"id":"a","id":"b"}]}"#,
        br#"{"schemaVersion":2,"modules":[{"id":"a","dependencies":[{"type":"library","type":"library","library":"l","scope":"compile"}]}]}"#,
        br#"{"schemaVersion":2,"modules":[{"id":"a","dependencies":[{"type":"library","library":"l","library":"l","scope":"compile"}]}]}"#,
        br#"{"schemaVersion":2,"modules":[{"id":"a","dependencies":[{"type":"library","library":"l","scope":"compile","scope":"test"}]}]}"#,
        b"{} {}", b"null", b"[]", b"\xff", br#"{"schemaVersion":2,"modules":[],"sdkArchives":["\uD800"]}"#,
    ] {
        assert!(matches!(v2::from_json(base(), bytes), Err(v2::Error::Validation(ProjectError::Json(_)))), "{bytes:?}");
    }
}

#[test]
fn ids_references_and_tuple_duplicates_are_validated_in_separate_namespaces() {
    for id in ["", " \t\n"] {
        assert!(matches!(
            parse(json!({"schemaVersion":2,"modules":[{"id":id}]})),
            Err(v2::Error::Validation(ProjectError::InvalidId {
                kind: "module",
                ..
            }))
        ));
        assert!(matches!(
            parse(json!({"schemaVersion":2,"modules":[],"libraries":[{"id":id,"binaries":[]}]})),
            Err(v2::Error::Validation(ProjectError::InvalidId {
                kind: "library",
                ..
            }))
        ));
    }
    for (kind, expected) in [("module", "module"), ("library", "library")] {
        let mut value = json!({"schemaVersion":2,"modules":[{"id":"a","dependencies":[edge(kind,"missing","test",true)]}]});
        assert!(
            matches!(parse(value.clone()), Err(v2::Error::Validation(ProjectError::UnknownReference { kind, .. })) if kind == expected)
        );
        // An identically named ID in the opposite namespace cannot satisfy a reference.
        if kind == "module" {
            value["libraries"] = json!([{"id":"missing","binaries":[]}]);
        } else {
            value["modules"]
                .as_array_mut()
                .unwrap()
                .push(json!({"id":"missing"}));
        }
        assert!(matches!(
            parse(value),
            Err(v2::Error::Validation(ProjectError::UnknownReference { .. }))
        ));
        let repeated = edge(kind, "b", "compile", false);
        let value = json!({"schemaVersion":2,"modules":[{"id":"a","dependencies":[repeated.clone(), repeated]},{"id":"b"}],"libraries":[{"id":"b","binaries":[]}]});
        assert!(matches!(
            parse(value),
            Err(v2::Error::Validation(
                ProjectError::DuplicateDependency { .. }
                    | ProjectError::DuplicateLibraryReference { .. }
            ))
        ));
    }
    assert!(matches!(
        parse(json!({"schemaVersion":2,"modules":[{"id":"a"},{"id":"a"}]})),
        Err(v2::Error::Validation(ProjectError::DuplicateId {
            kind: "module",
            ..
        }))
    ));
    assert!(matches!(
        parse(
            json!({"schemaVersion":2,"modules":[],"libraries":[{"id":"a","binaries":[]},{"id":"a","binaries":[]}]})
        ),
        Err(v2::Error::Validation(ProjectError::DuplicateId {
            kind: "library",
            ..
        }))
    ));
}

#[test]
fn whole_graph_handles_parallel_edges_self_and_disconnected_cycles_iteratively() {
    assert!(matches!(
        parse(
            json!({"schemaVersion":2,"modules":[{"id":"a","dependencies":[edge("module","a","runtime",true)]}]})
        ),
        Err(v2::Error::Validation(ProjectError::SelfDependency(_)))
    ));
    let parallel: Vec<_> = ["compile", "test", "runtime", "provided"]
        .into_iter()
        .flat_map(|scope| [false, true].map(|exported| edge("module", "b", scope, exported)))
        .collect();
    assert_eq!(
        parse(json!({"schemaVersion":2,"modules":[{"id":"a","dependencies":parallel},{"id":"b"}]}))
            .unwrap()
            .modules[0]
            .dependencies
            .len(),
        8
    );
    assert!(matches!(
        parse(
            json!({"schemaVersion":2,"modules":[{"id":"unrelated"},{"id":"a","dependencies":parallel},{"id":"b","dependencies":[edge("module","a","test",false)]}]})
        ),
        Err(v2::Error::Validation(ProjectError::DependencyCycle))
    ));
    let modules: Vec<_> = (0..MAX_MODULES)
        .map(|i| {
            let dependencies = if i + 1 < MAX_MODULES {
                vec![edge("module", &format!("m{}", i + 1), "compile", false)]
            } else {
                vec![]
            };
            json!({"id":format!("m{i}"),"dependencies":dependencies})
        })
        .collect();
    assert_eq!(
        parse(json!({"schemaVersion":2,"modules":modules}))
            .unwrap()
            .modules
            .len(),
        MAX_MODULES
    );
}

#[test]
fn paths_ownership_and_settings_keep_shared_semantics() {
    for path in ["", "a\0b"] {
        for value in [
            json!({"schemaVersion":2,"modules":[{"id":"a","sourceRoots":[{"path":path,"kind":"test"}]}]}),
            json!({"schemaVersion":2,"modules":[],"libraries":[{"id":"l","binaries":[path]}]}),
            json!({"schemaVersion":2,"modules":[],"libraries":[{"id":"l","binaries":[],"sources":[path]}]}),
            json!({"schemaVersion":2,"modules":[],"sdkArchives":[path]}),
        ] {
            assert!(matches!(
                parse(value),
                Err(v2::Error::Validation(ProjectError::InvalidPath(_)))
            ));
        }
        assert!(matches!(
            v2::from_json(path, &serde_json::to_vec(&minimal()).unwrap()),
            Err(v2::Error::Validation(ProjectError::InvalidPath(_)))
        ));
    }
    for (second_module, path, kind, generated) in [
        (false, "src/.", "test", false),
        (true, "src/deep", "test", false),
        (false, "src/deep", "production", false),
        (false, "src/deep", "test", true),
    ] {
        let mut value = json!({"schemaVersion":2,"modules":[{"id":"a","sourceRoots":[{"path":"src","kind":"test"}]}]});
        let root = json!({"path":path,"kind":kind,"generated":generated});
        if second_module {
            value["modules"]
                .as_array_mut()
                .unwrap()
                .push(json!({"id":"b","sourceRoots":[root]}));
        } else {
            value["modules"][0]["sourceRoots"]
                .as_array_mut()
                .unwrap()
                .push(root);
        }
        assert!(matches!(
            parse(value),
            Err(v2::Error::Validation(
                ProjectError::SourceRootOverlap { .. }
            ))
        ));
    }
    let roots = json!([{"path":"源/😀","kind":"test"},{"path":"源/😀/nested","kind":"test"},{"path":"源-other","kind":"production"}]);
    let bytes =
        serde_json::to_vec(&json!({"schemaVersion":2,"modules":[{"id":"a","sourceRoots":roots}]}))
            .unwrap();
    let model = v2::from_json("somewhere/../ktlsp.json", &bytes).unwrap();
    assert_eq!(
        model.modules[0].source_roots[0].path,
        std::env::current_dir().unwrap().join("源/😀")
    );
    for (settings, valid) in [
        (json!({"languageVersion":"2.9","apiVersion":"2.10"}), false),
        (
            json!({"languageVersion":"2.10","apiVersion":"2.9","jvmTarget":"1.8"}),
            true,
        ),
        (json!({"jvmTarget":"0"}), false),
        (json!({"languageVersion":"٢.٣"}), false),
        (json!({"extra":true}), false),
    ] {
        let mut value = minimal();
        value["modules"][0]["languageSettings"] = settings;
        assert_eq!(parse(value).is_ok(), valid);
    }
}

#[test]
fn exact_collection_preflight_boundaries_precede_typed_schema_allocation() {
    for (budget, limit, field) in [
        (Budget::Modules, MAX_MODULES, "modules"),
        (Budget::Libraries, MAX_LIBRARIES, "libraries"),
        (Budget::ArchivePaths, MAX_ARCHIVE_PATHS, "sdkArchives"),
    ] {
        for extra in [0, 1] {
            let mut value = json!({"schemaVersion":2,"modules":[]});
            value[field] = json!(vec![Value::Null; limit + extra]);
            let error = parse(value).unwrap_err();
            if extra == 0 {
                assert!(matches!(
                    error,
                    v2::Error::Validation(ProjectError::Json(_))
                ));
            } else {
                assert!(
                    matches!(error, v2::Error::Validation(ProjectError::BudgetExceeded { budget:b,limit:l }) if b == budget && l == limit)
                );
            }
        }
    }
    for (budget, limit, field) in [
        (Budget::Dependencies, MAX_DEPENDENCIES, "dependencies"),
        (Budget::SourceRoots, MAX_SOURCE_ROOTS, "sourceRoots"),
    ] {
        for extra in [0, 1] {
            let value = json!({"schemaVersion":2,"modules":[{"id":"a",field:vec![Value::Null;limit/2]},{"id":"b",field:vec![Value::Null;limit-limit/2+extra]}]});
            let error = parse(value).unwrap_err();
            if extra == 0 {
                assert!(matches!(
                    error,
                    v2::Error::Validation(ProjectError::Json(_))
                ));
            } else {
                assert!(
                    matches!(error, v2::Error::Validation(ProjectError::BudgetExceeded { budget:b,limit:l }) if b == budget && l == limit)
                );
            }
        }
    }
    let value = json!({"schemaVersion":2,"modules":[],"libraries":[{"id":"l","binaries":vec!["a";MAX_ARCHIVE_PATHS/3],"sources":vec!["a";MAX_ARCHIVE_PATHS/3]}],"sdkArchives":vec!["a";MAX_ARCHIVE_PATHS-2*(MAX_ARCHIVE_PATHS/3)]});
    assert!(parse(value.clone()).is_ok());
    let mut excess = value;
    excess["sdkArchives"]
        .as_array_mut()
        .unwrap()
        .push(json!("a"));
    assert!(matches!(
        parse(excess),
        Err(v2::Error::Validation(ProjectError::BudgetExceeded {
            budget: Budget::ArchivePaths,
            ..
        }))
    ));
}

#[test]
fn valid_large_libraries_roots_and_mixed_edges_remain_bounded() {
    assert_eq!(parse(json!({"schemaVersion":2,"modules":[],"libraries":(0..MAX_LIBRARIES).map(|i|json!({"id":format!("l{i}"),"binaries":[]})).collect::<Vec<_>>()})).unwrap().libraries.len(), MAX_LIBRARIES);
    assert_eq!(parse(json!({"schemaVersion":2,"modules":[{"id":"a","sourceRoots":(0..MAX_SOURCE_ROOTS).map(|i|json!({"path":format!("src/r{i}"),"kind":"production"})).collect::<Vec<_>>()}]})).unwrap().modules[0].source_roots.len(), MAX_SOURCE_ROOTS);
    let libraries: Vec<_> = (0..2048)
        .map(|i| json!({"id":format!("l{i}"),"binaries":[]}))
        .collect();
    let edges: Vec<_> = (0..2048)
        .flat_map(|i| {
            ["compile", "test", "runtime", "provided"]
                .into_iter()
                .flat_map(move |scope| {
                    [false, true].map(|exported| edge("library", &format!("l{i}"), scope, exported))
                })
        })
        .chain([edge("module", "b", "compile", false)])
        .collect();
    let bytes = serde_json::to_vec(&json!({"schemaVersion":2,"modules":[{"id":"a","dependencies":edges},{"id":"b"}],"libraries":libraries})).unwrap();
    assert!(bytes.len() < MAX_MANIFEST_BYTES);
    assert_eq!(
        v2::from_json(base(), &bytes).unwrap().modules[0]
            .dependencies
            .len(),
        16385
    );
}

#[test]
fn caller_base_amplification_is_bounded_before_any_model_path_expansion() {
    fn long_manifest(base_bytes: usize) -> PathBuf {
        let prefix = std::env::temp_dir().join("ktlsp-v2-budget");
        let padding = base_bytes - prefix.as_os_str().as_encoded_bytes().len() - 1;
        prefix.join("b".repeat(padding)).join("m")
    }
    let limit = v2::MAX_EXPANDED_PATH_BYTES;
    let path = long_manifest(limit / 2 - 2);
    assert_eq!(
        path.parent().unwrap().as_os_str().as_encoded_bytes().len(),
        limit / 2 - 2
    );
    for value in [
        json!({"schemaVersion":2,"modules":[{"id":"a","sourceRoots":[{"path":"a","kind":"test"},{"path":"b","kind":"test"}]}]}),
        json!({"schemaVersion":2,"modules":[],"libraries":[{"id":"l","binaries":["a","b"]}]}),
        json!({"schemaVersion":2,"modules":[],"libraries":[{"id":"l","binaries":[],"sources":["a","b"]}]}),
        json!({"schemaVersion":2,"modules":[],"sdkArchives":["a","b"]}),
    ] {
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(v2::from_json(&path, &bytes).is_ok());
        let excess = long_manifest(limit / 2 - 1);
        assert!(
            matches!(v2::from_json(&excess, &bytes), Err(v2::Error::ExpandedPathBudgetExceeded { limit:l }) if l == limit)
        );
    }
    // Different path collections consume one shared estimate.
    let mixed = serde_json::to_vec(&json!({"schemaVersion":2,"modules":[{"id":"a","sourceRoots":[{"path":"a","kind":"test"}]}],"sdkArchives":["b","c"]})).unwrap();
    assert!(matches!(
        v2::from_json(&path, &mixed),
        Err(v2::Error::ExpandedPathBudgetExceeded { .. })
    ));
    let absolute = std::env::temp_dir().join("small-api.jar");
    let bytes = serde_json::to_vec(
        &json!({"schemaVersion":2,"modules":[],"sdkArchives":[absolute.clone(),absolute]}),
    )
    .unwrap();
    assert!(v2::from_json(long_manifest(limit - 2), &bytes).is_ok());
    let oversized = long_manifest(limit);
    assert!(matches!(
        v2::from_json(&oversized, &serde_json::to_vec(&minimal()).unwrap()),
        Err(v2::Error::ExpandedPathBudgetExceeded { .. })
    ));
    assert!(matches!(
        v2::load_manifest(&oversized),
        Err(v2::Error::ExpandedPathBudgetExceeded { .. })
    ));
}

#[test]
fn expanded_serialization_retains_data_but_reparse_applies_input_byte_limit() {
    let path = std::env::temp_dir().join("b".repeat(80)).join("model.json");
    let input = serde_json::to_vec(&json!({"schemaVersion":2,"modules":[{"id":"a","dependencies":[edge("library","l","test",true)]}],"libraries":[{"id":"l","binaries":[]}],"sdkArchives":vec!["a";MAX_ARCHIVE_PATHS]})).unwrap();
    assert!(input.len() < MAX_MANIFEST_BYTES);
    let model = v2::from_json(&path, &input).unwrap();
    let serialized = serde_json::to_vec(&model).unwrap();
    assert!(serialized.len() > MAX_MANIFEST_BYTES);
    let value: Value = serde_json::from_slice(&serialized).unwrap();
    assert_eq!(
        value["sdkArchives"].as_array().unwrap().len(),
        MAX_ARCHIVE_PATHS
    );
    assert_eq!(
        value["modules"][0]["dependencies"][0],
        edge("library", "l", "test", true)
    );
    assert!(matches!(
        v2::from_json(&path, &serialized),
        Err(v2::Error::Validation(ProjectError::BudgetExceeded {
            budget: Budget::ManifestBytes,
            ..
        }))
    ));
}

#[test]
fn exact_byte_limit_bounded_file_reads_and_io_errors() {
    let mut bytes = serde_json::to_vec(&minimal()).unwrap();
    bytes.resize(MAX_MANIFEST_BYTES, b' ');
    assert!(v2::from_json(base(), &bytes).is_ok());
    bytes.push(b' ');
    assert!(matches!(
        v2::from_json(base(), &bytes),
        Err(v2::Error::Validation(ProjectError::BudgetExceeded {
            budget: Budget::ManifestBytes,
            ..
        }))
    ));
    let path = std::env::temp_dir().join(format!("ktlsp-v2-{}-oversize.json", std::process::id()));
    std::fs::write(&path, &bytes).unwrap();
    let result = v2::load_manifest(&path);
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(
        result,
        Err(v2::Error::Validation(ProjectError::BudgetExceeded {
            budget: Budget::ManifestBytes,
            ..
        }))
    ));
    assert!(matches!(
        v2::load_manifest(base().join("absent.json")),
        Err(v2::Error::Validation(ProjectError::Io(_)))
    ));
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/t021b/ordered.json");
    assert_eq!(
        v2::load_manifest(&path).unwrap().modules[0].source_roots[0].path,
        path.parent().unwrap().join("src/main")
    );
}

#[test]
fn malformed_truncations_arbitrary_bytes_and_deep_json_do_not_panic() {
    let end = ORDERED
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .unwrap();
    for length in 0..=end {
        assert!(v2::from_json(base(), &ORDERED[..length]).is_err());
    }
    for byte in 0..=255 {
        let _ = v2::from_json(base(), &[byte; 256]);
    }
    let nested = format!("{}0{}", "[".repeat(10000), "]".repeat(10000));
    assert!(matches!(
        v2::from_json(base(), nested.as_bytes()),
        Err(v2::Error::Validation(ProjectError::Json(_)))
    ));
}

#[cfg(unix)]
#[test]
fn non_utf8_manifest_paths_are_controlled_errors() {
    use std::os::unix::ffi::OsStringExt;
    let non_utf8 = PathBuf::from(std::ffi::OsString::from_vec(b"invalid-\xff".to_vec()));
    assert!(matches!(
        v2::from_json(&non_utf8, &serde_json::to_vec(&minimal()).unwrap()),
        Err(v2::Error::Validation(ProjectError::InvalidPath(_)))
    ));
    assert!(matches!(
        v2::load_manifest(&non_utf8),
        Err(v2::Error::Validation(ProjectError::InvalidPath(_)))
    ));
}

// macOS filesystems/sandbox reject creating raw non-Unicode directory names.
#[cfg(target_os = "linux")]
#[test]
fn non_utf8_cwd_paths_are_controlled_errors() {
    use std::os::unix::ffi::OsStringExt;
    if std::env::var_os("KTLSP_V2_NON_UTF8_CWD").is_some() {
        for path in ["ktlsp.json", "../ktlsp.json"] {
            assert!(matches!(
                v2::from_json(path, &serde_json::to_vec(&minimal()).unwrap()),
                Err(v2::Error::Validation(ProjectError::InvalidPath(_)))
            ));
        }
        return;
    }
    let directory = std::env::temp_dir().join(std::ffi::OsString::from_vec(
        format!("ktlsp-v2-{}-", std::process::id())
            .bytes()
            .chain([255])
            .collect(),
    ));
    std::fs::create_dir(&directory).unwrap();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "non_utf8_cwd_paths_are_controlled_errors"])
        .env("KTLSP_V2_NON_UTF8_CWD", "1")
        .current_dir(&directory)
        .output();
    std::fs::remove_dir(&directory).unwrap();
    let result = result.unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

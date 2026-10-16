use ktlsp_import::{Dependency, ImportError, MAX_BYTES, MAX_REPORTS, from_json};
use ktlsp_project::{DependencyScope, ProjectError};
use serde_json::{Value, json};
use std::path::PathBuf;

fn base() -> PathBuf {
    std::env::temp_dir().join("ktlsp-import-space 😀")
}
fn import(value: Value) -> Result<ktlsp_import::ImportedWorkspace, ImportError> {
    from_json(base(), &serde_json::to_vec(&value).unwrap())
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/source-model.json")).unwrap()
}
fn unsupported(value: Value, location: &str) {
    let Err(ImportError::Unsupported { fields, .. }) = import(value) else {
        panic!("expected unsupported {location}");
    };
    assert!(fields.iter().any(|f| f.path == location), "{fields:?}");
}
#[test]
fn preserves_graph_roots_library_sources_and_order() {
    let result = import(fixture()).unwrap();
    let model = result.project_projection();
    assert_eq!(model.modules.len(), 2);
    assert_eq!(model.modules[0].id, "app😀");
    assert_eq!(
        model.modules[0].dependencies[0].scope,
        DependencyScope::Test
    );
    assert!(model.modules[0].dependencies[0].exported);
    assert_eq!(
        model.modules[0].source_roots[0].path,
        base().join("app/src/kotlin")
    );
    assert_eq!(model.libraries[0].sources, [base().join("lib/sources.zip")]);
    assert_eq!(
        model.libraries[0].binaries,
        [base().join("lib/classes.jar")]
    );
    assert_eq!(result.modules[0].content_roots[0].path, base().join("app"));
    assert_eq!(
        result.modules[0].content_roots[0].source_roots[0].root_type,
        "kotlin-source"
    );
    assert!(matches!(
        result.modules[0].dependencies[0],
        Dependency::Library(_)
    ));
    assert!(matches!(
        result.modules[0].dependencies[1],
        Dependency::ModuleSource
    ));
    assert!(matches!(
        result.modules[0].dependencies[2],
        Dependency::Module(_)
    ));
    assert_eq!(result.libraries[0].roots[0].root_type, "SOURCES");
    assert_eq!(result.libraries[0].roots[1].root_type, "CLASSES");
    assert!(model.sdk_archives.is_empty());
}
#[test]
fn defaults_and_required_nullable_fields() {
    assert!(import(json!({})).is_ok());
    assert!(import(json!({"modules": [{"name": "empty"}]})).is_ok());
    assert!(
        matches!(import(json!({"libraries": [{"name": "x", "roots": []}]})), Err(ImportError::Schema { path, .. }) if path == "/libraries/0/type")
    );
    for value in [
        json!({"modules": null}),
        json!({"modules": [null]}),
        json!({"modules": [{"name": 4}]}),
        json!({"libraries": [{"name": "x", "type": null, "roots": null}]}),
    ] {
        assert!(matches!(import(value), Err(ImportError::Schema { .. })));
    }
}
#[test]
fn unknown_fields_at_all_supported_levels_and_versions() {
    for pointer in [
        "/schemaVersion",
        "/modules/0/future",
        "/modules/0/dependencies/0/future",
        "/modules/0/contentRoots/0/future",
        "/modules/0/contentRoots/0/sourceRoots/0/future",
        "/libraries/0/future",
        "/libraries/0/roots/0/future",
    ] {
        let mut value = fixture();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        value
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.into(), Value::Null);
        unsupported(value, pointer);
    }
    unsupported(json!({"version": 99}), "/version");
    unsupported(json!({"a/b~c": null}), "/a~1b~0c");
}
#[test]
fn meaningful_semantics_block_atomically() {
    for (pointer, replacement) in [
        ("/modules/0/dependencies/0/scope", json!("runtime")),
        ("/modules/0/dependencies/0/isExported", json!(true)),
        ("/modules/0/dependencies/2/isTestJar", json!(true)),
        (
            "/modules/0/contentRoots/0/excludedPatterns",
            json!(["*.kt"]),
        ),
        (
            "/modules/0/contentRoots/0/excludedUrls",
            json!(["<WORKSPACE>/build"]),
        ),
        (
            "/modules/0/contentRoots/0/sourceRoots/0/type",
            json!("java-resource"),
        ),
        (
            "/modules/0/facets",
            json!([{"name": "kotlin", "type": "kotlin-language"}]),
        ),
        ("/modules/0/type", Value::Null),
        ("/modules/0/coordinate", json!("example:app:1")),
        ("/modules/0/externalProjectId", json!(":app")),
        ("/libraries/0/level", json!("module")),
        ("/libraries/0/properties", json!({"tag": "properties"})),
        ("/libraries/0/roots/0/type", json!("JAVADOC")),
        (
            "/libraries/0/roots/0/inclusionOptions",
            json!("archives_under_root"),
        ),
        ("/sdks", json!([{"name": "JDK", "roots": []}])),
        (
            "/kotlinSettings",
            json!([{"module": "app😀", "compilerArguments": "<args/>"}]),
        ),
        (
            "/javaSettings",
            json!([{"module": "app😀", "languageLevelId": "JDK_21"}]),
        ),
        ("/externalSystem", json!("GRADLE")),
    ] {
        let mut value = fixture();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        value
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.into(), replacement);
        unsupported(value, pointer);
    }
    for variant in ["sdk", "inheritedSdk", "future"] {
        unsupported(
            json!({"modules": [{"name": "a", "dependencies": [{"type": variant}]}]}),
            "/modules/0/dependencies/0/type",
        );
    }
}
#[test]
fn malformed_json_duplicate_keys_and_enums() {
    for bytes in [
        b"{".as_slice(),
        b"{}{}",
        b"{\"modules\":[],\"modules\":[]}",
        b"{\"modules\":[{\"name\":\"a\",\"name\":\"b\"}]}",
        b"{\"future\":{\"x\":1,\"x\":2}}",
        &[0xff],
    ] {
        assert!(matches!(
            from_json(base(), bytes),
            Err(ImportError::Json(_))
        ));
    }
    let mut value = fixture();
    value["modules"][0]["dependencies"][2]["scope"] = json!("Compile");
    assert!(matches!(import(value), Err(ImportError::Schema { .. })));
}
#[test]
fn paths_are_explicit_lexical_and_missing_artifacts_retained() {
    let mut value = fixture();
    // Import paths use slash spelling, including native Windows drive prefixes.
    let absolute = base().join("不存在.jar");
    value["libraries"][0]["roots"][1]["path"] =
        json!(absolute.to_str().unwrap().replace('\\', "/"));
    assert!(import(value).is_ok());
    for value_path in [
        "src",
        "file:///tmp/src",
        "jar:///tmp/a.jar!/",
        "jrt:///jdk!/java.base",
        "<HOME>/src",
        "<MAVEN_REPO>/a.jar",
        "C:\\work\\src",
        "<WORKSPACE>//etc",
        "<WORKSPACE>/a\\b",
        "<WORKSPACE>/a/<HOME>/b",
    ] {
        let mut value = fixture();
        value["modules"][0]["contentRoots"][0]["sourceRoots"][0]["path"] = json!(value_path);
        unsupported(value, "/modules/0/contentRoots/0/sourceRoots/0/path");
    }
    for invalid in ["", "a\0b"] {
        let mut value = fixture();
        value["libraries"][0]["roots"][0]["path"] = json!(invalid);
        assert!(matches!(import(value), Err(ImportError::Schema { .. })));
    }
    assert!(from_json("relative", b"{}").is_err());
    let mut value = fixture();
    value["modules"][0]["contentRoots"][0]["sourceRoots"][0]["path"] =
        json!("<WORKSPACE>/../sibling");
    assert_eq!(
        import(value).unwrap().project_projection().modules[0].source_roots[0].path,
        base().parent().unwrap().join("sibling")
    );
}
#[test]
fn native_validation_is_not_flattened() {
    let mut value = fixture();
    value["modules"][1]["dependencies"] =
        json!([{"type": "module", "name": "app😀", "scope": "provided"}]);
    assert!(matches!(
        import(value),
        Err(ImportError::Project(ProjectError::DependencyCycle))
    ));
    let mut value = fixture();
    value["modules"][1]["name"] = json!("app😀");
    assert!(matches!(
        import(value),
        Err(ImportError::Project(ProjectError::DuplicateId { .. }))
    ));
    let mut value = fixture();
    value["modules"][0]["dependencies"][2]["name"] = json!("missing");
    assert!(matches!(
        import(value),
        Err(ImportError::Project(ProjectError::UnknownReference { .. }))
    ));
    let mut value = fixture();
    value["modules"][1]["contentRoots"][0]["sourceRoots"][0]["path"] = json!("<WORKSPACE>/app/src");
    assert!(matches!(
        import(value),
        Err(ImportError::Project(ProjectError::SourceRootOverlap { .. }))
    ));
    let mut value = fixture();
    let edge = value["modules"][0]["dependencies"][2].clone();
    value["modules"][0]["dependencies"]
        .as_array_mut()
        .unwrap()
        .push(edge);
    assert!(matches!(
        import(value),
        Err(ImportError::Project(
            ProjectError::DuplicateDependency { .. }
        ))
    ));
}
#[test]
fn bytes_and_reports_bounded_with_explicit_truncation() {
    assert!(matches!(
        from_json(base(), &vec![b' '; MAX_BYTES + 1]),
        Err(ImportError::BudgetExceeded("bytes"))
    ));
    let mut exact = vec![b' '; MAX_BYTES];
    exact[..2].copy_from_slice(b"{}");
    assert!(from_json(base(), &exact).is_ok());
    for count in [MAX_REPORTS, MAX_REPORTS + 1] {
        let value: serde_json::Map<String, Value> =
            (0..count).map(|n| (n.to_string(), Value::Null)).collect();
        let Err(ImportError::Unsupported { fields, truncated }) = import(Value::Object(value))
        else {
            panic!()
        };
        assert_eq!(fields.len(), MAX_REPORTS);
        assert_eq!(truncated, count > MAX_REPORTS);
    }
}
#[test]
fn deterministic_hostile_corpus_has_no_panics() {
    let atoms = ["null", "true", "4", "\"😀\"", "{}", "[]"];
    for depth in 0..70 {
        for atom in atoms {
            let text = format!("{}{}{}", "[".repeat(depth), atom, "]".repeat(depth));
            assert_eq!(
                from_json(base(), text.as_bytes()).is_ok(),
                depth == 0 && atom == "{}"
            );
        }
    }
    let source = include_bytes!("fixtures/source-model.json");
    for end in 0..source.len() {
        let _ = from_json(base(), &source[..end]);
    }
}

#[test]
fn all_module_scopes_and_defaults_survive() {
    for scope in ["compile", "test", "runtime", "provided"] {
        let result = import(json!({"modules": [
            {"name": "a", "dependencies": [{"type":"module", "name":"b", "scope":scope}]},
            {"name": "b"}
        ]}))
        .unwrap();
        let edge = &result.project_projection().modules[0].dependencies[0];
        assert_eq!(serde_json::to_value(edge.scope).unwrap(), json!(scope));
        assert!(!edge.exported);
    }
}

#[test]
fn expanded_paths_are_bounded_before_repeating_caller_base() {
    let oversized = base().join("x".repeat(ktlsp_import::MAX_EXPANDED_PATH_BYTES));
    assert!(matches!(
        from_json(oversized, b"{}"),
        Err(ImportError::BudgetExceeded("project directory bytes"))
    ));
    let long_base = base().join("x".repeat(3000));
    let roots: Vec<Value> = (0..1000)
        .map(|n| json!({"path": format!("<WORKSPACE>/src{n}"), "type": "java-source"}))
        .collect();
    let value = json!({"modules": [{"name": "a", "contentRoots": [{"path": "<WORKSPACE>/", "sourceRoots": roots}]}]});
    assert!(matches!(
        from_json(long_base, &serde_json::to_vec(&value).unwrap()),
        Err(ImportError::BudgetExceeded("expanded path bytes"))
    ));
}

#[cfg(unix)]
#[test]
fn non_utf8_caller_directory_is_rejected_before_path_serialization() {
    use std::os::unix::ffi::OsStringExt;
    let base = PathBuf::from(std::ffi::OsString::from_vec(b"/tmp/ktlsp-\xff".to_vec()));
    for bytes in [
        b"{}".as_slice(),
        include_bytes!("fixtures/source-model.json"),
    ] {
        assert!(matches!(
            from_json(&base, bytes),
            Err(ImportError::Schema { .. })
        ));
    }
}

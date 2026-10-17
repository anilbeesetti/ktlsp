use ktlsp_import::{
    ImportError, MAX_BYTES, MAX_DEPTH, MAX_EXPANDED_PATH_BYTES, MAX_NODES, MAX_REPORTS, v2,
};
use ktlsp_project::{DependencyScope, ModuleDependency, ProjectError, SourceRootKind};
use serde_json::{Value, json};
use std::{
    error::Error as _,
    path::{Path, PathBuf},
};

fn base() -> PathBuf {
    std::env::temp_dir().join("ktlsp-import-v2")
}

fn base_with_bytes(bytes: usize) -> String {
    let directory = base();
    let root = directory.ancestors().last().unwrap().to_str().unwrap();
    let value = format!("{root}{}", "x".repeat(bytes - root.len()));
    assert!(Path::new(&value).is_absolute());
    assert_eq!(value.len(), bytes);
    value
}
const FIXTURE: &[u8] = include_bytes!("fixtures/scoped-exported.json");

fn fixture() -> Value {
    serde_json::from_slice(FIXTURE).unwrap()
}
fn import(value: Value) -> Result<v2::ImportedWorkspace, v2::Error> {
    v2::from_json(base(), &serde_json::to_vec(&value).unwrap())
}
fn replace(value: &mut Value, pointer: &str, replacement: Value) {
    if let Some(target) = value.pointer_mut(pointer) {
        *target = replacement;
        return;
    }
    let (parent, key) = pointer.rsplit_once('/').unwrap();
    value
        .pointer_mut(parent)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert(key.into(), replacement);
}
fn unsupported(value: Value, pointer: &str) {
    let Err(v2::Error::Import(ImportError::Unsupported { fields, .. })) = import(value) else {
        panic!("expected Unsupported at {pointer}");
    };
    assert!(
        fields.iter().any(|field| field.path == pointer),
        "{fields:?}"
    );
}
fn validation(value: Value) -> ProjectError {
    match import(value).unwrap_err() {
        v2::Error::Project(ktlsp_project::v2::Error::Validation(error)) => error,
        other => panic!("expected project validation: {other:?}"),
    }
}

#[test]
fn scopes_exports_order_and_root_membership_survive_import() {
    use DependencyScope::{Compile, Provided, Runtime, Test};
    use v2::Dependency::{Library, Module, ModuleSource};
    let directory = base();
    let result = v2::from_json(&directory, FIXTURE).unwrap();
    assert_eq!(
        result
            .modules()
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>(),
        ["app😀", "shared"]
    );
    assert_eq!(
        result.modules()[0].dependencies,
        [
            Library {
                library: "shared".into(),
                scope: Compile,
                exported: false
            },
            ModuleSource,
            Module(ModuleDependency {
                module: "shared".into(),
                scope: Provided,
                exported: true
            }),
            Library {
                library: "shared".into(),
                scope: Compile,
                exported: true
            },
            Library {
                library: "shared".into(),
                scope: Test,
                exported: false
            },
            Library {
                library: "shared".into(),
                scope: Test,
                exported: true
            },
            ModuleSource,
            Library {
                library: "shared".into(),
                scope: Runtime,
                exported: false
            },
            Library {
                library: "shared".into(),
                scope: Runtime,
                exported: true
            },
            Module(ModuleDependency {
                module: "shared".into(),
                scope: Provided,
                exported: false
            }),
            Library {
                library: "shared".into(),
                scope: Provided,
                exported: false
            },
            Library {
                library: "shared".into(),
                scope: Provided,
                exported: true
            },
        ]
    );
    assert!(result.modules()[1].dependencies.is_empty());
    let project = result.project_projection();
    assert_eq!(
        project
            .modules
            .iter()
            .map(|module| module.id.as_str())
            .collect::<Vec<_>>(),
        ["app😀", "shared"]
    );
    assert_eq!(
        project.modules[0]
            .dependencies
            .iter()
            .map(|edge| match edge {
                ktlsp_project::v2::Dependency::Library {
                    library,
                    scope,
                    exported,
                } => ("library", library.as_str(), *scope, *exported),
                ktlsp_project::v2::Dependency::Module {
                    module,
                    scope,
                    exported,
                } => ("module", module.as_str(), *scope, *exported),
                other => panic!("unexpected projected dependency: {other:?}"),
            })
            .collect::<Vec<_>>(),
        [
            ("library", "shared", Compile, false),
            ("module", "shared", Provided, true),
            ("library", "shared", Compile, true),
            ("library", "shared", Test, false),
            ("library", "shared", Test, true),
            ("library", "shared", Runtime, false),
            ("library", "shared", Runtime, true),
            ("module", "shared", Provided, false),
            ("library", "shared", Provided, false),
            ("library", "shared", Provided, true),
        ]
    );
    assert_eq!(
        project.modules[0]
            .source_roots
            .iter()
            .map(|root| (
                root.path.strip_prefix(&directory).unwrap(),
                root.kind,
                root.generated
            ))
            .collect::<Vec<_>>(),
        [
            (
                Path::new("app/src/kotlin"),
                SourceRootKind::Production,
                false
            ),
            (Path::new("app/test"), SourceRootKind::Test, false),
            (Path::new("extra/src"), SourceRootKind::Production, false),
        ]
    );
    assert!(project.modules[1].source_roots.is_empty());
    assert!(project.modules[1].dependencies.is_empty());
    assert_eq!(project.libraries.len(), 1);
    assert_eq!(project.libraries[0].id, "shared");
    assert_eq!(
        project.libraries[0].binaries,
        [directory.join("lib/classes.jar")]
    );
    assert_eq!(
        project.libraries[0].sources,
        [directory.join("lib/sources.zip")]
    );
    assert_eq!(result.modules()[0].content_roots.len(), 2);
    assert_eq!(
        result.modules()[0].content_roots[0].path,
        directory.join("app")
    );
    assert_eq!(
        result.modules()[0].content_roots[1].path,
        directory.join("extra")
    );
    let roots: Vec<_> = result.modules()[0]
        .content_roots
        .iter()
        .flat_map(|c| &c.source_roots)
        .map(|r| {
            (
                r.path.strip_prefix(&directory).unwrap(),
                r.root_type.as_str(),
            )
        })
        .collect();
    assert_eq!(
        roots,
        [
            (Path::new("app/src/kotlin"), "kotlin-source"),
            (Path::new("app/test"), "java-test"),
            (Path::new("extra/src"), "java-source"),
        ]
    );
    assert_eq!(result.libraries().len(), 1);
    assert_eq!(result.libraries()[0].name, "shared");
    assert_eq!(
        result.libraries()[0]
            .roots
            .iter()
            .map(|r| (
                r.path.strip_prefix(&directory).unwrap(),
                r.root_type.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            (Path::new("lib/sources.zip"), "SOURCES"),
            (Path::new("lib/classes.jar"), "CLASSES"),
        ]
    );
    let bytes = serde_json::to_vec(project).unwrap();
    assert_eq!(
        ktlsp_project::v2::from_json(
            std::env::temp_dir().join("elsewhere/workspace.json"),
            &bytes
        )
        .unwrap(),
        *project
    );
}

#[test]
fn duplicate_tuples_references_and_graphs_are_not_flattened() {
    for (index, library) in [(0, true), (2, false)] {
        let mut value = fixture();
        let mut edge = value["modules"][0]["dependencies"][index].clone();
        if library {
            edge["isExported"] = json!(false);
        }
        value["modules"][0]["dependencies"]
            .as_array_mut()
            .unwrap()
            .push(edge);
        let error = validation(value);
        if library {
            assert!(
                matches!(error, ProjectError::DuplicateLibraryReference { module, library } if module == "app😀" && library == "shared")
            );
        } else {
            assert!(
                matches!(error, ProjectError::DuplicateDependency { module, dependency } if module == "app😀" && dependency == "shared")
            );
        }
    }
    for (index, kind) in [(0, "library"), (2, "module")] {
        let mut value = fixture();
        value["modules"][0]["dependencies"][index]["name"] = json!("missing");
        assert!(
            matches!(validation(value), ProjectError::UnknownReference { kind: actual, id } if actual == kind && id == "missing")
        );
    }
    for (edge_type, modules, libraries) in [
        (
            "library",
            json!([{"name":"a"},{"name":"shared"}]),
            json!([]),
        ),
        (
            "module",
            json!([{"name":"a"}]),
            json!([{"name":"shared","type":null,"roots":[]}]),
        ),
    ] {
        let mut value = json!({"modules":modules,"libraries":libraries});
        value["modules"][0]["dependencies"] =
            json!([{"type":edge_type,"name":"shared","scope":"runtime","isExported":true}]);
        assert!(
            matches!(validation(value), ProjectError::UnknownReference { kind, id } if kind == edge_type && id == "shared")
        );
    }
    let mut value = fixture();
    value["modules"][0]["dependencies"][2]["name"] = json!("app😀");
    assert!(matches!(validation(value), ProjectError::SelfDependency(id) if id == "app😀"));
    let mut value = fixture();
    value["modules"][1]["dependencies"] =
        json!([{"type":"module","name":"app😀","scope":"runtime","isExported":true}]);
    assert!(matches!(validation(value), ProjectError::DependencyCycle));
    for pointer in ["/modules/1/name", "/libraries/0/name"] {
        let mut value = fixture();
        replace(&mut value, pointer, json!(" "));
        assert!(matches!(validation(value), ProjectError::InvalidId { .. }));
    }
    for key in ["modules", "libraries"] {
        let mut value = fixture();
        let item = value[key][0].clone();
        value[key].as_array_mut().unwrap().push(item);
        assert!(matches!(
            validation(value),
            ProjectError::DuplicateId { .. }
        ));
    }
    let mut value = fixture();
    value["modules"][1]["contentRoots"] = json!([{"path":"<WORKSPACE>/other","sourceRoots":[{"path":"<WORKSPACE>/app/src","type":"java-source"}]}]);
    assert!(matches!(
        validation(value),
        ProjectError::SourceRootOverlap { .. }
    ));
}

#[test]
fn malformed_supported_fields_report_exact_schema_pointers() {
    for (key, values) in [
        ("name", vec![Value::Null, json!(1)]),
        (
            "scope",
            vec![Value::Null, json!("Compile"), json!("future")],
        ),
        ("isExported", vec![Value::Null, json!(1), json!("true")]),
        ("type", vec![Value::Null, json!(1)]),
    ] {
        for replacement in values {
            let mut value = fixture();
            value["modules"][0]["dependencies"][0][key] = replacement;
            assert!(
                matches!(import(value), Err(v2::Error::Import(ImportError::Schema { path, .. })) if path == format!("/modules/0/dependencies/0/{key}"))
            );
        }
    }
    for key in ["name", "scope"] {
        let mut value = fixture();
        value["modules"][0]["dependencies"][0]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            matches!(import(value), Err(v2::Error::Import(ImportError::Schema { path, .. })) if path == format!("/modules/0/dependencies/0/{key}"))
        );
    }
    for (pointer, replacement) in [
        ("/modules/0/dependencies", Value::Null),
        ("/modules/0/dependencies/0", json!(1)),
        ("/libraries/0/roots", Value::Null),
    ] {
        let mut value = fixture();
        replace(&mut value, pointer, replacement);
        assert!(
            matches!(import(value), Err(v2::Error::Import(ImportError::Schema { path, .. })) if path == pointer)
        );
    }
    let mut value = fixture();
    value["libraries"][0]
        .as_object_mut()
        .unwrap()
        .remove("type");
    assert!(
        matches!(import(value), Err(v2::Error::Import(ImportError::Schema { path, .. })) if path == "/libraries/0/type")
    );
    for value in [
        json!({}),
        json!({"modules":[],"libraries":[]}),
        json!({"modules":[{"name":"empty"}]}),
    ] {
        assert!(import(value).is_ok());
    }
}

#[test]
fn rich_edges_do_not_admit_other_unsupported_semantics() {
    for (pointer, replacement) in [
        ("/kotlinSettings", json!([{}])),
        ("/javaSettings", json!([{}])),
        ("/externalSystem", json!("GRADLE")),
        ("/modules/0/facets", json!([{}])),
        ("/modules/0/coordinate", json!("g:a:1")),
        ("/modules/0/externalProjectPath", json!("/a")),
        ("/modules/0/externalProjectId", json!(":a")),
        ("/modules/0/type", Value::Null),
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
            "/modules/0/contentRoots/0/sourceRoots/0/generated",
            json!(true),
        ),
        ("/libraries/0/type", json!("special")),
        ("/libraries/0/level", json!("module")),
        ("/libraries/0/module", json!("app😀")),
        ("/libraries/0/properties", json!({})),
        ("/libraries/0/excludedRoots", json!(["/excluded"])),
        (
            "/libraries/0/roots/0/inclusionOptions",
            json!("archives_under_root"),
        ),
        ("/libraries/0/roots/0/type", json!("JAVADOC")),
        ("/modules/0/dependencies/2/isTestJar", json!(true)),
        ("/modules/0/dependencies/0/isTestJar", json!(false)),
        ("/modules/0/dependencies/1/scope", json!("compile")),
    ] {
        for dangling in [false, true] {
            let mut value = fixture();
            if dangling {
                value["modules"][0]["dependencies"][0]["name"] = json!("missing");
            }
            replace(&mut value, pointer, replacement.clone());
            unsupported(value, pointer);
        }
    }
    {
        let mut value = fixture();
        value["modules"][0]["dependencies"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type":"future"}));
        unsupported(value, "/modules/0/dependencies/12/type");
    }
    let mut value = fixture();
    value["modules"][0]["dependencies"][0]["scope"] = Value::Null;
    assert!(
        matches!(import(value), Err(v2::Error::Import(ImportError::Schema { path, .. })) if path == "/modules/0/dependencies/0/scope")
    );
    assert!(
        matches!(import(json!({"sdks":null})), Err(v2::Error::Import(ImportError::Schema { path, .. })) if path == "/sdks")
    );
    for key in ["kotlinSettings", "javaSettings"] {
        unsupported(json!({key:null}), &format!("/{key}"));
        assert!(import(json!({key:[]})).is_ok());
    }
}

#[test]
fn unknown_fields_and_report_limits_stay_atomic() {
    for pointer in [
        "/schemaVersion",
        "/version",
        "/modules/0/future",
        "/modules/0/dependencies/0/future",
        "/modules/0/contentRoots/0/future",
        "/modules/0/contentRoots/0/sourceRoots/0/future",
        "/libraries/0/future",
        "/libraries/0/roots/0/future",
    ] {
        let mut value = fixture();
        replace(&mut value, pointer, Value::Null);
        unsupported(value, pointer);
    }
    unsupported(json!({"a/b~c":null}), "/a~1b~0c");
    for count in [MAX_REPORTS, MAX_REPORTS + 1] {
        let value: serde_json::Map<String, Value> =
            (0..count).map(|n| (n.to_string(), Value::Null)).collect();
        let Err(v2::Error::Import(ImportError::Unsupported { fields, truncated })) =
            import(Value::Object(value))
        else {
            panic!()
        };
        assert_eq!(fields.len(), MAX_REPORTS);
        assert_eq!(truncated, count > MAX_REPORTS);
    }
}

#[test]
fn malformed_duplicate_and_truncated_json_is_controlled() {
    for bytes in [
        b"{".as_slice(), b"{}{}", &[0xff],
        br#"{"modules":[],"modules":[]}"#,
        br#"{"future":{"x":1,"x":2}}"#,
        br#"{"modules":[{"name":"a","dependencies":[{"type":"library","name":"x","scope":"test","isExported":false,"isExpor\u0074ed":true}]}]}"#,
    ] {
        assert!(matches!(v2::from_json(base(), bytes), Err(v2::Error::Import(ImportError::Json(_)))));
    }
    let end = FIXTURE
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .unwrap();
    for cut in 0..=end {
        assert!(v2::from_json(base(), &FIXTURE[..cut]).is_err(), "cut {cut}");
    }
    assert!(v2::from_json(base(), &FIXTURE[..=end]).is_ok());
}

#[test]
fn old_api_stays_strict_and_preserves_compatible_results() {
    assert!(matches!(
        ktlsp_import::from_json(base(), FIXTURE),
        Err(ImportError::Unsupported { .. })
    ));
    let source = include_bytes!("fixtures/source-model.json");
    let old = ktlsp_import::from_json(base(), source).unwrap();
    let new = v2::from_json(base(), source).unwrap();
    assert_eq!(old.libraries(), new.libraries());
    for (old, new) in old.modules().iter().zip(new.modules()) {
        assert_eq!(old.name, new.name);
        assert_eq!(old.content_roots, new.content_roots);
        assert_eq!(old.dependencies.len(), new.dependencies.len());
        for (old, new) in old.dependencies.iter().zip(&new.dependencies) {
            match old {
                ktlsp_import::Dependency::Module(edge) => {
                    assert_eq!(new, &v2::Dependency::Module(edge.clone()))
                }
                ktlsp_import::Dependency::Library(library) => assert_eq!(
                    new,
                    &v2::Dependency::Library {
                        library: library.clone(),
                        scope: DependencyScope::Compile,
                        exported: false
                    }
                ),
                ktlsp_import::Dependency::ModuleSource => {
                    assert_eq!(new, &v2::Dependency::ModuleSource)
                }
            }
        }
    }
    let value = json!({"modules":[{"name":"a","dependencies":[
        {"type":"module","name":"b","scope":"compile"},
        {"type":"module","name":"b","scope":"test","isExported":true}
    ]},{"name":"b"}]});
    assert!(import(value.clone()).is_ok());
    assert!(matches!(
        ktlsp_import::from_json(base(), &serde_json::to_vec(&value).unwrap()),
        Err(ImportError::Project(
            ProjectError::DuplicateDependency { .. }
        ))
    ));
}

#[test]
fn byte_caller_node_and_depth_boundaries_keep_wrapped_errors() {
    let mut bytes = vec![b' '; MAX_BYTES];
    bytes[..2].copy_from_slice(b"{}");
    assert!(v2::from_json(base(), &bytes).is_ok());
    bytes.push(b' ');
    assert!(matches!(
        v2::from_json("relative", &bytes),
        Err(v2::Error::Import(ImportError::BudgetExceeded("bytes")))
    ));
    let caller = base_with_bytes(MAX_EXPANDED_PATH_BYTES);
    assert!(v2::from_json(&caller, b"{}").is_ok());
    assert!(matches!(
        v2::from_json(format!("{caller}x"), b"{"),
        Err(v2::Error::Import(ImportError::BudgetExceeded(
            "project directory bytes"
        )))
    ));
    for nodes in [MAX_NODES, MAX_NODES + 1] {
        let text = format!("{{\"future\":[{}]}}", vec!["null"; nodes - 2].join(","));
        match v2::from_json(base(), text.as_bytes()).unwrap_err() {
            v2::Error::Import(ImportError::Unsupported { .. }) => assert_eq!(nodes, MAX_NODES),
            v2::Error::Import(ImportError::BudgetExceeded("nodes")) => {
                assert_eq!(nodes, MAX_NODES + 1)
            }
            other => panic!("{other:?}"),
        }
    }
    for depth in [MAX_DEPTH, MAX_DEPTH + 1] {
        let text = format!(
            "{{\"future\":{}null{}}}",
            "[".repeat(depth - 2),
            "]".repeat(depth - 2)
        );
        match v2::from_json(base(), text.as_bytes()).unwrap_err() {
            v2::Error::Import(ImportError::Unsupported { .. }) => assert_eq!(depth, MAX_DEPTH),
            v2::Error::Import(ImportError::BudgetExceeded("depth")) => {
                assert_eq!(depth, MAX_DEPTH + 1)
            }
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn aggregate_content_paths_are_charged_before_join_and_normalization() {
    let base = base_with_bytes(MAX_EXPANDED_PATH_BYTES / 2 - 1);
    let value = json!({"modules":[{"name":"a","contentRoots":[{"path":"<WORKSPACE>/"},{"path":"<WORKSPACE>/"}]}]});
    assert_eq!(2 * (base.len() + 1), MAX_EXPANDED_PATH_BYTES);
    assert!(v2::from_json(&base, &serde_json::to_vec(&value).unwrap()).is_ok());
    let mut over = value.clone();
    over["modules"][0]["contentRoots"][1]["path"] = json!("<WORKSPACE>/x");
    assert!(matches!(
        v2::from_json(&base, &serde_json::to_vec(&over).unwrap()),
        Err(v2::Error::Import(ImportError::BudgetExceeded(
            "expanded path bytes"
        )))
    ));
    over["modules"][0]["contentRoots"][1]["path"] = json!("<WORKSPACE>/../");
    over["future"] = Value::Null;
    assert!(matches!(
        v2::from_json(&base, &serde_json::to_vec(&over).unwrap()),
        Err(v2::Error::Import(ImportError::BudgetExceeded(
            "expanded path bytes"
        )))
    ));
    let mixed_base = base_with_bytes(MAX_EXPANDED_PATH_BYTES / 3);
    let mixed = json!({
        "modules":[{"name":"a","contentRoots":[{"path":"<WORKSPACE>/","sourceRoots":[{"path":"<WORKSPACE>/s","type":"java-source"}]}]}],
        "libraries":[{"name":"lib","type":null,"roots":[{"path":"<WORKSPACE>/l"}]}]
    });
    let bytes = serde_json::to_vec(&mixed).unwrap();
    assert!(bytes.len() < MAX_BYTES);
    assert!(2 * mixed_base.len() + 3 <= MAX_EXPANDED_PATH_BYTES);
    assert!(3 * mixed_base.len() + 5 > MAX_EXPANDED_PATH_BYTES);
    assert!(matches!(
        v2::from_json(&mixed_base, &bytes),
        Err(v2::Error::Import(ImportError::BudgetExceeded(
            "expanded path bytes"
        )))
    ));
}

#[test]
fn escaped_projection_boundary_is_independent_of_input_bytes() {
    let value = json!({"modules":[{"name":"a","contentRoots":[{"path":"<WORKSPACE>/","sourceRoots":[{"path":"<WORKSPACE>/s","type":"java-source"}]}]}]});
    let expected = |base: &str| json!({"schemaVersion":2,"modules":[{"id":"a","sourceRoots":[{"path":Path::new(base).join("s"),"kind":"production"}],"dependencies":[]}],"libraries":[]});
    let directory = base();
    let prefix = directory.ancestors().last().unwrap().join("b");
    let prefix = prefix.to_str().unwrap();
    // Include native separator escaping when calculating the exact byte boundary.
    let overhead = serde_json::to_vec(&expected(prefix)).unwrap().len();
    let remaining = MAX_BYTES - overhead;
    let base = format!(
        "{prefix}{}{}",
        "\u{1}".repeat(remaining / 6),
        "a".repeat(remaining % 6)
    );
    assert_eq!(
        serde_json::to_vec(&expected(&base)).unwrap().len(),
        MAX_BYTES
    );
    let bytes = serde_json::to_vec(&value).unwrap();
    assert!(bytes.len() < 256);
    assert!(2 * base.len() + 3 < MAX_EXPANDED_PATH_BYTES);
    assert!(matches!(
        v2::from_json(&base, &bytes),
        Err(v2::Error::Project(
            ktlsp_project::v2::Error::NormalizedOutputBudgetExceeded { limit: MAX_BYTES }
        ))
    ));
    let over = format!("{base}a");
    assert_eq!(
        serde_json::to_vec(&expected(&over)).unwrap().len(),
        MAX_BYTES + 1
    );
    assert!(matches!(
        v2::from_json(&over, &bytes),
        Err(v2::Error::Import(ImportError::BudgetExceeded(
            "projection bytes"
        )))
    ));
}

#[test]
fn project_collection_limits_remain_additional_guards() {
    for count in [ktlsp_project::MAX_MODULES, ktlsp_project::MAX_MODULES + 1] {
        let value = json!({"modules":(0..count).map(|n| json!({"name":format!("m{n}")})).collect::<Vec<_>>()});
        if count == ktlsp_project::MAX_MODULES {
            assert!(import(value).is_ok());
        } else {
            assert!(matches!(
                validation(value),
                ProjectError::BudgetExceeded {
                    budget: ktlsp_project::Budget::Modules,
                    ..
                }
            ));
        }
    }
    for count in [
        ktlsp_project::MAX_LIBRARIES,
        ktlsp_project::MAX_LIBRARIES + 1,
    ] {
        let value = json!({"libraries":(0..count).map(|n| json!({"name":format!("l{n}"),"type":null,"roots":[]})).collect::<Vec<_>>()});
        if count == ktlsp_project::MAX_LIBRARIES {
            assert!(import(value).is_ok());
        } else {
            assert!(matches!(
                validation(value),
                ProjectError::BudgetExceeded {
                    budget: ktlsp_project::Budget::Libraries,
                    ..
                }
            ));
        }
    }
}

#[test]
fn paths_and_error_sources_retain_boundary_validation() {
    for path in [
        "src",
        "file:///tmp/src",
        "jar:///tmp/a.jar!/",
        "jrt:///jdk!/java.base",
        "<HOME>/src",
        "C:\\work\\src",
        "<WORKSPACE>//etc",
        "<WORKSPACE>/a\\b",
        "<WORKSPACE>/a/<HOME>/b",
    ] {
        let mut value = fixture();
        value["modules"][0]["contentRoots"][0]["sourceRoots"][0]["path"] = json!(path);
        unsupported(value, "/modules/0/contentRoots/0/sourceRoots/0/path");
    }
    for path in ["", "a\0b"] {
        let mut value = fixture();
        value["libraries"][0]["roots"][0]["path"] = json!(path);
        assert!(
            matches!(import(value), Err(v2::Error::Import(ImportError::Schema { path, .. })) if path == "/libraries/0/roots/0/path")
        );
    }
    let mut nul_directory = base().into_os_string();
    nul_directory.push("\0path");
    for directory in [PathBuf::from("relative"), PathBuf::from(nul_directory)] {
        assert!(
            matches!(v2::from_json(directory, b"{}"), Err(v2::Error::Import(ImportError::Schema { path, .. })) if path.is_empty())
        );
    }
    let mut value = fixture();
    value["libraries"][0]["roots"][0]["path"] = json!("<WORKSPACE>/../不存在.zip");
    assert_eq!(
        import(value).unwrap().libraries[0].roots[0].path,
        base().parent().unwrap().join("不存在.zip")
    );
    let error = v2::from_json(base(), b"{").unwrap_err();
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<ImportError>()
            .is_some()
    );
    let mut value = fixture();
    value["modules"][0]["dependencies"][0]["name"] = json!("missing");
    let error = import(value).unwrap_err();
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<ktlsp_project::v2::Error>()
            .is_some()
    );
}

#[cfg(unix)]
#[test]
fn non_unicode_caller_is_schema_without_path_serialization_panic() {
    use std::os::unix::ffi::OsStringExt;
    let base = PathBuf::from(std::ffi::OsString::from_vec(b"/tmp/ktlsp-\xff".to_vec()));
    assert!(
        matches!(v2::from_json(base, FIXTURE), Err(v2::Error::Import(ImportError::Schema { path, .. })) if path.is_empty())
    );
}

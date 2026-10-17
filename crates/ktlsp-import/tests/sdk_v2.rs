use ktlsp_import::{ImportError, v2};
use serde_json::{Value, json};

fn sdk(kind: &str) -> Value {
    json!({"name":"same😀","type":kind,"version":null,"homePath":null,"additionalData":"<opaque>é & untouched</opaque>"})
}
fn fixture() -> Value {
    let mut second = sdk("custom");
    second["version"] = json!("version text, not a JVM target");
    second["homePath"] = json!("<WORKSPACE>/sdk/../custom");
    second["roots"] = json!([
        {"url":"jar:///sdk%20dir/src.zip!/","type":"SOURCES"},
        {"url":"jrt:///jdk!/modules/java.base","type":"CLASSES"},
        {"url":"custom://root/../opaque","type":"CUSTOM"}
    ]);
    json!({"sdks":[sdk("JavaSDK"),second],"libraries":[{"name":"same😀","type":null,"roots":[]}],"modules":[{"name":"same😀","dependencies":[
        {"type":"sdk","name":"same😀","kind":"custom"},
        {"type":"moduleSource"},
        {"type":"library","name":"same😀","scope":"test","isExported":true},
        {"type":"inheritedSdk"},
        {"type":"sdk","name":"same😀","kind":"JavaSDK"}
    ]}]})
}
fn import(value: &Value) -> Result<v2::ImportedWorkspace, v2::Error> {
    v2::from_json(std::env::temp_dir(), &serde_json::to_vec(value).unwrap())
}
#[test]
fn production_consumer_retains_catalog_identity_nulls_urls_and_edge_order() {
    let value = fixture();
    let imported = import(&value).unwrap();
    let project = imported.project_projection();
    assert!(std::ptr::eq(imported.sdks(), project.sdks.as_slice()));
    assert_eq!(
        serde_json::to_value(imported.sdks()).unwrap()[1],
        value["sdks"][1]
    );
    assert_eq!(imported.sdks()[0].version, None);
    assert_eq!(imported.sdks()[0].home_path, None);
    assert_eq!(imported.sdks()[0].roots, None);
    assert_eq!(
        imported.sdks()[0].additional_data,
        "<opaque>é & untouched</opaque>"
    );
    assert_eq!(
        imported.sdks()[1].roots.as_ref().unwrap()[1].url,
        "jrt:///jdk!/modules/java.base"
    );
    assert_eq!(
        imported.modules()[0].dependencies,
        vec![
            v2::Dependency::Sdk {
                name: "same😀".into(),
                kind: "custom".into()
            },
            v2::Dependency::ModuleSource,
            v2::Dependency::Library {
                library: "same😀".into(),
                scope: ktlsp_project::DependencyScope::Test,
                exported: true
            },
            v2::Dependency::InheritedSdk,
            v2::Dependency::Sdk {
                name: "same😀".into(),
                kind: "JavaSDK".into()
            },
        ]
    );
    assert_eq!(
        project.modules[0].dependencies,
        vec![
            ktlsp_project::v2::Dependency::Sdk {
                name: "same😀".into(),
                kind: "custom".into()
            },
            ktlsp_project::v2::Dependency::Library {
                library: "same😀".into(),
                scope: ktlsp_project::DependencyScope::Test,
                exported: true
            },
            ktlsp_project::v2::Dependency::InheritedSdk {},
            ktlsp_project::v2::Dependency::Sdk {
                name: "same😀".into(),
                kind: "JavaSDK".into()
            },
        ]
    );
    assert!(project.sdk_archives.is_empty() && project.libraries[0].binaries.is_empty());
    let bytes = serde_json::to_vec(project).unwrap();
    assert_eq!(
        &ktlsp_project::v2::from_json(std::env::temp_dir().join("reloaded.json"), &bytes).unwrap(),
        project
    );
    let mut value = value;
    value["sdks"][0]["roots"] = Value::Null;
    assert_eq!(import(&value).unwrap().sdks()[0].roots, None);
    value["sdks"][0]["roots"] = json!([]);
    assert_eq!(import(&value).unwrap().sdks()[0].roots, Some(vec![]));
    assert!(
        ktlsp_import::from_json(std::env::temp_dir(), &serde_json::to_vec(&value).unwrap())
            .is_err()
    );
}
#[test]
fn malformed_required_nullable_and_unknown_fields_refuse_atomically() {
    for key in ["name", "type", "version", "homePath", "additionalData"] {
        let mut value = fixture();
        value["sdks"][0].as_object_mut().unwrap().remove(key);
        assert!(
            matches!(import(&value),Err(v2::Error::Import(ImportError::Schema {path,..})) if path==format!("/sdks/0/{key}"))
        );
        value["sdks"][0][key] = json!([]);
        assert!(import(&value).is_err());
    }
    for pointer in [
        "/sdks/0/future",
        "/sdks/1/roots/0/future",
        "/modules/0/dependencies/0/scope",
        "/modules/0/dependencies/3/kind",
    ] {
        let mut value = fixture();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        value.pointer_mut(parent).unwrap()[key] = Value::Null;
        assert!(
            matches!(import(&value),Err(v2::Error::Import(ImportError::Unsupported {fields,..})) if fields.iter().any(|f|f.path==pointer))
        );
    }
    for (key, invalid) in [
        ("name", json!("\0")),
        ("type", json!("")),
        ("version", json!("\0")),
        ("additionalData", json!("\0")),
        ("homePath", json!("<HOME>/jdk")),
        ("homePath", json!("relative")),
        ("homePath", json!("file:///jdk")),
        ("roots", json!([{"url":"/not-a-url","type":"CLASSES"}])),
    ] {
        let mut value = fixture();
        value["sdks"][0][key] = invalid;
        assert!(import(&value).is_err(), "{key}");
    }
    let bytes = br#"{"sdks":[{"name":"x","name":"y","type":"jdk","version":null,"homePath":null,"additionalData":""}]}"#;
    assert!(matches!(
        v2::from_json(std::env::temp_dir(), bytes),
        Err(v2::Error::Import(ImportError::Json(_)))
    ));
    let bytes = serde_json::to_vec(&fixture()).unwrap();
    for cut in 0..bytes.len() {
        assert!(v2::from_json(std::env::temp_dir(), &bytes[..cut]).is_err());
    }
}
#[test]
fn unresolved_duplicate_and_other_rich_semantics_do_not_return_partial_models() {
    for mutation in 0..7 {
        let mut value = fixture();
        match mutation {
            0 => value["sdks"][1] = sdk("JavaSDK"),
            1 => value["sdks"] = json!([]),
            2 => value["modules"][0]["dependencies"][0]["kind"] = json!("missing"),
            3 | 4 => {
                let edge =
                    value["modules"][0]["dependencies"][if mutation == 3 { 0 } else { 3 }].clone();
                value["modules"][0]["dependencies"]
                    .as_array_mut()
                    .unwrap()
                    .push(edge);
            }
            5 => value["kotlinSettings"] = json!([{}]),
            _ => value["modules"][0]["facets"] = json!([{}]),
        }
        assert!(import(&value).is_err(), "mutation {mutation}");
    }
    assert!(
        import(&json!({"modules":[{"name":"app","dependencies":[{"type":"inheritedSdk"}]}]}))
            .is_ok()
    );
    assert!(
        matches!(import(&json!({"sdks":null})),Err(v2::Error::Import(ImportError::Schema {path,..})) if path=="/sdks")
    );
}
#[test]
fn catalog_root_and_total_byte_resource_limits_refuse() {
    let value = json!({"sdks":vec![Value::Null;ktlsp_project::v2::MAX_SDKS+1]});
    assert!(matches!(
        import(&value),
        Err(v2::Error::Import(ImportError::BudgetExceeded(
            "SDK catalog"
        )))
    ));
    let mut value = json!({"sdks":[sdk("jdk")]});
    value["sdks"][0]["roots"] = json!(vec![Value::Null; ktlsp_project::v2::MAX_SDK_ROOTS + 1]);
    assert!(matches!(
        import(&value),
        Err(v2::Error::Import(ImportError::BudgetExceeded("SDK roots")))
    ));
    value["sdks"][0]["roots"] = Value::Null;
    value["sdks"][0]["additionalData"] = json!("x".repeat(ktlsp_import::MAX_BYTES));
    assert!(matches!(
        import(&value),
        Err(v2::Error::Import(ImportError::BudgetExceeded("bytes")))
    ));
}

#[test]
fn exact_real_sdk_and_aggregate_root_boundaries_reach_native_projection() {
    let sdks: Vec<_> = (0..ktlsp_project::v2::MAX_SDKS).map(|index| {
        json!({"name":index.to_string(),"type":"x","version":null,"homePath":null,"additionalData":""})
    }).collect();
    assert_eq!(
        import(&json!({"sdks":sdks})).unwrap().sdks().len(),
        ktlsp_project::v2::MAX_SDKS
    );
    let half = ktlsp_project::v2::MAX_SDK_ROOTS / 2;
    let mut first = sdk("first");
    let mut second = sdk("second");
    first["roots"] = json!(vec![json!({"url":"x://r","type":"c"}); half]);
    second["roots"] = first["roots"].clone();
    let mut value = json!({"sdks":[first,second]});
    assert_eq!(
        import(&value).unwrap().sdks()[1]
            .roots
            .as_ref()
            .unwrap()
            .len(),
        half
    );
    value["sdks"][1]["roots"]
        .as_array_mut()
        .unwrap()
        .push(json!({"url":"x://r","type":"c"}));
    assert!(matches!(
        import(&value),
        Err(v2::Error::Import(ImportError::BudgetExceeded("SDK roots")))
    ));
}

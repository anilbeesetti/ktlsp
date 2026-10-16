use ktlsp_project::{Budget, ProjectError, v2};
use serde_json::{Value, json};

fn sdk(kind: &str) -> Value {
    json!({"name":"shared😀","type":kind,"version":null,"homePath":null,"additionalData":"<opaque x=\"é\"/>"})
}
fn fixture() -> Value {
    json!({"schemaVersion":2,"modules":[{"id":"shared😀","dependencies":[
        {"type":"sdk","name":"shared😀","kind":"JavaSDK"},
        {"type":"library","library":"shared😀","scope":"compile"},
        {"type":"inheritedSdk"},
        {"type":"sdk","name":"shared😀","kind":"custom"}
    ]}],"libraries":[{"id":"shared😀","binaries":[]}],"sdks":[sdk("JavaSDK"),sdk("custom")]})
}
fn parse(value: &Value) -> Result<v2::ProjectModel, v2::Error> {
    v2::from_json(
        std::env::temp_dir().join("sdk-model.json"),
        &serde_json::to_vec(value).unwrap(),
    )
}
#[test]
fn catalog_and_ordered_edges_roundtrip_without_sdk_resolution() {
    let mut value = fixture();
    value["sdks"][0]["homePath"] = json!("<WORKSPACE>/jdks/../jdk😀");
    value["sdks"][0]["version"] = json!("");
    value["sdks"][1]["roots"] = json!([
        {"url":"jrt:///jdk!/modules/java.base","type":"CLASSES"},
        {"url":"jar:///sdk%20dir/src.zip!/","type":"SOURCES"},
        {"url":"custom+sdk://opaque/../root","type":"CUSTOM"}
    ]);
    let model = parse(&value).unwrap();
    assert_eq!(model.sdks.len(), 2);
    assert_eq!(
        model.sdks[0].home_path.as_deref(),
        Some("<WORKSPACE>/jdks/../jdk😀")
    );
    assert_eq!(model.sdks[0].version.as_deref(), Some(""));
    assert_eq!(model.sdks[0].roots, None);
    assert_eq!(
        model.sdks[1].roots.as_ref().unwrap()[0].url,
        "jrt:///jdk!/modules/java.base"
    );
    assert!(model.sdk_archives.is_empty() && model.libraries[0].binaries.is_empty());
    assert!(matches!(
        model.modules[0].dependencies[0],
        v2::Dependency::Sdk { .. }
    ));
    assert_eq!(
        model.modules[0].dependencies[2],
        v2::Dependency::InheritedSdk {}
    );
    let serialized = serde_json::to_value(&model).unwrap();
    assert_eq!(serialized["sdks"][0]["roots"], Value::Null);
    assert_eq!(serialized["sdks"][1], value["sdks"][1]);
    assert_eq!(parse(&serialized).unwrap(), model);
    value["sdks"][0]["roots"] = json!([]);
    assert_eq!(parse(&value).unwrap().sdks[0].roots, Some(vec![]));
}
#[test]
fn malformed_catalog_fields_and_urls_refuse_the_whole_model() {
    for key in ["name", "type", "version", "homePath", "additionalData"] {
        let mut value = fixture();
        value["sdks"][0].as_object_mut().unwrap().remove(key);
        assert!(parse(&value).is_err(), "missing {key}");
        value["sdks"][0][key] = json!(7);
        assert!(parse(&value).is_err(), "typed {key}");
    }
    for (key, invalid) in [
        ("name", json!("")),
        ("type", json!("")),
        ("name", json!("bad\0")),
        ("version", json!("bad\0")),
        ("additionalData", json!("bad\0")),
        ("homePath", json!("relative/jdk")),
        ("homePath", json!("file:///jdk")),
        ("homePath", json!("C:\\jdk")),
        ("homePath", json!("<HOME>/jdk")),
        ("homePath", json!("<WORKSPACE>//jdk")),
        ("homePath", json!("")),
        ("homePath", json!("/jdk\0")),
        ("roots", json!({})),
        ("future", Value::Null),
    ] {
        let mut value = fixture();
        value["sdks"][0][key] = invalid;
        assert!(parse(&value).is_err(), "{key}");
    }
    for url in [
        "",
        "/jdk",
        "file://",
        "1x:///jdk",
        "jrt:///jdk\0",
        "file:///a b",
        "file:///a\\b",
        "file:///a\n",
    ] {
        let mut value = fixture();
        value["sdks"][0]["roots"] = json!([{"url":url,"type":"CLASSES"}]);
        assert!(parse(&value).is_err(), "{url:?}");
    }
    for root in [
        json!({"url":"file:///jdk"}),
        json!({"url":"file:///jdk","type":""}),
        json!({"url":"file:///jdk","type":"CLASSES","future":null}),
    ] {
        let mut value = fixture();
        value["sdks"][0]["roots"] = json!([root]);
        assert!(parse(&value).is_err());
    }
}
#[test]
fn identity_references_duplicates_and_cross_kind_names_are_not_guessed() {
    let mut value = fixture();
    value["sdks"][1] = sdk("JavaSDK");
    assert!(matches!(
        parse(&value),
        Err(v2::Error::Validation(ProjectError::DuplicateId {
            kind: "SDK",
            ..
        }))
    ));
    let mut value = fixture();
    value["sdks"] = json!([]);
    assert!(matches!(
        parse(&value),
        Err(v2::Error::Validation(ProjectError::UnknownReference {
            kind: "SDK",
            ..
        }))
    ));
    let mut value = fixture();
    value["modules"][0]["dependencies"][0]["kind"] = json!("missing");
    assert!(parse(&value).is_err());
    for index in [0, 2] {
        let mut value = fixture();
        let duplicate = value["modules"][0]["dependencies"][index].clone();
        value["modules"][0]["dependencies"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert!(matches!(
            parse(&value),
            Err(v2::Error::Validation(
                ProjectError::DuplicateDependency { .. }
            ))
        ));
    }
    for edge in [
        json!({"type":"sdk","name":"shared😀","kind":"JavaSDK","scope":"compile"}),
        json!({"type":"inheritedSdk","name":"shared😀"}),
    ] {
        let mut value = fixture();
        value["modules"][0]["dependencies"][0] = edge;
        assert!(matches!(
            parse(&value),
            Err(v2::Error::Validation(ProjectError::Json(_)))
        ));
    }
    assert!(parse(&json!({"schemaVersion":2,"modules":[{"id":"app","dependencies":[{"type":"inheritedSdk"}]}]})).is_ok());
    let mut v1 = fixture();
    v1["schemaVersion"] = json!(1);
    assert!(ktlsp_project::from_json("model.json", &serde_json::to_vec(&v1).unwrap()).is_err());
}
#[test]
fn streaming_sdk_collection_limits_precede_typed_allocation() {
    for (field, count, budget) in [
        ("sdks", v2::MAX_SDKS, Budget::Sdks),
        ("roots", v2::MAX_SDK_ROOTS, Budget::SdkRoots),
    ] {
        for excess in [false, true] {
            let entries = vec![Value::Null; count + usize::from(excess)];
            let value = if field == "sdks" {
                json!({"schemaVersion":2,"modules":[],"sdks":entries})
            } else {
                json!({"schemaVersion":2,"modules":[],"sdks":[{"roots":entries}]})
            };
            let error = parse(&value).unwrap_err();
            if excess {
                assert!(
                    matches!(error,v2::Error::Validation(ProjectError::BudgetExceeded {budget:b,..}) if b==budget)
                );
            } else {
                assert!(matches!(
                    error,
                    v2::Error::Validation(ProjectError::Json(_))
                ));
            }
        }
    }
    let mut value = fixture();
    value["sdks"][0]["additionalData"] = json!("x".repeat(ktlsp_project::MAX_MANIFEST_BYTES));
    assert!(matches!(
        parse(&value),
        Err(v2::Error::Validation(ProjectError::BudgetExceeded {
            budget: Budget::ManifestBytes,
            ..
        }))
    ));
}

#[test]
fn exact_sdk_and_shared_root_count_boundaries_admit_real_records() {
    let sdks: Vec<_> = (0..v2::MAX_SDKS).map(|index| {
        json!({"name":index.to_string(),"type":"x","version":null,"homePath":null,"additionalData":""})
    }).collect();
    let model = parse(&json!({"schemaVersion":2,"modules":[],"sdks":sdks})).unwrap();
    assert_eq!(model.sdks.len(), v2::MAX_SDKS);
    let half = v2::MAX_SDK_ROOTS / 2;
    let mut first = sdk("first");
    let mut second = sdk("second");
    first["roots"] = json!(vec![json!({"url":"x://r","type":"c"}); half]);
    second["roots"] = first["roots"].clone();
    let mut value = json!({"schemaVersion":2,"modules":[],"sdks":[first,second]});
    assert_eq!(
        parse(&value).unwrap().sdks[1].roots.as_ref().unwrap().len(),
        half
    );
    value["sdks"][1]["roots"]
        .as_array_mut()
        .unwrap()
        .push(json!({"url":"x://r","type":"c"}));
    assert!(matches!(
        parse(&value),
        Err(v2::Error::Validation(ProjectError::BudgetExceeded {
            budget: Budget::SdkRoots,
            ..
        }))
    ));
}

#[test]
fn sdk_accounting_is_v2_only_and_v1_keeps_strict_unknown_schema_errors() {
    for sdks in [
        json!(vec![Value::Null; v2::MAX_SDKS + 1]),
        json!([{"roots": vec![Value::Null; v2::MAX_SDK_ROOTS + 1]}]),
    ] {
        let bytes =
            serde_json::to_vec(&json!({"schemaVersion":1,"modules":[],"sdks":sdks})).unwrap();
        assert!(matches!(
            ktlsp_project::from_json("v1.json", &bytes),
            Err(ProjectError::Json(_))
        ));
    }
    assert!(ktlsp_project::from_json("v1.json", br#"{"schemaVersion":1,"modules":[]}"#).is_ok());
}

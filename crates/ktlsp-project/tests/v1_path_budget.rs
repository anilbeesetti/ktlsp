use ktlsp_project::{Budget, ProjectError, *};
use serde_json::{Value, json};
use std::cell::Cell;
use std::path::{Path, PathBuf};

const EMPTY: &[u8] = br#"{"schemaVersion":1,"modules":[]}"#;

fn manifest(base_bytes: usize) -> PathBuf {
    let prefix = std::env::temp_dir().join("ktlsp-v1-budget");
    let padding = base_bytes - prefix.as_os_str().as_encoded_bytes().len() - 1;
    let path = prefix.join("b".repeat(padding)).join("m");
    assert_eq!(
        path.parent().unwrap().as_os_str().as_encoded_bytes().len(),
        base_bytes
    );
    path
}

fn budget(error: ProjectError, expected: Budget, expected_limit: usize) {
    assert!(
        matches!(error, ProjectError::BudgetExceeded { budget, limit } if budget == expected && limit == expected_limit),
        "{error:?}"
    );
}

fn parse(path: &Path, value: &Value) -> Result<ProjectModel, ProjectError> {
    from_json(path, &serde_json::to_vec(value).unwrap())
}

#[test]
fn each_collection_accepts_exact_expansion_and_rejects_one_more_byte() {
    let path = manifest(MAX_EXPANDED_PATH_BYTES / 2 - 2);
    for (mut value, pointer) in [
        (
            json!({"schemaVersion":1,"modules":[{"id":"a","sourceRoots":[{"path":"a","kind":"test"},{"path":"b","kind":"test"}]}]}),
            "/modules/0/sourceRoots/1/path",
        ),
        (
            json!({"schemaVersion":1,"modules":[],"libraries":[{"id":"l","binaries":["a","b"]}]}),
            "/libraries/0/binaries/1",
        ),
        (
            json!({"schemaVersion":1,"modules":[],"libraries":[{"id":"l","binaries":[],"sources":["a","b"]}]}),
            "/libraries/0/sources/1",
        ),
        (
            json!({"schemaVersion":1,"modules":[],"sdkArchives":["a","b"]}),
            "/sdkArchives/1",
        ),
    ] {
        assert!(parse(&path, &value).is_ok());
        *value.pointer_mut(pointer).unwrap() = json!("bb");
        budget(
            parse(&path, &value).unwrap_err(),
            Budget::ExpandedPathBytes,
            MAX_EXPANDED_PATH_BYTES,
        );
    }
}

#[test]
fn all_four_collections_share_admission_before_any_model_validation() {
    let path = manifest(MAX_EXPANDED_PATH_BYTES / 4 - 2);
    let mut value = json!({"schemaVersion":1,"modules":[{"id":"a","sourceRoots":[{"path":"a","kind":"test"}]}],"libraries":[{"id":"l","binaries":["b"],"sources":["c"]}],"sdkArchives":["d"]});
    assert!(parse(&path, &value).is_ok());
    value["sdkArchives"][0] = json!("dd");
    budget(
        parse(&path, &value).unwrap_err(),
        Budget::ExpandedPathBytes,
        MAX_EXPANDED_PATH_BYTES,
    );
    value["modules"][0]["id"] = json!("");
    budget(
        parse(&path, &value).unwrap_err(),
        Budget::ExpandedPathBytes,
        MAX_EXPANDED_PATH_BYTES,
    );
    let conflict = json!({"schemaVersion":1,"modules":[{"id":"a","sourceRoots":[{"path":"a","kind":"test"},{"path":"a","kind":"test"}]}],"sdkArchives":["a"]});
    budget(
        parse(&manifest(MAX_EXPANDED_PATH_BYTES / 2 - 2), &conflict).unwrap_err(),
        Budget::ExpandedPathBytes,
        MAX_EXPANDED_PATH_BYTES,
    );
    assert!(matches!(
        parse(&manifest(128), &conflict),
        Err(ProjectError::SourceRootOverlap { .. })
    ));
}

#[test]
fn absolute_paths_exclude_long_base_and_preserve_v1_v2_results_and_errors() {
    let path = manifest(MAX_CALLER_PATH_BYTES - 2);
    let directory = std::env::temp_dir().join("ktlsp-absolute");
    let absolute = directory.join("child/../api.jar");
    let mut value = json!({"schemaVersion":1,"modules":[{"id":"a","sourceRoots":[{"path":absolute,"kind":"test"}]}],"libraries":[{"id":"l","binaries":[absolute],"sources":[absolute]}],"sdkArchives":[absolute]});
    let v1 = parse(&path, &value).unwrap();
    value["schemaVersion"] = json!(2);
    let v2 = v2::from_json(&path, &serde_json::to_vec(&value).unwrap()).unwrap();
    let expected = directory.join("api.jar");
    assert_eq!(v1.modules[0].source_roots[0].path, expected);
    assert_eq!(v1.modules[0].source_roots, v2.modules[0].source_roots);
    assert_eq!(v1.libraries, v2.libraries);
    assert_eq!(v1.sdk_archives, v2.sdk_archives);
    let small_path = manifest(128);
    for invalid in ["", "a\0b"] {
        value["sdkArchives"][0] = json!(invalid);
        assert!(matches!(
            v2::from_json(&small_path, &serde_json::to_vec(&value).unwrap()),
            Err(v2::Error::Validation(ProjectError::InvalidPath(_)))
        ));
        value["schemaVersion"] = json!(1);
        assert!(matches!(
            parse(&small_path, &value),
            Err(ProjectError::InvalidPath(_))
        ));
        value["schemaVersion"] = json!(2);
    }
}

#[test]
fn mixed_absolute_relative_paths_charge_only_relative_base() {
    let absolute = std::env::temp_dir().join("api.jar");
    let absolute_bytes = absolute.as_os_str().as_encoded_bytes().len();
    let path = manifest(MAX_EXPANDED_PATH_BYTES - absolute_bytes - 2);
    let mut value = json!({"schemaVersion":1,"modules":[],"sdkArchives":[absolute,"a"]});
    assert!(parse(&path, &value).is_ok());
    value["sdkArchives"][1] = json!("aa");
    budget(
        parse(&path, &value).unwrap_err(),
        Budget::ExpandedPathBytes,
        MAX_EXPANDED_PATH_BYTES,
    );
}

#[test]
fn caller_exact_and_above_limit_precede_io_and_nul_diagnostic_copies() {
    let exact = manifest(MAX_CALLER_PATH_BYTES - 2);
    assert_eq!(
        exact.as_os_str().as_encoded_bytes().len(),
        MAX_CALLER_PATH_BYTES
    );
    assert!(from_json(&exact, EMPTY).is_ok());
    let excess = manifest(MAX_CALLER_PATH_BYTES - 1);
    for error in [
        from_json(&excess, EMPTY).unwrap_err(),
        load_manifest(&excess).unwrap_err(),
    ] {
        budget(error, Budget::CallerPathBytes, MAX_CALLER_PATH_BYTES);
    }
    let shrinking = exact.parent().unwrap().join("../m");
    budget(
        from_json(shrinking, EMPTY).unwrap_err(),
        Budget::CallerPathBytes,
        MAX_CALLER_PATH_BYTES,
    );
    let mut nul = exact.into_os_string();
    nul.push("\0");
    budget(
        from_json(PathBuf::from(nul), EMPTY).unwrap_err(),
        Budget::CallerPathBytes,
        MAX_CALLER_PATH_BYTES,
    );
    assert!(matches!(
        from_json("a\0b", EMPTY),
        Err(ProjectError::InvalidPath(_))
    ));
    assert!(matches!(
        from_json("", EMPTY),
        Err(ProjectError::InvalidPath(_))
    ));
    assert!(matches!(
        load_manifest("ktlsp-no-such-manifest.json"),
        Err(ProjectError::Io(_))
    ));
}

#[test]
fn native_caller_root_trailing_separator_and_unicode_spellings_are_preserved() {
    let directory = std::env::temp_dir();
    let root = directory.ancestors().last().unwrap();
    assert!(matches!(
        from_json(root, EMPTY),
        Err(ProjectError::InvalidPath(_))
    ));
    let bytes = br#"{"schemaVersion":1,"modules":[],"sdkArchives":["api.jar"]}"#;
    let trailing = directory.join("manifest").join("");
    assert_eq!(
        from_json(trailing, bytes).unwrap().sdk_archives,
        vec![directory.join("api.jar")]
    );
    let caller = directory.join("é😀\n\t\"\\x").join("m");
    assert_eq!(
        from_json(&caller, bytes).unwrap().sdk_archives,
        vec![caller.parent().unwrap().join("api.jar")]
    );
}

#[test]
fn relative_caller_composition_is_checked_before_shrinking_normalization() {
    let cwd = std::env::current_dir().unwrap();
    let cwd_bytes = cwd.as_os_str().as_encoded_bytes().len();
    let padding = MAX_CALLER_PATH_BYTES - cwd_bytes - 1 - "/../m".len();
    let exact = format!("{}/../m", "b".repeat(padding));
    assert!(from_json(&exact, EMPTY).is_ok());
    let excess = format!("{}/../m", "b".repeat(padding + 1));
    budget(
        from_json(&excess, EMPTY).unwrap_err(),
        Budget::CallerPathBytes,
        MAX_CALLER_PATH_BYTES,
    );
    let model = from_json(
        "somewhere/../m",
        br#"{"schemaVersion":1,"modules":[],"sdkArchives":["../api.jar"]}"#,
    )
    .unwrap();
    assert_eq!(model.sdk_archives[0], cwd.parent().unwrap().join("api.jar"));
}

#[test]
fn model_normalization_shrink_does_not_bypass_conservative_admission() {
    let path = manifest(MAX_EXPANDED_PATH_BYTES / 2);
    let value = json!({"schemaVersion":1,"modules":[],"sdkArchives":["../a","../b"]});
    budget(
        parse(&path, &value).unwrap_err(),
        Budget::ExpandedPathBytes,
        MAX_EXPANDED_PATH_BYTES,
    );
    let small = parse(&manifest(128), &value).unwrap();
    assert_eq!(
        small.sdk_archives[0],
        manifest(128).parent().unwrap().parent().unwrap().join("a")
    );
}

#[test]
fn decoded_unicode_and_escaped_keys_paths_use_bytes_not_wire_spelling() {
    let decoded = "é😀\n\t\"\\";
    let path = manifest(MAX_EXPANDED_PATH_BYTES - 1 - decoded.len());
    let bytes =
        br#"{"schemaVersion":1,"modules":[],"sdk\u0041rchives":["\u00e9\ud83d\ude00\n\t\"\\"]}"#;
    let model = from_json(&path, bytes).unwrap();
    assert_eq!(
        model,
        parse(
            &path,
            &json!({"schemaVersion":1,"modules":[],"sdkArchives":[decoded]})
        )
        .unwrap()
    );
    let excess = json!({"schemaVersion":1,"modules":[],"sdkArchives":[format!("{decoded}a")]});
    budget(
        parse(&path, &excess).unwrap_err(),
        Budget::ExpandedPathBytes,
        MAX_EXPANDED_PATH_BYTES,
    );
}

#[test]
fn json_version_and_input_errors_keep_precedence_over_caller_budget() {
    let excess = manifest(MAX_CALLER_PATH_BYTES - 1);
    budget(
        from_json(&excess, &vec![b' '; MAX_MANIFEST_BYTES + 1]).unwrap_err(),
        Budget::ManifestBytes,
        MAX_MANIFEST_BYTES,
    );
    assert!(matches!(
        from_json(&excess, b"{"),
        Err(ProjectError::Json(_))
    ));
    assert!(matches!(
        from_json(&excess, br#"{"schemaVersion":2,"modules":[]}"#),
        Err(ProjectError::UnsupportedSchemaVersion(2))
    ));
    assert!(matches!(
        from_json(
            &excess,
            br#"{"schemaVersion":1,"modules":[],"unknown":true}"#
        ),
        Err(ProjectError::Json(_))
    ));
}

struct ChangingPath {
    calls: Cell<usize>,
    first: PathBuf,
    later: PathBuf,
}

impl AsRef<Path> for ChangingPath {
    fn as_ref(&self) -> &Path {
        let call = self.calls.get();
        self.calls.set(call + 1);
        if call == 0 { &self.first } else { &self.later }
    }
}

#[test]
fn caller_conversion_is_single_for_guard_open_and_base_resolution() {
    let caller = ChangingPath {
        calls: Cell::new(0),
        first: Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multimodule.json"),
        later: manifest(MAX_CALLER_PATH_BYTES - 1),
    };
    let loaded = load_manifest(&caller).unwrap();
    assert_eq!(caller.calls.get(), 1);
    assert_eq!(
        loaded.modules[0].source_roots[0].path,
        caller.first.parent().unwrap().join("src/main/kotlin")
    );
    caller.calls.set(0);
    assert!(from_json(&caller, EMPTY).is_ok());
    assert_eq!(caller.calls.get(), 1);
}

#[cfg(unix)]
#[test]
fn non_unicode_callers_and_lossy_path_errors_preserve_v1_behavior() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let caller = PathBuf::from(OsString::from_vec(b"/tmp/ktlsp-\xff/m".to_vec()));
    let bytes = br#"{"schemaVersion":1,"modules":[],"sdkArchives":["api.jar"]}"#;
    let model = from_json(&caller, bytes).unwrap();
    assert_eq!(
        model.sdk_archives[0],
        caller.parent().unwrap().join("api.jar")
    );
    let relative = PathBuf::from(OsString::from_vec(b"ktlsp-\xff/../m".to_vec()));
    let model = from_json(&relative, bytes).unwrap();
    assert_eq!(
        model.sdk_archives[0],
        std::env::current_dir().unwrap().join("api.jar")
    );
    let invalid = PathBuf::from(OsString::from_vec(b"ktlsp-\xff\0/m".to_vec()));
    assert!(
        matches!(from_json(invalid, EMPTY), Err(ProjectError::InvalidPath(message)) if message.contains('\u{fffd}') && message.contains('\0'))
    );
}

#[test]
fn normalized_serialization_retains_existing_reload_limit_and_small_roundtrip() {
    let path = manifest(128);
    let value = json!({"schemaVersion":1,"modules":[],"sdkArchives":vec!["a";MAX_ARCHIVE_PATHS]});
    let bytes = serde_json::to_vec(&value).unwrap();
    assert_eq!(bytes.len(), 131_120);
    let model = from_json(&path, &bytes).unwrap();
    let serialized = serde_json::to_vec(&model).unwrap();
    assert!(serialized.len() > MAX_MANIFEST_BYTES);
    budget(
        from_json(&path, &serialized).unwrap_err(),
        Budget::ManifestBytes,
        MAX_MANIFEST_BYTES,
    );
    let small = parse(
        &path,
        &json!({"schemaVersion":1,"modules":[],"sdkArchives":["../api.jar"]}),
    )
    .unwrap();
    assert_eq!(
        from_json(manifest(256), &serde_json::to_vec(&small).unwrap()).unwrap(),
        small
    );
}

#[cfg(windows)]
#[test]
fn windows_absolute_rooted_and_prefixed_paths_preserve_both_model_versions() {
    let caller = Path::new(r"C:\base\m");
    for (input, expected) in [
        (r"C:\absolute\..\api.jar", r"C:\api.jar"),
        (r"\api.jar", r"C:\api.jar"),
        (
            r"\\server\share\child\..\api.jar",
            r"\\server\share\api.jar",
        ),
        (r"\\?\C:\child\..\api.jar", r"\\?\C:\api.jar"),
    ] {
        let v1 = parse(
            caller,
            &json!({"schemaVersion":1,"modules":[],"sdkArchives":[input]}),
        )
        .unwrap();
        let v2 = v2::from_json(
            caller,
            &serde_json::to_vec(&json!({"schemaVersion":2,"modules":[],"sdkArchives":[input]}))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(v1.sdk_archives, vec![PathBuf::from(expected)]);
        assert_eq!(v1.sdk_archives, v2.sdk_archives);
    }
    let value = json!({"schemaVersion":1,"modules":[],"sdkArchives":["C:api.jar"]});
    assert!(matches!(
        parse(caller, &value),
        Err(ProjectError::InvalidPath(_))
    ));
    let value = json!({"schemaVersion":2,"modules":[],"sdkArchives":["C:api.jar"]});
    assert!(matches!(
        v2::from_json(caller, &serde_json::to_vec(&value).unwrap()),
        Err(v2::Error::Validation(ProjectError::InvalidPath(_)))
    ));
}

#[cfg(any(windows, target_os = "cygwin"))]
#[test]
fn retained_prefix_paths_share_exact_prejoin_budget_and_preserve_resolution() {
    use std::path::Component;
    let base_bytes = MAX_EXPANDED_PATH_BYTES / 2 - 3;
    let seed = Path::new(r"\\s\share\dir\m");
    let server_bytes = base_bytes - seed.parent().unwrap().as_os_str().as_encoded_bytes().len() + 1;
    let caller = PathBuf::from(format!(r"\\{}\share\dir\m", "s".repeat(server_bytes)));
    let base = caller.parent().unwrap();
    assert_eq!(base.as_os_str().as_encoded_bytes().len(), base_bytes);
    assert!(matches!(
        base.components().next(),
        Some(Component::Prefix(_))
    ));
    assert!(matches!(
        Path::new("/a").components().next(),
        Some(Component::RootDir)
    ));
    let mut value = json!({"schemaVersion":1,"modules":[],"sdkArchives":["/a","/b"]});
    assert!(parse(&caller, &value).is_ok());
    value["sdkArchives"][1] = json!("/bb");
    budget(
        parse(&caller, &value).unwrap_err(),
        Budget::ExpandedPathBytes,
        MAX_EXPANDED_PATH_BYTES,
    );

    let small_caller = Path::new(r"\\server\share\dir\m");
    let v1 = parse(small_caller, &value).unwrap();
    // Rooted-only input retains the UNC share, not the caller's directory.
    assert_eq!(v1.sdk_archives[0], Path::new(r"\\server\share\a"));
    value["schemaVersion"] = json!(2);
    let v2 = v2::from_json(small_caller, &serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(v1.sdk_archives, v2.sdk_archives);
}

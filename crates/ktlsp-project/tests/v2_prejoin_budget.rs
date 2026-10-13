#![cfg(any(windows, target_os = "cygwin"))]

use ktlsp_project::{MAX_MANIFEST_BYTES, ProjectError, v2};
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};

fn manifest(base_bytes: usize) -> PathBuf {
    let seed = Path::new(r"\\s\share\dir\m");
    let server_bytes = base_bytes - seed.parent().unwrap().as_os_str().as_encoded_bytes().len() + 1;
    let caller = PathBuf::from(format!(r"\\{}\share\dir\m", "s".repeat(server_bytes)));
    let base = caller.parent().unwrap();
    assert_eq!(base.as_os_str().as_encoded_bytes().len(), base_bytes);
    assert!(caller.is_absolute());
    assert!(matches!(
        base.components().next(),
        Some(Component::Prefix(_))
    ));
    let root_only = Path::new("/a");
    assert_eq!(root_only.as_os_str().as_encoded_bytes().len(), 2);
    assert!(matches!(
        root_only.components().next(),
        Some(Component::RootDir)
    ));
    #[cfg(target_os = "cygwin")]
    assert!(root_only.is_absolute());
    #[cfg(windows)]
    assert!(!root_only.is_absolute());
    caller
}

fn parse(caller: &Path, value: &Value) -> Result<v2::ProjectModel, v2::Error> {
    let bytes = serde_json::to_vec(value).unwrap();
    assert!(bytes.len() < MAX_MANIFEST_BYTES);
    v2::from_json(caller, &bytes)
}

fn expanded_budget(error: v2::Error) {
    assert!(
        matches!(error, v2::Error::ExpandedPathBudgetExceeded { limit } if limit == v2::MAX_EXPANDED_PATH_BYTES),
        "{error:?}"
    );
}

// Cygwin absolute-root regressions; Windows exercises the existing relative branch.
#[test]
fn each_collection_cygwin_regression_windows_preservation() {
    let limit = v2::MAX_EXPANDED_PATH_BYTES;
    let caller = manifest(limit / 2 - 3);
    assert_eq!(2 * (limit / 2 - 3 + 1 + 2), limit);
    for (mut value, pointer) in [
        (
            json!({"schemaVersion":2,"modules":[{"id":"a","sourceRoots":[{"path":"/a","kind":"test"},{"path":"/b","kind":"test"}]}]}),
            "/modules/0/sourceRoots/1/path",
        ),
        (
            json!({"schemaVersion":2,"modules":[],"libraries":[{"id":"l","binaries":["/a","/b"]}]}),
            "/libraries/0/binaries/1",
        ),
        (
            json!({"schemaVersion":2,"modules":[],"libraries":[{"id":"l","binaries":[],"sources":["/a","/b"]}]}),
            "/libraries/0/sources/1",
        ),
        (
            json!({"schemaVersion":2,"modules":[],"sdkArchives":["/a","/b"]}),
            "/sdkArchives/1",
        ),
    ] {
        assert!(matches!(
            parse(&caller, &value),
            Err(v2::Error::NormalizedOutputBudgetExceeded {
                limit: MAX_MANIFEST_BYTES
            })
        ));
        *value.pointer_mut(pointer).unwrap() = json!("/bb");
        expanded_budget(parse(&caller, &value).unwrap_err());
    }
}

#[test]
fn mixed_precedence_cygwin_regression_windows_preservation() {
    let limit = v2::MAX_EXPANDED_PATH_BYTES;
    let caller = manifest(limit / 4 - 3);
    assert_eq!(4 * (limit / 4 - 3 + 1 + 2), limit);
    let mut value = json!({"schemaVersion":2,"modules":[{"id":"","sourceRoots":[{"path":"/a","kind":"test"}]}],"libraries":[{"id":"l","binaries":["/b"],"sources":["/c"]}],"sdkArchives":["/d"]});
    assert!(matches!(
        parse(&caller, &value),
        Err(v2::Error::Validation(ProjectError::InvalidId { kind: "module", id })) if id.is_empty()
    ));
    value["sdkArchives"][0] = json!("/dd");
    expanded_budget(parse(&caller, &value).unwrap_err());
}

#[test]
fn small_resolution_cygwin_regression_windows_preservation() {
    let caller = Path::new(r"\\server\share\dir\m");
    let value = json!({"schemaVersion":2,"modules":[{"id":"a","sourceRoots":[{"path":"/a","kind":"test"}]}],"libraries":[{"id":"l","binaries":["/b"],"sources":["/c"]}],"sdkArchives":["/d"]});
    let model = parse(caller, &value).unwrap();
    assert_eq!(
        model.modules[0].source_roots[0].path,
        Path::new(r"\\server\share\a")
    );
    assert_eq!(
        model.libraries[0].binaries,
        [PathBuf::from(r"\\server\share\b")]
    );
    assert_eq!(
        model.libraries[0].sources,
        [PathBuf::from(r"\\server\share\c")]
    );
    assert_eq!(model.sdk_archives, [PathBuf::from(r"\\server\share\d")]);

    let absolute = Path::new(r"\\other\share\api.jar");
    assert!(absolute.is_absolute());
    assert!(matches!(
        absolute.components().next(),
        Some(Component::Prefix(_))
    ));
    let value = json!({"schemaVersion":2,"modules":[],"sdkArchives":[absolute]});
    let model = parse(&manifest(v2::MAX_EXPANDED_PATH_BYTES - 2), &value).unwrap();
    assert_eq!(model.sdk_archives, [absolute.to_path_buf()]);
}

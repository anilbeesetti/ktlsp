use ktlsp_oracle_replay::{Evidence, Limits, differences, replay};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

fn metadata() -> Vec<u8> {
    serde_json::to_vec(&json!({"release":"fixture-only", "sha256":"0".repeat(64), "returncode":0}))
        .unwrap()
}

fn records() -> Value {
    json!([
        {"direction":"client","message":{"jsonrpc":"2.0","id":"😀","method":"shutdown"}},
        {"direction":"server","message":{"jsonrpc":"2.0","id":"😀","result":null}},
        {"direction":"client","message":{"jsonrpc":"2.0","method":"exit"}}
    ])
}

fn fixture() -> Evidence {
    Evidence::parse(&serde_json::to_vec(&records()).unwrap(), &metadata()).unwrap()
}

fn command(mode: &str) -> Vec<String> {
    vec![env!("CARGO_BIN_EXE_replay-test-peer").into(), mode.into()]
}

fn limits() -> Limits {
    Limits {
        operation_ms: 300,
        session_ms: 1500,
    }
}

#[test]
fn successful_fragmented_unicode_replay_and_real_mismatches() {
    for mode in ["match", "fragmented"] {
        let report = replay(&fixture(), &command(mode), Limits::default()).unwrap();
        assert_eq!(report["matches"], true);
        assert_eq!(
            report["normalization"],
            "none; original IDs, URIs and array order retained"
        );
    }
    let report = replay(&fixture(), &command("mismatch"), Limits::default()).unwrap();
    assert_eq!(report["matches"], false);
    assert_eq!(
        report["responses"][0]["differences"][0]["pointer"],
        "/result"
    );
    let report = replay(&fixture(), &command("nonzero"), Limits::default()).unwrap();
    assert_eq!(report["exit"]["actual"], 7);
    assert_eq!(report["exit"]["matches"], false);
}

#[test]
fn rejects_malformed_evidence_before_execution() {
    for change in 0..9 {
        let mut data = records();
        match change {
            0 => data[1]["message"]["id"] = json!(99),
            1 => data[0]["direction"] = json!("other"),
            2 => data[0]["message"]["id"] = Value::Null,
            3 => data[1]["message"]["method"] = json!("workspace/configuration"),
            4 => data[2]["message"]["method"] = json!("other"),
            5 => data[1]["message"]["error"] = json!({"code":1}),
            6 => data[0]["message"]["jsonrpc"] = json!("1.0"),
            7 => data[0]["message"]["params"] = json!(17),
            _ => data[0]["message"]["params"] = json!("invalid"),
        }
        assert!(Evidence::parse(&serde_json::to_vec(&data).unwrap(), &metadata()).is_err());
    }
    assert!(Evidence::parse(b"[]", b"{}").is_err());
}

#[test]
fn failures_have_bounded_deadlines_and_cleanup() {
    for mode in [
        "no-read",
        "partial",
        "ignore-exit",
        "stderr-flood",
        "bad-frame",
        "bad-json",
        "server-request",
        "unknown-id",
        "duplicate",
        "notifications-flood",
        "invalid-params",
    ] {
        let started = Instant::now();
        assert!(
            replay(&fixture(), &command(mode), limits()).is_err(),
            "{mode}"
        );
        assert!(started.elapsed() < Duration::from_secs(3), "{mode}");
    }
    // Fill the pipe so a child that never reads also exercises blocked writes.
    let mut data = records();
    data[0]["message"]["params"] = json!({"text":"x".repeat(1024 * 1024)});
    let evidence = Evidence::parse(&serde_json::to_vec(&data).unwrap(), &metadata()).unwrap();
    let started = Instant::now();
    assert!(replay(&evidence, &command("no-read"), limits()).is_err());
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn comparisons_preserve_semantic_values_absence_and_array_order() {
    let expected = json!({"severity":1,"range":{"start":{"character":1}},"edits":[1,2],"ranking":["a","b"],"a/b~":null});
    let actual =
        json!({"severity":2,"range":{"start":{"character":2}},"edits":[2,1],"ranking":["b","a"]});
    let diff = differences(Some(&expected), Some(&actual));
    assert_eq!(diff.len(), 5);
    assert!(
        diff.iter()
            .any(|d| d["pointer"] == "/a~1b~0" && d["actualPresent"] == false)
    );
}

#[test]
fn committed_observations_are_supported_without_fabricated_messages() {
    for directory in ["lifecycle-folding", "standalone-declarations"] {
        Evidence::load(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../docs/research/oracle")
                .join(directory),
        )
        .unwrap();
    }
}

#[test]
fn cli_reports_match_mismatch_and_harness_failure_without_java() {
    let directory = std::env::temp_dir().join(format!("ktlsp-replay-cli-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("transcript.json"),
        serde_json::to_vec(&records()).unwrap(),
    )
    .unwrap();
    fs::write(directory.join("metadata.json"), metadata()).unwrap();
    for (mode, code, outcome) in [
        ("match", 0, "match"),
        ("mismatch", 1, "mismatch"),
        ("bad-frame", 2, "harnessFailure"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_ktlsp-oracle-replay"))
            .arg(&directory)
            .args(["test-only", "--"])
            .args(command(mode))
            .env("PATH", "")
            .env_remove("JAVA_HOME")
            .env_remove("JDK_HOME")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(code));
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["outcome"], outcome);
        assert!(output.stderr.is_empty());
    }
    fs::remove_dir_all(directory).unwrap();
}

use ktlsp_oracle_replay::recorder::{CaptureLimits, record, validate_pr174_inputs};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Inputs(PathBuf);
impl Inputs {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ktlsp-pr174-preparation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        for name in ["declarations.kt", "error-node.kt", "script.json"] {
            fs::copy(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("fixtures/official-pr174-symbols")
                    .join(name),
                path.join(name),
            )
            .unwrap();
        }
        let inputs = Self(path);
        inputs.save(&identity(), &preflight());
        inputs
    }
    fn save(&self, identity: &Value, preflight: &Value) {
        fs::write(self.0.join("identity.json"), identity.to_string()).unwrap();
        fs::write(self.0.join("preflight.json"), preflight.to_string()).unwrap();
    }
    fn refusal(&self) -> String {
        self.validate().unwrap_err()
    }
    fn validate(&self) -> Result<(), String> {
        validate_pr174_inputs(
            &self.0.join("identity.json"),
            &self.0.join("preflight.json"),
            &self.0,
        )
    }
}
impl Drop for Inputs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Fabricated run claims test data consistency; they are never official evidence.
fn identity() -> Value {
    json!({"schemaVersion":1,
        "contractSha256":"65eb16c5bd73c99fe7fc000fddf0242bf6abab09283676612edf5ce9f285b61d",
        "sourceBaseline":"b4fa25286a5fa0d7600b0b0420a7bcae6d520edc",
        "originalRun":{"repository":"anilbeesetti/ktlsp","event":"workflow_dispatch","attempt":1,
            "runId":1,"jobId":2,"suiteId":3,"sourceHead":"1".repeat(40),"sourceTree":"2".repeat(40),
            "workflowSha256":"3".repeat(64),"containerId":"4".repeat(64),"imageSha256":"5".repeat(64),"nonce":"6".repeat(64)}})
}
fn preflight() -> Value {
    json!({"schemaVersion":1,"identity":identity(),
        "pins":{"release":"kotlin-lsp/v263.6379.0","platform":"linux-amd64",
            "archiveSha256":"ab8ca4455dc2fc5fe1a24db2bccc46c104254d2c465155c4251ee65df8f3f7cc",
            "launcherSha256":"adf02baec622b4fcd1e7db3ec96f7c7b3309d5ab59e170e60260d7511222878e",
            "scriptSha256":"930a87ea1fa9c3f6e6a6b6008d010dd9435082329ea8b897a42084d7369ab008",
            "declarationsSha256":"fa935b144a49838fd9e603bfc95462bcf3701eeb19877501d451ca069ce9e233",
            "errorNodeSha256":"c052b78e7603a88f3531f5b06f3a533f8df5394ce8ce9eeab3eb2b3ce3f4bd2c"},
        "command":["/tmp/ktlsp-t045-pr174-host/distribution/kotlin-server-263.6379.0/bin/intellij-server",
            "--stdio","--data-sharing=none","--region=apac","--system-path=/tmp/ktlsp-t045-pr174-host/capture/system"],
        "limits":{"phaseMs":60000,"sessionMs":300000,"cleanupMs":5000,"streamBytes":8388608,"stderrBytes":65536,"messages":1024},
        "outputDirectory":"/tmp/ktlsp-t045-pr174-host/capture"})
}

#[test]
fn consistent_proposed_inputs_validate_without_creating_output() {
    let inputs = Inputs::new();
    let before: Vec<_> = fs::read_dir(&inputs.0)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert!(inputs.validate().is_ok());
    let after: Vec<_> = fs::read_dir(&inputs.0)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(before.len(), after.len());
    for path in before {
        assert!(after.contains(&path));
    }
}

#[test]
fn missing_malformed_mismatched_or_extended_identity_is_refused() {
    let inputs = Inputs::new();
    for (pointer, value) in [
        ("/schemaVersion", json!(2)),
        ("/contractSha256", json!("0".repeat(64))),
        ("/sourceBaseline", json!("0".repeat(40))),
        ("/originalRun/repository", json!("other/repository")),
        ("/originalRun/event", json!("push")),
        ("/originalRun/attempt", json!(2)),
        ("/originalRun/runId", json!(0)),
        ("/originalRun/jobId", json!(0)),
        ("/originalRun/suiteId", json!(0)),
        ("/originalRun/sourceHead", json!("not-a-revision")),
        ("/originalRun/sourceTree", json!("F".repeat(40))),
        ("/originalRun/workflowSha256", json!("x".repeat(64))),
        ("/originalRun/containerId", json!("4".repeat(63))),
        ("/originalRun/imageSha256", json!(null)),
        ("/originalRun/nonce", json!("6".repeat(65))),
    ] {
        let mut changed = identity();
        *changed.pointer_mut(pointer).unwrap() = value;
        let mut receipt = preflight();
        receipt["identity"] = changed.clone();
        inputs.save(&changed, &receipt);
        assert!(inputs.validate().is_err(), "{pointer}");
    }
    let mut changed = identity();
    changed["allowOfficialExecution"] = json!(true);
    inputs.save(&changed, &preflight());
    assert!(inputs.refusal().contains("unknown field"));
    let mut receipt = preflight();
    receipt["identity"]["originalRun"]["nonce"] = json!("7".repeat(64));
    inputs.save(&identity(), &receipt);
    assert!(inputs.refusal().contains("identity mismatch"));
    fs::write(inputs.0.join("identity.json"), b"{").unwrap();
    assert!(inputs.refusal().contains("frozen identity"));
    fs::remove_file(inputs.0.join("identity.json")).unwrap();
    assert!(inputs.validate().is_err());
}

#[test]
fn changed_preflight_or_fixture_bytes_are_rejected() {
    let inputs = Inputs::new();
    for (pointer, value) in [
        ("/schemaVersion", json!(2)),
        ("/pins/archiveSha256", json!("0".repeat(64))),
        ("/command/0", json!(env!("CARGO_BIN_EXE_record-test-peer"))),
        ("/command/1", json!("--help")),
        ("/limits/phaseMs", json!(60001)),
        ("/outputDirectory", json!("/tmp/other-capture")),
    ] {
        let mut receipt = preflight();
        *receipt.pointer_mut(pointer).unwrap() = value;
        inputs.save(&identity(), &receipt);
        assert!(inputs.refusal().contains("mismatch"), "{pointer}");
    }
    inputs.save(&identity(), &preflight());
    for name in ["declarations.kt", "error-node.kt", "script.json"] {
        let path = inputs.0.join(name);
        let original = fs::read(&path).unwrap();
        let mut changed = original.clone();
        changed[0] ^= 1;
        fs::write(&path, &changed).unwrap();
        assert!(inputs.refusal().contains("fixture bytes mismatch"));
        changed = original.clone();
        changed.push(b'\n');
        fs::write(&path, changed).unwrap();
        assert!(inputs.refusal().contains("byte limit"));
        fs::write(&path, original).unwrap();
    }
}

#[test]
fn input_types_and_size_are_checked_before_reading() {
    let inputs = Inputs::new();
    fs::write(inputs.0.join("identity.json"), vec![b' '; 16_385]).unwrap();
    assert!(inputs.refusal().contains("byte limit"));
    fs::remove_file(inputs.0.join("identity.json")).unwrap();
    fs::create_dir(inputs.0.join("identity.json")).unwrap();
    assert!(inputs.refusal().contains("regular file"));
    fs::remove_dir(inputs.0.join("identity.json")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("preflight.json", inputs.0.join("identity.json")).unwrap();
        assert!(inputs.refusal().contains("regular file"));
    }
}

#[test]
fn contradictory_duplicate_pins_and_limits_are_rejected() {
    let inputs = Inputs::new();
    for (key, wrong) in [("archiveSha256", "\"wrong\""), ("phaseMs", "1")] {
        let original = preflight().to_string();
        let field = format!("\"{key}\":");
        let contradictory = original.replace(&field, &format!("{field}{wrong},{field}"));
        assert_ne!(contradictory, original);
        fs::write(inputs.0.join("preflight.json"), contradictory).unwrap();
        assert!(inputs.refusal().contains("duplicate field"), "{key}");
    }
}

#[test]
fn script_embeds_exact_unchanged_source_and_peer_recording_stays_synthetic() {
    let script: Vec<Value> = serde_json::from_slice(include_bytes!(
        "../fixtures/official-pr174-symbols/script.json"
    ))
    .unwrap();
    assert_eq!(script.len(), 8);
    assert_eq!(
        script[2]["params"]["textDocument"]["text"]
            .as_str()
            .unwrap()
            .as_bytes(),
        include_bytes!("../fixtures/official-pr174-symbols/declarations.kt")
    );
    assert_eq!(
        script[4]["params"]["textDocument"]["text"]
            .as_str()
            .unwrap()
            .as_bytes(),
        include_bytes!("../fixtures/official-pr174-symbols/error-node.kt")
    );
    let ids: Vec<_> = script.iter().filter_map(|m| m.get("id")).collect();
    assert_eq!(ids, [&json!(1), &json!(2), &json!(3), &json!(4)]);
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let inputs = Inputs::new();
        let report = record(
            &script,
            &[
                env!("CARGO_BIN_EXE_record-test-peer").into(),
                "nonzero".into(),
            ],
            &inputs.0.join("synthetic-output"),
            CaptureLimits {
                phase_ms: 3000,
                session_ms: 5000,
                cleanup_ms: 500,
                ..CaptureLimits::default()
            },
        )
        .unwrap();
        assert_eq!(
            report["scope"],
            "synthetic-only; official live execution blocked"
        );
        assert_eq!(report["complete"], true);
        assert_eq!(report["exit"]["code"], 7);
        assert!(report.get("officialCaptureSuccess").is_none());
    }
}

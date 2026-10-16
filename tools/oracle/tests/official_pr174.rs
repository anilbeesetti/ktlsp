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

#[test]
fn production_official_cli_refuses_arbitrary_inputs_and_command_without_launch() {
    for args in [
        vec![],
        vec!["--synthetic-only"],
        vec!["--identity", "/tmp/caller.json"],
        vec!["--command", env!("CARGO_BIN_EXE_record-test-peer")],
        vec!["--output", "/tmp/caller-capture"],
    ] {
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_ktlsp-oracle-official-pr174"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8(result.stderr)
                .unwrap()
                .contains("four fixed protected-host argument pairs")
        );
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod synthetic_adapter_mechanics {
    use super::*;
    use ktlsp_oracle_replay::{Evidence, Limits, replay};
    use ktlsp_protocol::read_frame;
    use std::io::Cursor;

    // These executable peer captures exercise the adapter, not the official distribution.
    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn command(mode: &str) -> Vec<String> {
        vec![env!("CARGO_BIN_EXE_record-test-peer").into(), mode.into()]
    }

    fn limits() -> CaptureLimits {
        CaptureLimits {
            phase_ms: 5000,
            session_ms: 20_000,
            cleanup_ms: 500,
            ..CaptureLimits::default()
        }
    }

    fn script() -> Vec<Value> {
        vec![
            json!({"jsonrpc":"2.0","id":"😀","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ]
    }

    fn load_json(directory: &Directory, name: &str) -> Value {
        serde_json::from_slice(&fs::read(directory.0.join(name)).unwrap()).unwrap()
    }

    fn capture(mode: &str, script: &[Value], limits: CaptureLimits) -> (Directory, Value) {
        eprintln!("SYNTHETIC / NONOFFICIAL adapter-mechanics smoke: {mode}");
        let directory = Directory(std::env::temp_dir().join(format!(
            "ktlsp-pr174-synthetic-adapter-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        let report = record(script, &command(mode), &directory.0, limits).unwrap();
        assert_eq!(load_json(&directory, "manifest.json"), report);
        assert_eq!(
            report["scope"],
            "synthetic-only; official live execution blocked"
        );
        assert_eq!(report["officialAcceptance"], false);
        assert_ne!(report["officialCaptureSuccess"], true);
        (directory, report)
    }

    fn assert_nonofficial_metadata(directory: &Directory, returncode: i64) {
        let metadata = load_json(directory, "metadata.json");
        assert_eq!(metadata["release"], "synthetic-only");
        assert_eq!(metadata["sha256"], "0".repeat(64));
        assert_eq!(metadata["returncode"], returncode);
        assert_eq!(
            metadata["scope"],
            "synthetic-only; official live execution blocked"
        );
        assert_eq!(metadata["officialAcceptance"], false);
        assert_eq!(metadata["provenance"]["archive"], "none");
        for field in ["runtime", "containment", "receiver"] {
            assert_eq!(metadata[field], "unverified");
        }
    }

    fn assert_rawrefs(directory: &Directory, report: &Value, script: &[Value]) -> Value {
        let transcript = load_json(directory, "transcript.json");
        let records = transcript.as_array().unwrap();
        let chronology = report["chronology"].as_array().unwrap();
        assert_eq!(records.len(), chronology.len());
        assert_eq!(report["export"]["records"], records.len());
        assert_eq!(
            report["export"]["bytes"],
            fs::metadata(directory.0.join("transcript.json"))
                .unwrap()
                .len()
        );
        let client_raw = fs::read(directory.0.join("client-intended.raw")).unwrap();
        let server_raw = fs::read(directory.0.join("stdout.raw")).unwrap();
        let mut counts = [0usize; 2];
        let mut offsets = [0usize; 2];
        for (record, rawref) in records.iter().zip(chronology) {
            assert_eq!(record["direction"], rawref["direction"]);
            let client = record["direction"] == "client";
            let direction = if client { 0 } else { 1 };
            let index = rawref["frameIndex"].as_u64().unwrap() as usize;
            assert_eq!(index, counts[direction]);
            let frame = &report[if client {
                "clientFrames"
            } else {
                "stdoutFrames"
            }][index];
            let start = frame["start"].as_u64().unwrap() as usize;
            let end = frame["end"].as_u64().unwrap() as usize;
            assert_eq!(start, offsets[direction]);
            let raw = if client { &client_raw } else { &server_raw };
            let mut reader = Cursor::new(&raw[start..end]);
            let payload = read_frame(&mut reader).unwrap().unwrap();
            assert_eq!(reader.position() as usize, end - start);
            assert_eq!(
                serde_json::from_slice::<Value>(&payload).unwrap(),
                record["message"]
            );
            if client {
                assert_eq!(frame["acceptedBytes"], frame["intendedLength"]);
                assert_eq!(frame["persistedBytes"], frame["intendedLength"]);
                assert_eq!(record["message"], script[index]);
            } else {
                assert_eq!(record["message"], frame["message"]);
            }
            counts[direction] += 1;
            offsets[direction] = end;
        }
        assert_eq!(counts[0], script.len());
        assert_eq!(offsets, [client_raw.len(), server_raw.len()]);
        transcript
    }

    #[test]
    fn synthetic_nonofficial_peer_export_load_and_replay_preserve_chronology_and_status() {
        let script = script();
        for mode in ["notices", "exit-burst", "nonzero", "error"] {
            let (directory, report) = capture(mode, &script, limits());
            assert_eq!(report["complete"], true, "{report}");
            assert_eq!(report["export"]["complete"], true);
            let returncode = if mode == "nonzero" { 7 } else { 0 };
            assert_eq!(report["exit"]["code"], returncode);
            assert_nonofficial_metadata(&directory, returncode);
            let transcript = assert_rawrefs(&directory, &report, &script);
            if mode == "notices" {
                assert_eq!(transcript.as_array().unwrap().len(), 103);
                assert_eq!(transcript[0]["message"], script[0]);
                for index in 1..=100 {
                    assert_eq!(transcript[index]["direction"], "server");
                    assert_eq!(transcript[index]["message"]["method"], "notice");
                }
                assert_eq!(transcript[101]["message"]["id"], "😀");
                assert_eq!(transcript[102]["message"], script[1]);
            } else if mode == "exit-burst" {
                assert_eq!(transcript.as_array().unwrap().len(), 103);
                assert_eq!(transcript[1]["message"]["id"], "😀");
                assert_eq!(transcript[2]["message"], script[1]);
                for index in 3..103 {
                    assert_eq!(transcript[index]["direction"], "server");
                    assert_eq!(transcript[index]["message"]["method"], "notice");
                }
            } else if mode == "error" {
                assert_eq!(transcript[1]["message"]["error"]["code"], -32603);
                assert!(transcript[1]["message"].get("result").is_none());
                assert!(
                    fs::read_to_string(directory.0.join("stderr.raw"))
                        .unwrap()
                        .contains("SYNTHETIC / NONOFFICIAL")
                );
            }
            let evidence = Evidence::load(&directory.0).unwrap();
            let replayed = replay(&evidence, &command(mode), Limits::default()).unwrap();
            assert_eq!(replayed["outcome"], "match", "{replayed}");
            assert_eq!(replayed["exit"]["actual"], returncode);
            assert_eq!(replayed["officialMetadata"]["officialAcceptance"], false);
            assert_eq!(replayed["officialMetadata"]["release"], "synthetic-only");
            if mode == "nonzero" {
                let mismatch = replay(&evidence, &command("match"), Limits::default()).unwrap();
                assert_eq!(mismatch["outcome"], "mismatch");
                assert_eq!(mismatch["exit"]["expected"], 7);
                assert_eq!(mismatch["exit"]["actual"], 0);
                assert_eq!(mismatch["exit"]["matches"], false);
                assert_eq!(mismatch["responses"][0]["matches"], true);
            }
        }
    }

    #[test]
    fn premature_response_is_retained_but_cannot_complete_or_reorder_client_acceptance() {
        let script = vec![
            json!({"jsonrpc":"2.0","id":1,"method":"shutdown","params":{"large":"x".repeat(2 * 1024 * 1024)}}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let (directory, report) = capture("premature-response", &script, limits());
        assert_eq!(report["complete"], false);
        assert!(
            report["failureReason"]
                .as_str()
                .unwrap()
                .contains("response before client frame fully accepted"),
            "{report}"
        );
        let frame = &report["clientFrames"][0];
        assert!(
            frame["acceptedBytes"].as_u64().unwrap() < frame["intendedLength"].as_u64().unwrap()
        );
        assert_eq!(
            report["chronology"],
            json!([{"direction":"server","frameIndex":0}])
        );
        assert_eq!(report["stdoutFrames"][0]["message"]["result"], Value::Null);
        assert!(!fs::read(directory.0.join("stdout.raw")).unwrap().is_empty());
        assert!(Evidence::load(&directory.0).is_err());
    }

    #[test]
    fn combined_directional_record_limit_is_consumer_compatible_and_fails_closed() {
        let mut boundary: Vec<Value> = (0..511)
            .map(|id| json!({"jsonrpc":"2.0","id":id,"method":"shutdown"}))
            .collect();
        boundary.push(json!({"jsonrpc":"2.0","method":"initialized"}));
        boundary.push(json!({"jsonrpc":"2.0","method":"exit"}));
        let (directory, report) = capture("match", &boundary, limits());
        assert_eq!(report["complete"], true, "{report}");
        assert_eq!(report["export"]["complete"], true);
        assert_eq!(
            assert_rawrefs(&directory, &report, &boundary)
                .as_array()
                .unwrap()
                .len(),
            1024
        );
        let evidence = Evidence::load(&directory.0).unwrap();
        assert_eq!(
            replay(&evidence, &command("match"), Limits::default()).unwrap()["outcome"],
            "match"
        );

        boundary[511] = json!({"jsonrpc":"2.0","id":511,"method":"shutdown"});
        let (directory, report) = capture("match", &boundary, limits());
        assert_export_failure(&directory, &report, "transcript record limit");
        assert_eq!(report["chronology"].as_array().unwrap().len(), 1025);
        assert_eq!(report["clientFrames"].as_array().unwrap().len(), 513);
        assert_eq!(report["stdoutFrames"].as_array().unwrap().len(), 512);
        assert_eq!(report["exit"]["observed"], true);
    }

    fn assert_export_failure(directory: &Directory, report: &Value, reason: &str) {
        // Synthetic `complete` retains its original transport meaning. Consumer
        // export has separate, stricter combined budgets and cannot claim success.
        assert_eq!(report["complete"], true);
        assert_eq!(report["failureReason"], Value::Null);
        assert_eq!(report["export"]["complete"], false);
        assert!(
            report["export"]["failureReason"]
                .as_str()
                .unwrap()
                .contains(reason)
        );
        assert!(!directory.0.join("transcript.json").exists());
        assert!(Evidence::load(&directory.0).is_err());
        assert!(
            !fs::read(directory.0.join("client-intended.raw"))
                .unwrap()
                .is_empty()
        );
        assert!(!fs::read(directory.0.join("stdout.raw")).unwrap().is_empty());
    }

    #[test]
    fn unaccepted_client_intent_and_protocol_failure_never_become_success() {
        let mut script = script();
        script[0]["params"] = json!({"text":"x".repeat(100_000)});
        let (directory, report) = capture(
            "no-read",
            &script,
            CaptureLimits {
                phase_ms: 200,
                session_ms: 1000,
                ..limits()
            },
        );
        assert_eq!(report["complete"], false);
        assert_eq!(report["scriptCompleted"], false);
        let frame = &report["clientFrames"][0];
        assert!(
            frame["acceptedBytes"].as_u64().unwrap() < frame["intendedLength"].as_u64().unwrap()
        );
        assert_eq!(frame["persistedBytes"], frame["intendedLength"]);
        assert!(report["chronology"].as_array().unwrap().is_empty());
        assert!(Evidence::load(&directory.0).is_err());
        assert_eq!(report["exit"]["observed"], true);

        let (directory, report) = capture("bad-frame-tail", &self::script(), limits());
        assert_eq!(report["complete"], false);
        assert!(
            report["failureReason"]
                .as_str()
                .unwrap()
                .contains("Content-Length")
        );
        assert!(
            fs::read(directory.0.join("stdout.raw"))
                .unwrap()
                .ends_with(b"malformed-tail")
        );
        assert!(Evidence::load(&directory.0).is_err());
        assert_eq!(report["exit"]["observed"], true);
        let (directory, report) = capture("exit-burst-bad", &self::script(), limits());
        assert_eq!(report["complete"], false);
        assert_eq!(report["export"]["complete"], false);
        assert!(!directory.0.join("transcript.json").exists());
        assert!(!directory.0.join("metadata.json").exists());
        assert!(report["stdoutFrames"].as_array().unwrap().len() > 1);
        assert!(Evidence::load(&directory.0).is_err());
    }
}

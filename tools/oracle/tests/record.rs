#![cfg(any(target_os = "linux", target_os = "macos"))]
use ktlsp_oracle_replay::recorder::{CaptureLimits, record};
use ktlsp_protocol::{read_frame, write_frame};
use serde_json::{Value, json};
use std::{
    fs,
    io::Cursor,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "ktlsp-record-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn script() -> Vec<Value> {
    vec![
        json!({"jsonrpc":"2.0","id":"😀","method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ]
}
fn limits() -> CaptureLimits {
    CaptureLimits {
        phase_ms: 1200,
        session_ms: 3000,
        cleanup_ms: 350,
        stream_bytes: 128 * 1024,
        stderr_bytes: 8192,
        messages: 200,
    }
}
fn capture(mode: &str, messages: &[Value], limits: CaptureLimits) -> (Directory, Value) {
    let dir = Directory::new();
    let start = Instant::now();
    let report = record(
        messages,
        &[env!("CARGO_BIN_EXE_record-test-peer").into(), mode.into()],
        &dir.0,
        limits,
    )
    .unwrap();
    assert!(start.elapsed() < Duration::from_secs(5), "{mode}");
    let saved: Value =
        serde_json::from_slice(&fs::read(dir.0.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(saved, report);
    (dir, report)
}
#[test]
fn exact_raw_offsets_fragmentation_and_isolation() {
    for mode in ["match", "fragmented", "notices", "notices-eof", "nonzero"] {
        let (dir, report) = capture(
            mode,
            &script(),
            CaptureLimits {
                phase_ms: 2000,
                ..limits()
            },
        );
        assert_eq!(report["complete"], true, "{mode}: {report}");
        assert_eq!(
            report["exit"]["code"],
            if mode == "nonzero" { 7 } else { 0 }
        );
        assert_eq!(report["stdout"]["eof"], true);
        assert_eq!(report["stderr"]["eof"], true);
        let bytes = fs::read(dir.0.join("stdout.raw")).unwrap();
        let mut offset = 0;
        for frame in report["stdoutFrames"].as_array().unwrap() {
            let start = frame["start"].as_u64().unwrap() as usize;
            let end = frame["end"].as_u64().unwrap() as usize;
            assert_eq!(start, offset);
            let payload = read_frame(&mut Cursor::new(&bytes[start..end]))
                .unwrap()
                .unwrap();
            assert_eq!(
                serde_json::from_slice::<Value>(&payload).unwrap(),
                frame["message"]
            );
            offset = end;
        }
        assert_eq!(offset, bytes.len());
        let intended = fs::read(dir.0.join("client-intended.raw")).unwrap();
        let mut reader = Cursor::new(&intended);
        for message in script() {
            assert_eq!(
                serde_json::from_slice::<Value>(&read_frame(&mut reader).unwrap().unwrap())
                    .unwrap(),
                message
            );
        }
        for frame in report["clientFrames"].as_array().unwrap() {
            assert_eq!(
                frame["acceptedBytes"].as_u64().unwrap(),
                frame["end"].as_u64().unwrap() - frame["start"].as_u64().unwrap()
            );
        }
        for name in ["home", "config", "system", "log", "tmp", "workspace"] {
            assert!(dir.0.join(name).is_dir());
        }
    }
}
#[test]
fn partial_malformed_and_unknown_request_keep_raw_evidence() {
    for mode in [
        "partial-eof",
        "bad-frame-tail",
        "bad-json-tail",
        "server-request",
    ] {
        let (dir, report) = capture(mode, &script(), limits());
        // The first I/O/protocol error depends on scheduling; retained evidence
        // and failure/cleanup facts are the contract, not diagnostic wording.
        assert_eq!(report["complete"], false);
        assert_eq!(report["scriptCompleted"], false);
        assert_eq!(report["export"]["complete"], false);
        assert!(!dir.0.join("transcript.json").exists());
        assert!(!dir.0.join("metadata.json").exists());
        let raw = fs::read(dir.0.join("stdout.raw")).unwrap();
        let expected = match mode {
            "partial-eof" => b"Content-Length: 20\r\n\r\n{".to_vec(),
            "bad-frame-tail" => b"Content-Length: -1\r\n\r\nmalformed-tail".to_vec(),
            "bad-json-tail" => {
                let mut frame = Vec::new();
                write_frame(&mut frame, b"{").unwrap();
                frame.extend_from_slice(b"raw-tail");
                frame
            }
            "server-request" => {
                let mut frame = Vec::new();
                write_frame(
                    &mut frame,
                    &serde_json::to_vec(
                        &json!({"jsonrpc":"2.0","id":9,"method":"workspace/configuration"}),
                    )
                    .unwrap(),
                )
                .unwrap();
                frame
            }
            _ => unreachable!(),
        };
        assert_eq!(raw, expected, "{mode}");
        assert_eq!(report["stdout"]["bytes"], raw.len());
        assert_eq!(report["stdout"]["observedBytes"], raw.len());
        assert_eq!(report["stdout"]["unpersistedBytes"], 0);
        assert_eq!(report["stdout"]["storageFailed"], false);
        assert_eq!(report["stdout"]["truncated"], false);
        assert_eq!(report["stdout"]["eof"], true);
        assert_eq!(report["stdout"]["undecodedEnd"], raw.len());
        assert_eq!(
            report["stdout"]["undecodedStart"],
            if mode == "server-request" {
                raw.len()
            } else {
                0
            }
        );
        assert_eq!(
            report["stdoutFrames"].as_array().unwrap().len(),
            if mode == "server-request" { 1 } else { 0 }
        );
        let intended = fs::read(dir.0.join("client-intended.raw")).unwrap();
        let messages = script();
        let mut offset = 0;
        for (index, frame) in report["clientFrames"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let mut expected = Vec::new();
            write_frame(
                &mut expected,
                &serde_json::to_vec(&messages[index]).unwrap(),
            )
            .unwrap();
            assert_eq!(frame["start"], offset);
            offset += expected.len();
            assert_eq!(frame["end"], offset);
            assert_eq!(frame["persistedBytes"], expected.len());
            assert_eq!(frame["intendedLength"], expected.len());
            assert!(frame["acceptedBytes"].as_u64().unwrap() <= expected.len() as u64);
            assert_eq!(&intended[offset - expected.len()..offset], expected);
        }
        assert_eq!(intended.len(), offset);
        assert_eq!(report["exit"]["observed"], true);
        assert!(report["exit"]["code"].is_i64() || report["exit"]["signal"].is_i64());
        assert_eq!(report["cleanup"]["forcedTermination"], true);
        let cleanup = &report["cleanup"];
        assert!(cleanup["groupKillAttempted"].is_boolean(), "{report}");
        if cleanup["error"].is_null() {
            assert_eq!(cleanup["groupKillAttempted"], true, "{report}");
        } else {
            assert!(
                cleanup["error"].as_str().is_some_and(|s| !s.is_empty()),
                "{report}"
            );
        }
    }
}
#[test]
fn blocked_writes_floods_and_deadlines_are_bounded() {
    let mut large = script();
    large[0]["params"] = json!({"text":"x".repeat(100_000)});
    let (dir, report) = capture("no-read", &large, limits());
    let frame = &report["clientFrames"][0];
    assert!(frame["acceptedBytes"].as_u64().unwrap() < frame["end"].as_u64().unwrap());
    assert_eq!(
        fs::metadata(dir.0.join("client-intended.raw"))
            .unwrap()
            .len(),
        frame["end"].as_u64().unwrap()
    );
    assert_eq!(report["cleanup"]["forcedTermination"], true);
    for mode in ["stdout-flood", "stderr-flood", "both-flood", "no-read"] {
        let (dir, report) = capture(mode, &script(), limits());
        assert_eq!(report["complete"], false);
        assert!(fs::metadata(dir.0.join("stdout.raw")).unwrap().len() <= 128 * 1024 + 1);
        assert!(fs::metadata(dir.0.join("stderr.raw")).unwrap().len() <= 8193);
        assert_eq!(report["exit"]["observed"], true);
    }
    let (_, report) = capture(
        "notices",
        &script(),
        CaptureLimits {
            messages: 4,
            ..limits()
        },
    );
    assert!(
        report["failureReason"]
            .as_str()
            .unwrap()
            .contains("message limit")
    );
}
#[test]
fn child_exit_pipe_eof_and_descendant_termination_are_independent() {
    let (dir, report) = capture(
        "descendant",
        &script(),
        CaptureLimits {
            phase_ms: 2500,
            session_ms: 4000,
            ..limits()
        },
    );
    assert_eq!(report["complete"], false);
    assert_eq!(report["scriptCompleted"], true);
    assert_eq!(report["exit"]["code"], 0);
    assert_eq!(report["cleanup"]["before"]["childExitObserved"], true);
    assert_eq!(report["cleanup"]["before"]["stdoutEof"], false);
    assert_eq!(report["cleanup"]["forcedTermination"], true);
    assert_eq!(report["stdout"]["eof"], true);
    assert_eq!(report["stderr"]["eof"], true);
    let heartbeat = dir.0.join("tmp/heartbeat");
    let before = fs::read(&heartbeat).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(before, fs::read(heartbeat).unwrap());
    let (_, report) = capture("closed-pipes", &script(), limits());
    assert_eq!(report["stdout"]["eof"], true);
    assert_eq!(report["stderr"]["eof"], true);
    assert_eq!(report["exit"]["signal"], 9);
    let (_, report) = capture("signal", &script(), limits());
    assert!(report["exit"]["signal"].as_i64().is_some());
}
#[test]
fn invalid_input_and_collision_never_overwrite_evidence() {
    let (dir, _) = capture("match", &script(), limits());
    let old = fs::read(dir.0.join("manifest.json")).unwrap();
    let command = [
        env!("CARGO_BIN_EXE_record-test-peer").into(),
        "match".into(),
    ];
    assert!(record(&script(), &command, &dir.0, limits()).is_err());
    assert_eq!(old, fs::read(dir.0.join("manifest.json")).unwrap());
    let dir = Directory::new();
    let mut bad = script();
    bad[0]["id"] = Value::Null;
    assert!(record(&bad, &command, &dir.0, limits()).is_err());
    assert!(!dir.0.exists());
    let (_, report) = capture(
        "no-read",
        &script(),
        CaptureLimits {
            phase_ms: 20,
            ..limits()
        },
    );
    assert_eq!(report["complete"], false);
}

#[test]
fn all_shutdown_tail_frames_are_interpreted_before_completion() {
    let script = vec![json!({"jsonrpc":"2.0","method":"exit"})];
    let (_, report) = capture("exit-burst", &script, limits());
    assert_eq!(report["complete"], true, "{report}");
    assert_eq!(report["stdoutFrames"].as_array().unwrap().len(), 100);
    assert_eq!(
        report["stdout"]["undecodedStart"],
        report["stdout"]["bytes"]
    );
    for mode in ["exit-burst-bad", "exit-burst-request"] {
        let (_, report) = capture(mode, &script, limits());
        assert_eq!(report["complete"], false, "{report}");
    }
    let (_, report) = capture(
        "exit-burst",
        &script,
        CaptureLimits {
            messages: 64,
            ..limits()
        },
    );
    assert_eq!(report["complete"], false);
    assert!(
        report["failureReason"]
            .as_str()
            .unwrap()
            .contains("message limit")
    );
}

#[test]
fn valid_stdout_burst_hits_byte_cap_and_silent_descendant_is_killed() {
    let script = vec![json!({"jsonrpc":"2.0","method":"exit"})];
    let (dir, report) = capture(
        "exit-burst",
        &script,
        CaptureLimits {
            stream_bytes: 2048,
            ..limits()
        },
    );
    assert_eq!(report["complete"], false);
    assert!(
        report["failureReason"]
            .as_str()
            .unwrap()
            .contains("byte limit"),
        "{report}"
    );
    assert_eq!(report["stdout"]["truncated"], true);
    assert_eq!(fs::metadata(dir.0.join("stdout.raw")).unwrap().len(), 2049);
    let (dir, report) = capture(
        "descendant-silent",
        &script,
        CaptureLimits {
            phase_ms: 2500,
            session_ms: 4000,
            ..limits()
        },
    );
    assert_eq!(report["complete"], true, "{report}");
    assert_eq!(report["cleanup"]["forcedTermination"], false);
    let path = dir.0.join("tmp/heartbeat");
    let before = fs::read(&path).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(before, fs::read(path).unwrap());
}
#[test]
fn recorder_cli_validates_flag_input_and_exit_status() {
    use std::process::Command;
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/synthetic-recorder/script.json");
    for (mode, code) in [("fragmented", 0), ("bad-frame-tail", 2)] {
        let dir = Directory::new();
        let output = Command::new(env!("CARGO_BIN_EXE_ktlsp-oracle-record"))
            .arg("--synthetic-only")
            .arg(&fixture)
            .arg(&dir.0)
            .arg("--")
            .args([env!("CARGO_BIN_EXE_record-test-peer"), mode])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(code));
        assert!(output.stderr.is_empty());
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["complete"], code == 0);
    }
    assert_eq!(
        Command::new(env!("CARGO_BIN_EXE_ktlsp-oracle-record"))
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
    for input in [PathBuf::from("/dev/null"), std::env::temp_dir()] {
        let dir = Directory::new();
        let output = Command::new(env!("CARGO_BIN_EXE_ktlsp-oracle-record"))
            .arg("--synthetic-only")
            .arg(input)
            .arg(&dir.0)
            .arg("--")
            .arg(env!("CARGO_BIN_EXE_record-test-peer"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(!dir.0.exists());
    }
}
#[test]
#[ignore = "isolated process resource-limit fixture; invoked by storage_failure_preserves_actual_offsets"]
fn storage_limit_child() {
    #[repr(C)]
    struct Rlimit {
        current: u64,
        maximum: u64,
    }
    unsafe extern "C" {
        fn setrlimit(resource: i32, limit: *const Rlimit) -> i32;
        fn signal(signal: i32, handler: usize) -> usize;
    }
    // Process-local limits only; the parent tests and host limits are unchanged.
    unsafe {
        signal(25, 1);
        assert_eq!(
            setrlimit(
                1,
                &Rlimit {
                    current: 4096,
                    maximum: 4096
                }
            ),
            0
        );
    }
    let dir = PathBuf::from(std::env::var_os("KTLSP_STORAGE_FIXTURE").unwrap());
    let report = record(
        &script(),
        &[
            env!("CARGO_BIN_EXE_record-test-peer").into(),
            "stdout-flood".into(),
        ],
        &dir,
        limits(),
    )
    .unwrap();
    assert_eq!(report["complete"], false);
    assert_eq!(report["stdout"]["storageFailed"], true);
    assert_eq!(report["stdout"]["bytes"], 4096);
    assert!(report["stdout"]["unpersistedBytes"].as_u64().unwrap() > 0);
    assert_eq!(report["stdout"]["undecodedEnd"], 4096);
    assert_eq!(fs::metadata(dir.join("stdout.raw")).unwrap().len(), 4096);
    assert!(
        report["failureReason"]
            .as_str()
            .unwrap()
            .contains("evidence write")
    );
}
#[test]
fn storage_failure_preserves_actual_offsets() {
    let dir = Directory::new();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "storage_limit_child", "--ignored"])
        .env("KTLSP_STORAGE_FIXTURE", &dir.0)
        .status()
        .unwrap();
    assert!(status.success());
}

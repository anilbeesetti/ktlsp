//! NONOFFICIAL native mechanics: these tests never establish container calibration.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use ktlsp_oracle_replay::recorder::confinement::{Receipt, abi, profile};
use serde_json::Value;
use std::{
    fs,
    io::Read,
    os::unix::{fs::DirBuilderExt, process::CommandExt},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const OUTPUT_LIMIT: u64 = 64 * 1024;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ktlsp-confinement-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

unsafe extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

fn driver(mode: &str) -> Value {
    fn reader(stream: impl Read + Send + 'static) -> mpsc::Receiver<std::io::Result<Vec<u8>>> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = stream
                .take(OUTPUT_LIMIT + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes);
            let _ = tx.send(result);
        });
        rx
    }
    let root = Directory::new();
    // The driver, not cargo's shared test process, becomes nondumpable.
    let mut child = Command::new(env!("CARGO_BIN_EXE_confinement-test-peer"))
        .arg(mode)
        .arg(&root.0)
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdin.take());
    let stdout = reader(child.stdout.take().unwrap());
    let stderr = reader(child.stderr.take().unwrap());
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            unsafe {
                kill(-(child.id() as i32), 9);
            }
            let _ = child.kill();
            panic!("NONOFFICIAL mechanics driver deadline");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let out = stdout
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .unwrap()
        .unwrap();
    let err = stderr
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .unwrap()
        .unwrap();
    assert!(out.len() as u64 <= OUTPUT_LIMIT && err.len() as u64 <= OUTPUT_LIMIT);
    assert!(
        status.success(),
        "driver exit {status}: {}",
        String::from_utf8_lossy(&err)
    );
    serde_json::from_slice(&out).unwrap()
}

fn receipt(report: &Value) -> Receipt {
    assert_eq!(report["kind"], "NONOFFICIAL_NATIVE_MECHANICS");
    let receipt: Receipt = serde_json::from_value(report["receipt"].clone()).unwrap();
    assert_eq!(receipt.schema_version, 1);
    assert_eq!(receipt.profile, profile());
    assert_eq!(receipt.profile.schema_version, 1);
    assert_eq!(receipt.profile.id, "pr174-evidence-custody-landlock-v1");
    assert_eq!(receipt.profile.minimum_abi, 6);
    // Independent UAPI expectations, not a second sandbox implementation.
    assert_eq!(receipt.profile.handled_access_fs, 0xfff2);
    assert_eq!(receipt.profile.allowed_access_fs, 0x77b2);
    assert_eq!(receipt.profile.handled_access_net, 0);
    assert_eq!(receipt.profile.scoped, 3);
    assert_eq!(receipt.profile.parent_dumpable, 0);
    assert!(receipt.profile.no_new_privs);
    assert!(receipt.profile.close_range_cloexec);
    assert!(receipt.profile.child_only);
    assert_eq!(
        receipt.profile.write_leaves,
        ["home", "config", "system", "log", "tmp", "workspace"]
    );
    assert!(receipt.parent_pid > 0);
    receipt
}

fn denials(probes: &Value) {
    assert!(probes["sameUid"].is_u64());
    assert_eq!(probes["leakedFds"], serde_json::json!([]));
    assert_eq!(
        probes["allowedWriteLeaves"],
        serde_json::json!(["home", "config", "system", "log", "tmp", "workspace"])
    );
    assert_eq!(probes["noNewPrivs"], true);
    assert_eq!(probes["denials"]["inheritedEvidenceFd"]["return"], -1);
    assert_eq!(probes["denials"]["inheritedEvidenceFd"]["errno"], 9);
    for name in [
        "evidenceWrite",
        "evidenceTruncate",
        "evidenceUnlink",
        "evidenceRename",
        "evidenceHardlink",
        "captureRootCreate",
        "outsideWrite",
        "symlinkEvidenceWrite",
        "captureRootSymlink",
        "procParentFd",
        "procParentMem",
        "pidfdGetfd",
        "ptraceSeize",
        "processVmWritev",
        "signalZero",
        "signalBenign",
        "pidfdSignalZero",
        "pidfdSignalBenign",
        "abstractUnixConnect",
    ] {
        let observation = &probes["denials"][name];
        assert_eq!(observation["return"], -1, "{name}: {probes}");
        assert!(
            matches!(observation["errno"].as_i64(), Some(1 | 13 | 18)),
            "{name}: {observation}"
        );
    }
}

#[test]
fn actual_production_hook_confines_same_uid_child_and_detached_exec_descendant() {
    let observed = abi().ok();
    let report = driver("driver");
    let receipt = receipt(&report);
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        assert_eq!(report["outcome"], "REFUSED_NO_CHILD_ENTRY");
        assert_eq!(report["actualDenials"], false);
        assert_eq!(report["childEntered"], false);
        assert_eq!(receipt.hook_outcome, "REFUSED_NO_OFFICIAL_EXEC");
        assert_ne!(receipt.hook_outcome, "ENFORCED_BEFORE_EXEC");
        // A blocked native facility is evidence of refusal only, never successful
        // denial mechanics. The report makes that distinction machine-readable.
        eprintln!("NONOFFICIAL mechanics NOT EXERCISED: production hook refused, ABI={observed:?}");
        return;
    }
    assert!(observed.is_some_and(|value| value >= 6));
    assert_eq!(report["actualDenials"], true);
    assert_eq!(report["outcome"], "ENFORCED");
    assert_eq!(receipt.landlock_abi, observed);
    assert_eq!(receipt.parent_dumpable, Some(0));
    assert_eq!(receipt.hook_outcome, "ENFORCED_BEFORE_EXEC");
    assert_eq!(report["evidenceUnchanged"], true);
    assert_eq!(report["parentMemoryUnchanged"], true);
    assert_eq!(report["parentAlive"], true);
    assert_eq!(report["parentSignals"], 0);
    denials(&report["probes"]);
    denials(&report["probes"]["descendant"]);
    assert_eq!(report["probes"]["sameUid"], report["parentUid"]);
    assert_eq!(
        report["probes"]["descendant"]["sameUid"],
        report["parentUid"]
    );
    assert_eq!(report["probes"]["descendant"]["detachedSession"], true);
    assert_eq!(
        report["probes"]["descendant"]["sessionId"],
        report["probes"]["descendant"]["pid"]
    );
}

#[test]
fn production_hook_refuses_symlinked_write_leaf_before_child_entry() {
    let report = driver("invalid-leaf");
    let receipt = receipt(&report);
    assert_eq!(report["outcome"], "REFUSED_NO_CHILD_ENTRY");
    assert_eq!(report["actualDenials"], false);
    assert_eq!(report["childEntered"], false);
    assert_eq!(receipt.hook_outcome, "REFUSED_NO_OFFICIAL_EXEC");
}

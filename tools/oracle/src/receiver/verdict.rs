use super::retrieval::{AuthenticatedArtifact, safe_name};
use super::{FILE_COUNT, Files, Result, file, hash, json_bytes, require, transport};
use crate::authority::{self, CalibrationReport, GrantScope, RootGrant};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub use crate::authority::{ExportBinding, ExportFile, OriginalRun};
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate {
    pub passed: bool,
    pub reason: String,
}
impl Gate {
    fn from(result: Result<()>) -> Self {
        match result {
            Ok(()) => Self {
                passed: true,
                reason: "verified".into(),
            },
            Err(reason) => Self {
                passed: false,
                reason,
            },
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    pub schema_version: u8,
    pub artifact_id: u64,
    pub artifact_sha256: String,
    pub provenance: Gate,
    pub transport: Gate,
    pub runtime: Gate,
    pub teardown: Gate,
    pub calibration: Gate,
    pub useful_symbols: bool,
    pub official_success: bool,
    pub trust_boundary: &'static str,
}

/// Only fixed authenticated retrieval constructs AuthenticatedArtifact in production.
/// Expected grants are separate ROOT input, never loaded from the received artifact.
pub fn evaluate(
    a: &AuthenticatedArtifact,
    admission: &super::ReceiverAdmission,
    calibrations: &[(super::ReceiverAdmission, AuthenticatedArtifact)],
) -> Verdict {
    let expected = &admission.grant;
    let bound = authenticated_provenance(a, admission);
    let provenance = Gate::from(bound.as_ref().map(|_| ()).map_err(Clone::clone));
    let checked_transport = transport(&a.files, true);
    let useful_symbols = checked_transport.as_ref().is_ok_and(|t| t.useful_symbols);
    let transport = Gate::from(checked_transport.map(|_| ()));
    let runtime = Gate::from(
        bound
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|b| runtime(&a.files, b, expected)),
    );
    let teardown = Gate::from(
        bound
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|b| teardown(&a.files, b, true)),
    );
    let calibration = Gate::from(
        bound
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|_| calibration(&a.files, expected, calibrations)),
    );
    let official_success = expected.scope == GrantScope::OfficialCapture
        && provenance.passed
        && transport.passed
        && runtime.passed
        && teardown.passed
        && calibration.passed
        && useful_symbols;
    Verdict {
        schema_version: 1,
        artifact_id: a.artifact_id,
        artifact_sha256: a.archive_sha256.clone(),
        provenance,
        transport,
        runtime,
        teardown,
        calibration,
        useful_symbols,
        official_success,
        trust_boundary: "authenticated GitHub original run/attempt/job plus independently ROOT-admitted reviewed workflow and protected original-job OS-root export; not cryptographic source-to-binary attestation; OS-accepted client writes do not prove peer consumption",
    }
}

pub fn provenance(files: &Files, expected: &RootGrant) -> Result<ExportBinding> {
    expected.validate_shape()?;
    let b: ExportBinding = serde_json::from_value(json_bytes(file(files, "export-binding.json")?)?)
        .map_err(|e| e.to_string())?;
    require(
        b.schema_version == 1
            && b.scope == "PR174_PROTECTED_EXPORT"
            && b.run.repository == expected.repository
            && b.run.event == expected.event
            && b.run.attempt == expected.attempt
            && b.run.run_id == expected.run_id
            && b.run.job_id == expected.job_id
            && b.run.suite_id == expected.suite_id
            && b.run.source_head == expected.source_head
            && b.run.source_tree == expected.source_tree
            && b.workflow_sha256 == expected.workflow_sha256
            && b.image == expected.image
            && b.image_id == expected.image_id,
        "protected original run/job/head/workflow/image binding mismatch",
    )?;
    require(
        authority::digest(&b.nonce)
            && authority::digest(&b.container_id)
            && !b.boot_id.is_empty()
            && b.boot_id.len() <= 128
            && !b.daemon_id.is_empty()
            && b.daemon_id.len() <= 128,
        "invalid nonce/CID/boot/daemon",
    )?;
    require(
        b.files.len() < FILE_COUNT && b.files.len() + 1 == files.len(),
        "protected export inventory mismatch",
    )?;
    let mut names = BTreeSet::new();
    for entry in &b.files {
        require(
            safe_name(&entry.path)
                && entry.path != "export-binding.json"
                && names.insert(&entry.path)
                && authority::digest(&entry.sha256),
            "unsafe/duplicate export inventory",
        )?;
        let raw = file(files, &entry.path)?;
        require(
            raw.len() as u64 == entry.bytes && hash(raw) == entry.sha256,
            "independent exported file hash/size mismatch",
        )?;
    }
    let grant: RootGrant = authority::decode(file(files, "root-authorization.json")?)?;
    grant.validate(expected)?;
    require(
        hash(file(files, "root-authorization.json")?) == b.root_authorization_sha256
            && hash(file(files, "calibration.json")?) == b.calibration_sha256
            && hash(file(files, "workflow.yml")?) == expected.workflow_sha256,
        "ROOT/calibration/workflow seal mismatch",
    )?;
    let admission = json_bytes(file(files, "admission.json")?)?;
    let scope = if expected.scope == GrantScope::OfficialCapture {
        "PR174_HOST_EXECUTION_ADMITTED"
    } else {
        "PR174_RUST_CALIBRATION_ADMITTED"
    };
    require(
        admission["schemaVersion"] == 1
            && admission["scope"] == scope
            && admission["run"] == serde_json::to_value(&b.run).map_err(|e| e.to_string())?,
        "Admission original scope/run mismatch",
    )?;
    for (key, value) in [
        ("nonce", &b.nonce),
        ("bootId", &b.boot_id),
        ("daemonId", &b.daemon_id),
        ("image", &b.image),
        ("imageId", &b.image_id),
        ("workflowSha256", &b.workflow_sha256),
        ("rootAuthorizationSha256", &b.root_authorization_sha256),
        ("calibrationSha256", &b.calibration_sha256),
        ("contractSha256", &expected.contract_sha256),
        ("hostBinarySha256", &expected.host_binary_sha256),
        ("adapterSha256", &expected.adapter_sha256),
    ] {
        require(admission[key] == *value, "Admission sealed field mismatch")?;
    }
    let binding = json_bytes(file(files, "binding.json")?)?;
    for (key, value) in [
        ("nonce", &b.nonce),
        ("bootId", &b.boot_id),
        ("daemonId", &b.daemon_id),
        ("containerId", &b.container_id),
    ] {
        require(binding[key] == *value, "cleanup binding identity mismatch")?;
    }
    for tool in &expected.tools {
        require(
            admission["tools"][&tool.role] == tool.path,
            "Admission original tool role mismatch",
        )?;
        if ["docker", "curl", "systemctl"].contains(&tool.role.as_str()) {
            require(
                binding[&tool.role] == tool.path,
                "cleanup original tool binding mismatch",
            )?;
        }
    }
    if expected.scope == GrantScope::OfficialCapture {
        require(
            expected.required_calibration_sha256.as_deref() == Some(b.calibration_sha256.as_str()),
            "unadmitted calibration report",
        )?;
        let identity = json_bytes(file(files, "preflight/identity.json")?)?;
        let preflight = file(files, "preflight/host-preflight.json")?;
        let authorization = json_bytes(file(files, "preflight/launch-authorization.json")?)?;
        let metadata = json_bytes(file(files, "capture/metadata.json")?)?;
        let original = &identity["originalRun"];
        let run = serde_json::to_value(&b.run).map_err(|e| e.to_string())?;
        for (key, value) in run.as_object().ok_or("invalid original run")? {
            require(
                original[key] == *value,
                "adapter original-run handoff mismatch",
            )?;
        }
        require(
            identity["schemaVersion"] == 1
                && identity["contractSha256"] == expected.contract_sha256
                && original["nonce"] == b.nonce
                && original["containerId"] == b.container_id
                && original["workflowSha256"] == expected.workflow_sha256
                && original["imageSha256"]
                    == expected
                        .image
                        .rsplit_once("@sha256:")
                        .ok_or("image digest")?
                        .1
                && authorization["identity"] == *original
                && authorization["preflightSha256"] == hash(preflight)
                && authorization["rootAuthorizationSha256"] == b.root_authorization_sha256
                && authorization["contractSha256"] == expected.contract_sha256
                && authorization["bootId"] == b.boot_id
                && authorization["daemonId"] == b.daemon_id
                && metadata["identity"] == identity
                && metadata["authorization"] == authorization,
            "protected adapter handoff mismatch",
        )?;
    }
    Ok(b)
}
fn lines(files: &Files, path: &str, cap: usize) -> Result<Vec<Value>> {
    let raw = file(files, path)?;
    require(
        !raw.is_empty() && raw.len() <= cap && raw.ends_with(b"\n"),
        "missing/truncated/oversized host evidence",
    )?;
    let mut records = Vec::new();
    for line in raw.split_inclusive(|b| *b == b'\n') {
        require(
            records.len() < 4096 && line.len() > 1,
            "host evidence record limit/empty record",
        )?;
        records.push(json_bytes(line)?);
    }
    Ok(records)
}
fn unhex(v: &Value) -> Result<Vec<u8>> {
    let s = v.as_str().ok_or("missing hex bytes")?;
    require(
        s.len().is_multiple_of(2) && s.len() <= 2 * 1024 * 1024,
        "hex byte bound",
    )?;
    s.as_bytes()
        .chunks_exact(2)
        .map(|b| {
            let a = (b[0] as char).to_digit(16).ok_or("invalid hex")?;
            let c = (b[1] as char).to_digit(16).ok_or("invalid hex")?;
            Ok((a * 16 + c) as u8)
        })
        .collect()
}
fn text_hex(v: &Value) -> Result<String> {
    String::from_utf8(unhex(v)?).map_err(|e| e.to_string())
}
fn command_output(v: &Value) -> Result<Vec<u8>> {
    let r = &v["result"];
    require(
        v["kind"] == "command"
            && r["exitCode"] == 0
            && r.get("signal") == Some(&Value::Null)
            && r["EOF"] == json!([true, true])
            && r["timeout"] == false
            && r["truncated"] == false
            && r["reaped"] == true,
        "incomplete/nonzero host command",
    )?;
    unhex(&r["stdoutHex"])
}
fn docker_args(v: &Value, tail: &[&str]) -> bool {
    let mut args = vec!["--host=unix:///run/docker.sock"];
    args.extend_from_slice(tail);
    v["args"] == json!(args)
}
fn original_stat(raw: &str) -> Result<(u64, u64)> {
    let (pid, rest) = raw.split_once(" (").ok_or("invalid proc stat")?;
    let (_, fields) = rest.rsplit_once(") ").ok_or("invalid proc stat comm")?;
    let fields: Vec<_> = fields.split_whitespace().collect();
    require(fields.len() >= 20, "partial proc stat")?;
    Ok((
        pid.parse().map_err(|_| "invalid proc PID")?,
        fields[19].parse().map_err(|_| "invalid proc starttime")?,
    ))
}
fn runtime(files: &Files, b: &ExportBinding, expected: &RootGrant) -> Result<()> {
    let observations = lines(files, "receipts/observer.evidence", 786432)?;
    let custody = writer_custody(files, b, expected, &observations)?;
    let identity = json_bytes(file(files, "process-identity.json")?)?;
    require(
        identity["cgroup"] == format!("/system.slice/docker-{}.scope", b.container_id),
        "original cgroup/CID mismatch",
    )?;
    let mut roles = BTreeSet::new();
    let mut jvm = false;
    let mut normal = false;
    let mut original_recorder = None;
    let mut launcher_parent = None;
    let mut config_seen = false;
    let mut resources = BTreeSet::new();
    for r in &observations {
        if r["kind"] == "original-process" {
            let stat = original_stat(&text_hex(&r["statHex"])?)?;
            require(
                r["hostPid"].as_u64() == Some(stat.0)
                    && r["starttime"].as_u64() == Some(stat.1)
                    && r["namespace"] == identity["pidNamespace"]
                    && text_hex(&r["cgroupHex"])?.trim()
                        == format!(
                            "0::{}",
                            identity["cgroup"].as_str().ok_or("missing cgroup")?
                        ),
                "original process identity mismatch",
            )?;
            let status = text_hex(&r["statusHex"])?;
            for (name, expected) in [
                ("NoNewPrivs:", "1"),
                ("Seccomp:", "2"),
                ("CapEff:", "0000000000000000"),
                ("CapPrm:", "0000000000000000"),
            ] {
                require(
                    status
                        .lines()
                        .find_map(|l| l.strip_prefix(name))
                        .is_some_and(|s| s.trim() == expected),
                    "original security status mismatch",
                )?;
            }
            for name in ["Uid:", "Gid:"] {
                require(
                    status
                        .lines()
                        .find_map(|l| l.strip_prefix(name))
                        .is_some_and(|s| s.split_whitespace().collect::<Vec<_>>() == ["65532"; 4]),
                    "original process UID/GID mismatch",
                )?;
            }
            let limits = text_hex(&r["limitsHex"])?;
            require(
                limits
                    .lines()
                    .find_map(|l| l.strip_prefix("Max core file size"))
                    .is_some_and(|s| {
                        s.split_whitespace().take(2).collect::<Vec<_>>() == ["0", "0"]
                    }),
                "original core limit mismatch",
            )?;
            let role = r["role"]
                .as_str()
                .ok_or("host missing independently joinable executable/JVM inode measurements")?;
            if role == "recorder" {
                require(
                    r["procDirectoryUid"] == 0 && r["procDirectoryGid"] == 0,
                    "original recorder proc directory lacks independent nondumpable custody",
                )?;
                let nspid = status
                    .lines()
                    .find_map(|l| l.strip_prefix("NSpid:"))
                    .ok_or("actual recorder namespace PID absent")?
                    .split_whitespace()
                    .map(str::parse::<u64>)
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|_| "invalid recorder namespace PIDs")?;
                require(
                    nspid.len() == 2
                        && nspid[0] == stat.0
                        && custody["parentPid"].as_u64() == nspid.last().copied(),
                    "measured confined-child writer is not original recorder",
                )?;
            }
            require(
                ["recorder", "launcher"].contains(&role)
                    && r["exeDevice"].as_u64().is_some()
                    && r["exeInode"].as_u64().is_some_and(|n| n > 0)
                    && r["exeDevice"] == r["sealedExecutableDevice"]
                    && r["exeInode"] == r["sealedExecutableInode"],
                "sealed original executable identity mismatch",
            )?;
            let argv = unhex(&r["cmdlineHex"])?;
            let expected_argv = if role == "launcher" {
                vec![
                    "/tmp/ktlsp-t045-pr174-host/distribution/kotlin-server-263.6379.0/bin/intellij-server",
                    "--stdio",
                    "--data-sharing=none",
                    "--region=apac",
                    "--system-path=/tmp/ktlsp-t045-pr174-host/capture/system",
                ]
            } else {
                vec![
                    "/tmp/ktlsp-t045-pr174-host/tool/ktlsp-oracle-official-pr174",
                    "--identity",
                    "/tmp/ktlsp-t045-pr174-host/preflight/identity.json",
                    "--preflight",
                    "/tmp/ktlsp-t045-pr174-host/preflight/host-preflight.json",
                    "--authorization",
                    "/tmp/ktlsp-t045-pr174-host/preflight/launch-authorization.json",
                    "--fixture-directory",
                    "/tmp/ktlsp-t045-pr174-host/fixture",
                ]
            };
            let mut sealed = expected_argv.join("\0").into_bytes();
            sealed.push(0);
            require(argv == sealed, "original process argv mismatch")?;
            let ancestry = &r["ancestry"];
            let init = original_stat(&text_hex(&ancestry["initStatHex"])?)?;
            require(
                Some(init.0) == identity["initPid"].as_u64()
                    && Some(init.1) == identity["initStart"].as_u64(),
                "original init ancestry mismatch",
            )?;
            let parent = original_stat(&text_hex(&ancestry["parentStatHex"])?)?;
            require(
                Some(parent.0) == ancestry["parentHostPid"].as_u64()
                    && Some(parent.1) == ancestry["parentStarttime"].as_u64(),
                "original parent identity mismatch",
            )?;
            let stat_text = text_hex(&r["statHex"])?;
            let actual_parent = stat_text
                .rsplit_once(") ")
                .and_then(|(_, s)| s.split_whitespace().nth(1))
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or("missing original parent PID")?;
            require(actual_parent == parent.0, "original parent PID mismatch")?;
            if role == "recorder" {
                require(
                    parent == init && original_recorder.is_none_or(|previous| previous == stat),
                    "recorder original identity changed/not init child",
                )?;
                original_recorder = Some(stat);
            } else {
                require(
                    launcher_parent.is_none_or(|previous| previous == parent),
                    "launcher parent changed",
                )?;
                launcher_parent = Some(parent);
            }
            roles.insert(role);
            if role == "launcher" && r["pinnedLibjvmMapped"] == true {
                require(
                    r["sealedJvmDevice"] == r["namespaceJvmDevice"]
                        && r["sealedJvmInode"] == r["namespaceJvmInode"]
                        && r["sealedJvmInode"].as_u64().is_some_and(|n| n > 0),
                    "namespace JVM inode mismatch",
                )?;
                let device = r["sealedJvmDevice"].as_u64().ok_or("missing JVM device")?;
                let inode = r["sealedJvmInode"].as_u64().ok_or("missing JVM inode")?;
                let major = ((device >> 8) & 0xfff) | ((device >> 32) & 0xfffff000);
                let minor = (device & 0xff) | ((device >> 12) & 0xffffff00);
                let maps = text_hex(&r["mapsHex"])?;
                let mut mapped = false;
                for line in maps.lines().filter(|l| l.contains("libjvm.so")) {
                    let p: Vec<_> = line.split_whitespace().collect();
                    let device_match =
                        p.get(3)
                            .and_then(|s| s.split_once(':'))
                            .is_some_and(|(a, b)| {
                                u64::from_str_radix(a, 16).ok() == Some(major)
                                    && u64::from_str_radix(b, 16).ok() == Some(minor)
                            });
                    require(
                        p.len() == 6
                            && device_match
                            && p[4].parse::<u64>().ok() == Some(inode)
                            && p[5]
                                == "/tmp/ktlsp-t045-pr174-host/distribution/kotlin-server-263.6379.0/jbr/lib/server/libjvm.so",
                        "deleted/substituted original JVM mapping",
                    )?;
                    mapped = true;
                }
                jvm |= mapped;
            }
        }
        if r["kind"] == "capture-host-observation" {
            normal |= r["containerId"] == b.container_id
                && r["originalNormalExitZero"] == true
                && r["pinnedLibjvmObserved"] == true;
        }
        if r["kind"] == "effective-cgroup-resource" {
            let name = r["name"].as_str().ok_or("missing resource name")?;
            let wanted = match name {
                "memory.max" => "4294967296",
                "memory.swap.max" => "0",
                "pids.max" => "256",
                "cpu.max" => "200000 100000",
                _ => return Err("unexpected resource".into()),
            };
            require(
                text_hex(&r["rawHex"])?.trim() == wanted && resources.insert(name),
                "effective resource bound mismatch/duplicate",
            )?;
        }
        if r["kind"] == "command" && docker_args(r, &["container", "inspect", &b.container_id]) {
            let output = json_bytes(&command_output(r)?)?;
            let v = output
                .as_array()
                .filter(|v| v.len() == 1)
                .and_then(|v| v.first())
                .ok_or("runtime inspect ambiguous")?;
            require(
                v["Id"] == b.container_id
                    && v["Image"] == expected.image_id
                    && v["Config"]["User"] == "65532:65532"
                    && v["Config"]["Image"] == expected.image
                    && v["HostConfig"]["ReadonlyRootfs"] == true
                    && v["HostConfig"]["Privileged"] == false
                    && v["HostConfig"]["Init"] == true
                    && v["HostConfig"]["NetworkMode"] == "none"
                    && v["HostConfig"]["PidMode"] == ""
                    && v["HostConfig"]["IpcMode"] == "private"
                    && v["HostConfig"]["CgroupnsMode"] == "private"
                    && v["HostConfig"]["CapDrop"] == json!(["ALL"])
                    && v["HostConfig"]["Memory"] == 4294967296u64
                    && v["HostConfig"]["MemorySwap"] == 4294967296u64
                    && v["HostConfig"]["PidsLimit"] == 256
                    && v["HostConfig"]["NanoCpus"] == 2000000000u64
                    && v["HostConfig"]["RestartPolicy"]["Name"] == "no"
                    && v["AppArmorProfile"] == "docker-default",
                "actual image/config/containment mismatch",
            )?;
            config_seen = true;
        }
    }
    require(
        roles == BTreeSet::from(["recorder", "launcher"])
            && original_recorder.is_some()
            && launcher_parent == original_recorder
            && config_seen
            && resources.len() == 4
            && jvm
            && normal
            && expected.scope == GrantScope::OfficialCapture,
        "original mapped JVM/config/resources/ancestry/normal host observation missing",
    )
}
fn cgroup_closure(
    r: &Value,
    stage: &str,
    path: &Value,
    device: &Value,
    inode: &Value,
) -> Result<bool> {
    if r["kind"] != "original-cgroup-closure" || r["stage"] != stage {
        return Ok(false);
    }
    require(
        r["path"] == *path && r["originalDevice"] == *device && r["originalInode"] == *inode,
        "closure cgroup original identity mismatch",
    )?;
    if r["attributableAbsent"] == true {
        return Ok(true);
    }
    require(
        r["attributableAbsent"] == false && r["empty"] == true,
        "nonempty/unknown cgroup closure",
    )?;
    let rows = r["rows"].as_array().ok_or("closure rows absent")?;
    require(!rows.is_empty() && rows.len() <= 256, "closure row bounds")?;
    let mut paths = BTreeSet::new();
    for row in rows {
        let p = row["path"].as_str().ok_or("row path missing")?;
        let root = format!(
            "/sys/fs/cgroup{}",
            path.as_str().ok_or("cgroup path missing")?
        );
        require(
            (p == root || p.strip_prefix(&root).is_some_and(|s| s.starts_with('/')))
                && paths.insert(p)
                && text_hex(&row["procsHex"])?.trim().is_empty()
                && text_hex(&row["threadsHex"])?.trim().is_empty(),
            "foreign/duplicate/nonempty cgroup row",
        )?;
        let events = text_hex(&row["eventsHex"])?;
        require(
            events.ends_with('\n')
                && events
                    .lines()
                    .filter(|l| l.starts_with("populated "))
                    .collect::<Vec<_>>()
                    == ["populated 0"],
            "populated/partial cgroup events",
        )?;
        if p == root {
            require(
                row["device"] == *device && row["inode"] == *inode,
                "root cgroup object mismatch",
            )?;
        }
    }
    let root = format!(
        "/sys/fs/cgroup{}",
        path.as_str().ok_or("cgroup path missing")?
    );
    require(
        paths.contains(root.as_str()),
        "original root cgroup row missing",
    )?;
    Ok(true)
}
pub fn teardown(files: &Files, b: &ExportBinding, normal_required: bool) -> Result<()> {
    let records = lines(files, "receipts/cleanup.evidence", 262144)?;
    let receipt = json_bytes(file(files, "teardown-receipt.json")?)?;
    require(
        receipt["schemaVersion"] == 1
            && receipt["containerId"] == b.container_id
            && receipt["nonce"] == b.nonce
            && receipt["bootId"] == b.boot_id
            && receipt["teardown"] == "CONTAINER_TEARDOWN_CONFIRMED"
            && receipt["directPrearm"] == false,
        "original CID teardown not confirmed",
    )?;
    let identity = json_bytes(file(files, "process-identity.json")?)?;
    let binding = json_bytes(file(files, "binding.json")?)?;
    let clock = json_bytes(file(files, "cleanup-begun.json")?)?;
    let armed = json_bytes(file(files, "armed.json")?)?;
    let first = clock["firstMs"]
        .as_u64()
        .ok_or("cleanup original clock missing")?;
    let original_end = clock["endMs"].as_u64().ok_or("cleanup deadline missing")?;
    let armed_ms = armed["armedMs"].as_u64().ok_or("arm clock missing")?;
    let outer = armed_ms
        .checked_add(600000)
        .ok_or("arm deadline overflow")?;
    require(
        clock["nonce"] == b.nonce
            && clock["containerId"] == b.container_id
            && clock["bootId"] == b.boot_id
            && first >= armed_ms
            && original_end
                == first
                    .checked_add(60000)
                    .ok_or("cleanup deadline overflow")?
                    .min(outer)
            && armed["finalDeadlineMs"].as_u64() == Some(outer),
        "original finite cleanup clock mismatch",
    )?;
    let expected_args = json!([
        "show",
        "--property=ExecMainStartTimestampMonotonic",
        format!("ktlsp-pr174-{}-cleanup.service", b.nonce)
    ]);
    let mut starts = records.iter().filter(|r| {
        r["kind"] == "command" && r["program"] == binding["systemctl"] && r["args"] == expected_args
    });
    let start = String::from_utf8(command_output(
        starts
            .next()
            .ok_or("actual cleanup service start measurement absent")?,
    )?)
    .map_err(|_| "cleanup start encoding")?;
    require(
        starts.next().is_none() && start.ends_with('\n') && start.lines().count() == 1,
        "cleanup service start duplicated/truncated",
    )?;
    let value = start
        .trim_end_matches('\n')
        .strip_prefix("ExecMainStartTimestampMonotonic=")
        .ok_or("cleanup service timestamp property absent")?;
    let usec = match value.parse::<u64>() {
        Ok(n) => n,
        Err(_) => authority::systemctl_usec(value)?,
    };
    let service_start = usec / 1000;
    require(
        service_start > 0 && service_start <= first,
        "cleanup service start not original monotonic instant",
    )?;
    let service_end = original_end.min(
        service_start
            .checked_add(60000)
            .ok_or("cleanup service deadline overflow")?,
    );
    let end = service_end
        .checked_sub(12000)
        .ok_or("cleanup export reserve underflow")?;
    let mut budgets = records
        .iter()
        .filter(|r| r["kind"] == "cleanup-export-budget");
    let budget = budgets
        .next()
        .ok_or("actual surviving export budget absent")?;
    require(
        budgets.next().is_none()
            && first <= end
            && budget["teardownEndMs"].as_u64() == Some(end)
            && budget["serviceEndMs"].as_u64() == Some(service_end)
            && budget["exportEndMs"].as_u64() == service_end.checked_sub(2000)
            && receipt["endMs"].as_u64() == Some(service_end),
        "actual cleanup service/export deadline mismatch",
    )?;
    let intent = json_bytes(file(files, "start-intent.json")?)?;
    let result = json_bytes(file(files, "start-result.json")?)?;
    require(
        intent["containerId"] == b.container_id
            && intent["nonce"] == b.nonce
            && intent["bootId"] == b.boot_id
            && result["containerId"] == b.container_id
            && result["complete"] == true,
        "original START not settled",
    )?;
    let mut stages = BTreeSet::new();
    let mut stopped = false;
    let mut waited = false;
    let mut removed = false;
    let mut listed = false;
    let mut typed_absence = false;
    let mut daemon_before = false;
    let mut daemon_after = false;
    let mut supervisor_inactive = false;
    let mut wait_exit = None;
    let mut forced_kill = false;
    for r in &records {
        if matches!(
            r["kind"].as_str(),
            Some("original-init-closure" | "original-cgroup-closure")
        ) {
            require(
                r["observedMs"]
                    .as_u64()
                    .is_some_and(|ms| first <= ms && ms <= end),
                "closure missing/outside original cleanup deadline",
            )?;
            match r["stage"].as_str() {
                Some("pre-removal" | "supervisor-stopped") => {
                    require(!removed, "pre-removal proof after removal")?
                }
                Some("post-removal") => require(removed, "post-removal proof before removal")?,
                _ => return Err("unknown original closure stage".into()),
            }
        }
        if r["kind"] == "original-init-closure" {
            let stage = r["stage"].as_str().ok_or("closure stage absent")?;
            require(
                r["hostPid"] == identity["initPid"]
                    && r["originalStarttime"] == identity["initStart"]
                    && r["originalGone"] == true,
                "original init closure mismatch",
            )?;
            let raw = text_hex(&r["statHex"])?;
            require(
                raw.is_empty()
                    || original_stat(&raw)?
                        != (
                            identity["initPid"].as_u64().ok_or("init PID absent")?,
                            identity["initStart"].as_u64().ok_or("init start absent")?,
                        ),
                "original init still present",
            )?;
            stages.insert(format!("init-{stage}"));
        }
        for stage in ["pre-removal", "post-removal"] {
            if cgroup_closure(
                r,
                stage,
                &identity["cgroup"],
                &identity["cgroupDevice"],
                &identity["cgroupInode"],
            )? {
                stages.insert(format!("cgroup-{stage}"));
            }
        }
        if cgroup_closure(
            r,
            "supervisor-stopped",
            &binding["supervisorCgroup"],
            &binding["supervisorCgroupDevice"],
            &binding["supervisorCgroupInode"],
        )? {
            stages.insert("supervisor-stopped".into());
        }
        if r["kind"] != "command" {
            continue;
        }
        let args = r["args"].as_array().ok_or("command args missing")?;
        if args
            .first()
            .is_some_and(|a| a == "--host=unix:///run/docker.sock")
        {
            require(
                r["program"] == binding["docker"],
                "cleanup command tool substitution",
            )?;
        }
        if r["program"] == binding["systemctl"]
            && r["args"]
                == json!([
                    "show",
                    "--property=ActiveState",
                    format!("ktlsp-pr174-{}-supervisor.service", b.nonce)
                ])
        {
            let raw =
                String::from_utf8(command_output(r)?).map_err(|_| "supervisor status encoding")?;
            supervisor_inactive = matches!(
                raw.as_str(),
                "ActiveState=inactive\n" | "ActiveState=failed\n"
            );
        }
        if normal_required {
            require(
                !docker_args(r, &["kill", "--signal=KILL", &b.container_id])
                    && !docker_args(r, &["rm", "--force", &b.container_id]),
                "forced original container termination/removal",
            )?;
        }
        if !normal_required && docker_args(r, &["kill", "--signal=KILL", &b.container_id]) {
            require(
                !forced_kill
                    && String::from_utf8(command_output(r)?)
                        .map_err(|_| "forced kill output encoding")?
                        .trim()
                        == b.container_id,
                "actual exact-CID calibration kill missing/duplicated",
            )?;
            forced_kill = true;
        }
        if docker_args(r, &["container", "inspect", &b.container_id]) {
            let output = command_output(r)?;
            let v = json_bytes(&output)?;
            let v = v
                .as_array()
                .filter(|v| v.len() == 1)
                .and_then(|v| v.first())
                .ok_or("inspect identity ambiguous")?;
            require(v["Id"] == b.container_id, "foreign inspected CID")?;
            if v["State"]["Running"] == false
                && v["State"]["Restarting"] == false
                && v["State"]["Dead"] == false
                && v["State"]["Pid"] == 0
            {
                require(
                    v["State"]["ExitCode"] == if normal_required { 0 } else { 137 }
                        && v["State"]["OOMKilled"] == false
                        && v["RestartCount"] == 0,
                    "original exit/OOM/restart mismatch",
                )?;
                stopped = waited && v["State"]["ExitCode"].as_i64() == wait_exit;
            }
        } else if docker_args(r, &["wait", &b.container_id]) {
            let raw = command_output(r)?;
            let code = std::str::from_utf8(&raw)
                .map_err(|_| "wait encoding")?
                .trim()
                .parse::<i32>()
                .map_err(|_| "invalid wait exit")?;
            require(
                if normal_required {
                    code == 0
                } else {
                    forced_kill && code == 137
                },
                "original normal/forced wait exit mismatch",
            )?;
            waited = true;
            wait_exit = Some(i64::from(code));
            stopped = false;
        } else if docker_args(r, &["rm", &b.container_id]) {
            require(
                stopped
                    && waited
                    && supervisor_inactive
                    && daemon_before
                    && stages.contains("supervisor-stopped")
                    && stages.contains("init-pre-removal")
                    && stages.contains("cgroup-pre-removal"),
                "removal before original stopped/closure/wait/supervisor/daemon proof",
            )?;
            require(
                String::from_utf8(command_output(r)?)
                    .map_err(|_| "rm encoding")?
                    .trim()
                    == b.container_id,
                "original ordinary rm not confirmed",
            )?;
            removed = true;
        } else if docker_args(
            r,
            &[
                "container",
                "ls",
                "--all",
                "--no-trunc",
                "--format",
                "{{json .ID}}",
            ],
        ) {
            let raw = command_output(r)?;
            require(
                removed && daemon_after && (raw.is_empty() || raw.ends_with(b"\n")),
                "absence before healthy post-rm daemon proof/partial list",
            )?;
            let mut ids = BTreeSet::new();
            for line in raw.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
                let id = json_bytes(line)?;
                let id = id.as_str().ok_or("invalid full container ID list")?;
                require(
                    authority::digest(id) && id != b.container_id && ids.insert(id.to_owned()),
                    "original CID present/duplicate/invalid list",
                )?;
            }
            listed = true;
        } else if args
            .iter()
            .any(|a| a == &format!("http://localhost/v1.47/containers/{}/json", b.container_id))
        {
            require(
                r["program"] == binding["curl"]
                    && r["args"]
                        == json!([
                            "--silent",
                            "--show-error",
                            "--max-time",
                            "10",
                            "--include",
                            "--unix-socket",
                            "/run/docker.sock",
                            "--noproxy",
                            "*",
                            format!("http://localhost/v1.47/containers/{}/json", b.container_id)
                        ]),
                "typed absence tool/argv mismatch",
            )?;
            let raw = String::from_utf8(command_output(r)?).map_err(|_| "404 encoding")?;
            let (headers, body) = raw
                .split_once("\r\n\r\n")
                .ok_or("missing typed404 headers")?;
            let message = json_bytes(body.as_bytes())?;
            require(
                removed
                    && headers
                        .lines()
                        .next()
                        .is_some_and(|l| l.starts_with("HTTP/1.1 404 "))
                    && message["message"] == format!("No such container: {}", b.container_id),
                "original typed404 absence mismatch",
            )?;
            typed_absence = true;
        } else if docker_args(r, &["info", "--format", "{{json .}}"]) {
            require(
                json_bytes(&command_output(r)?)?["ID"] == b.daemon_id,
                "original daemon substituted",
            )?;
            if removed {
                daemon_after = true;
            } else {
                daemon_before = true;
            }
        }
    }
    require(
        stopped
            && waited
            && removed
            && listed
            && typed_absence
            && daemon_before
            && daemon_after
            && supervisor_inactive
            && [
                "init-pre-removal",
                "init-post-removal",
                "cgroup-pre-removal",
                "cgroup-post-removal",
                "supervisor-stopped",
            ]
            .iter()
            .all(|s| stages.contains(*s)),
        "incomplete original daemon/init/cgroup/removal closure",
    )
}
fn calibration(
    files: &Files,
    expected: &RootGrant,
    supplied: &[(super::ReceiverAdmission, AuthenticatedArtifact)],
) -> Result<()> {
    let report: CalibrationReport = authority::decode(file(files, "calibration.json")?)?;
    report.validate(expected)?;
    require(
        supplied.len() == report.cases.len(),
        "missing authenticated original calibration artifacts/grants",
    )?;
    let mut seen = BTreeSet::new();
    for (admission, artifact) in supplied {
        let grant = &admission.grant;
        require(
            grant.scope == GrantScope::RustCalibration && seen.insert(grant.calibration_case),
            "duplicate/unscoped calibration grant",
        )?;
        let b = authenticated_provenance(artifact, admission)?;
        let case = report
            .cases
            .iter()
            .find(|c| Some(c.case) == grant.calibration_case)
            .ok_or("unreported calibration case")?;
        require(
            case.run_id == grant.run_id
                && case.job_id == grant.job_id
                && case.suite_id == grant.suite_id
                && case.attempt == grant.attempt
                && case.nonce == b.nonce
                && case.container_id == b.container_id
                && hash(file(&artifact.files, "receipts/observer.evidence")?)
                    == case.observation_sha256
                && hash(file(&artifact.files, "teardown-receipt.json")?) == case.teardown_sha256
                && hash(file(&artifact.files, "export-binding.json")?) == case.export_sha256,
            "calibration original job/raw/export mismatch",
        )?;
        require(
            grant.source_head == expected.source_head
                && grant.source_tree == expected.source_tree
                && grant.workflow_sha256 == expected.workflow_sha256
                && grant.host_binary_sha256 == expected.host_binary_sha256
                && grant.peer_sha256 == expected.peer_sha256
                && grant.containment_template_sha256 == expected.containment_template_sha256
                && grant.image == expected.image
                && grant.image_id == expected.image_id
                && grant.host == expected.host
                && b.boot_id == report.boot_id
                && b.daemon_id == report.daemon_id,
            "calibration same original host/image/config mismatch",
        )?;
        teardown(&artifact.files, &b, !case.case.forced())?;
        let observations = lines(&artifact.files, "receipts/observer.evidence", 786432)?;
        host_custody(&artifact.files, &b, grant, &observations)?;
        let peer_bytes = file(&artifact.files, "capture/calibration-peer.json")?;
        let peer: crate::calibration_peer::Report =
            serde_json::from_value(json_bytes(peer_bytes)?).map_err(|e| e.to_string())?;
        peer.validate_case(case.case)?;
        let ready: crate::calibration_peer::Report = serde_json::from_value(json_bytes(file(
            &artifact.files,
            "capture/calibration-ready.json",
        )?)?)
        .map_err(|e| e.to_string())?;
        require(
            ready.schema_version == 1
                && ready.scope == "PR174_RUST_CALIBRATION_PEER"
                && ready.case == case.case
                && ready.outcome == crate::calibration_peer::Outcome::AwaitingObserver
                && !ready.observer_gate_observed
                && ready.first_failure.is_none()
                && ready.original_process == peer.original_process
                && ready.children == peer.children,
            "original pre-gate peer identities/outcome mismatch",
        )?;
        let observed = observations
            .iter()
            .filter(|r| r["kind"] == "calibration-host-observation")
            .collect::<Vec<_>>();
        require(
            observed.len() == 1,
            "missing/ambiguous actual finite calibration host observation",
        )?;
        let host = observed[0];
        require(
            host["case"] == case.case.as_str()
                && host["containerId"] == b.container_id
                && host["nonce"] == b.nonce
                && host["bootId"] == b.boot_id
                && host["daemonId"] == b.daemon_id
                && host["run"] == serde_json::to_value(&b.run).map_err(|e| e.to_string())?
                && host["containmentTemplateSha256"] == grant.containment_template_sha256
                && host["normalExit"] == !case.case.forced()
                && host["peerReportSha256"]
                    == hash(&serde_json::to_vec(&peer).map_err(|e| e.to_string())?),
            "finite calibration actual host/raw peer binding mismatch",
        )?;
        let serialized = serde_json::to_value(&peer).map_err(|e| e.to_string())?;
        require(
            observations
                .iter()
                .any(|r| r["kind"] == "calibration-peer-observation" && r["report"] == serialized),
            "protected host did not retain actual peer report",
        )?;
        calibration_processes(&artifact.files, &b, &peer, &observations)?;
        if case.case.forced() {
            forced_calibration_action(
                &artifact.files,
                &b,
                case.case,
                &observations,
                artifact.timeout_completed_unix_ms,
            )?;
        }
    }
    Ok(())
}

fn calibration_processes(
    files: &Files,
    b: &ExportBinding,
    peer: &crate::calibration_peer::Report,
    observations: &[Value],
) -> Result<()> {
    let identity = json_bytes(file(files, "process-identity.json")?)?;
    require(
        identity["cgroup"] == format!("/system.slice/docker-{}.scope", b.container_id),
        "calibration original cgroup/CID mismatch",
    )?;
    let processes: Vec<_> = observations
        .iter()
        .filter(|r| r["kind"] == "calibration-original-process")
        .collect();
    require(
        processes.len() == 1 + peer.children.len(),
        "calibration original process inventory incomplete/ambiguous",
    )?;
    let reported = std::iter::once(&peer.original_process).chain(peer.children.iter());
    for (index, p) in reported.enumerate() {
        let mut matches = 0;
        for r in &processes {
            let status = text_hex(&r["statusHex"])?;
            let ids = status
                .lines()
                .find_map(|l| l.strip_prefix("NSpid:"))
                .ok_or("calibration NSpid absent")?
                .split_whitespace()
                .map(str::parse::<u64>)
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|_| "calibration invalid namespace PID")?;
            if ids.last().copied() != Some(p.pid as u64) {
                continue;
            }
            if index == 0 && peer.case == authority::CalibrationCase::BoundaryDenials {
                require(
                    r["procDirectoryUid"] == 0 && r["procDirectoryGid"] == 0,
                    "original denial driver proc directory lacks independent nondumpable custody",
                )?;
            }
            let stat = original_stat(&text_hex(&r["statHex"])?)?;
            require(
                ids.len() == 2
                    && ids[0] == stat.0
                    && r["hostPid"].as_u64() == Some(stat.0)
                    && stat.1 == p.start_time_ticks
                    && r["starttime"].as_u64() == Some(stat.1)
                    && r["child"] == (index != 0)
                    && r["case"] == peer.case.as_str()
                    && r["namespace"] == identity["pidNamespace"]
                    && text_hex(&r["cgroupHex"])?.trim()
                        == format!(
                            "0::{}",
                            identity["cgroup"]
                                .as_str()
                                .ok_or("calibration cgroup absent")?
                        ),
                "calibration original raw PID/start/ns/cgroup mismatch",
            )?;
            require(
                r["exeDevice"].as_u64() == Some(p.executable_device)
                    && r["exeInode"].as_u64() == Some(p.executable_inode)
                    && r["exeDevice"] == r["sealedExecutableDevice"]
                    && r["exeInode"] == r["sealedExecutableInode"],
                "calibration original sealed executable mismatch",
            )?;
            let expected = format!(
                "/tmp/ktlsp-t045-pr174-host/tool/ktlsp-oracle-calibration-peer\0{}\0{}\0",
                if index == 0 { "--case" } else { "--descendant" },
                peer.case.as_str()
            );
            require(
                unhex(&r["cmdlineHex"])? == expected.as_bytes(),
                "calibration actual peer argv mismatch",
            )?;
            for (name, expected) in [("NSpgid:", p.process_group), ("NSsid:", p.session)] {
                require(
                    status
                        .lines()
                        .find_map(|l| l.strip_prefix(name))
                        .and_then(|s| s.split_whitespace().last())
                        .and_then(|s| s.parse::<u32>().ok())
                        == Some(expected),
                    "calibration actual process-group/session mismatch",
                )?;
            }
            matches += 1;
        }
        require(
            matches == 1,
            "calibration original peer process missing/ambiguous",
        )?;
    }
    Ok(())
}

fn authenticated_provenance(
    a: &AuthenticatedArtifact,
    admission: &super::ReceiverAdmission,
) -> Result<ExportBinding> {
    admission.validate()?;
    require(
        a.admission_sha256 == hash(&serde_json::to_vec(admission).map_err(|e| e.to_string())?),
        "artifact authenticated for a different independent ROOT receiver/uploader admission",
    )?;
    provenance(&a.files, &admission.grant)
}

fn forced_calibration_action(
    files: &Files,
    b: &ExportBinding,
    case: authority::CalibrationCase,
    observations: &[Value],
    timeout_completed_ms: Option<u64>,
) -> Result<()> {
    let action: authority::CalibrationAction =
        authority::decode(file(files, "calibration-action.json")?)?;
    action.validate()?;
    let identity = json_bytes(file(files, "process-identity.json")?)?;
    let armed = json_bytes(file(files, "armed.json")?)?;
    let clock = json_bytes(file(files, "cleanup-begun.json")?)?;
    require(
        action.case == case
            && action.nonce == b.nonce
            && action.container_id == b.container_id
            && action.boot_id == b.boot_id
            && action.daemon_id == b.daemon_id
            && Some(action.init_pid as u64) == identity["initPid"].as_u64()
            && Some(action.init_starttime) == identity["initStart"].as_u64()
            && Some(action.armed_ms) == armed["armedMs"].as_u64()
            && clock["firstMs"]
                .as_u64()
                .is_some_and(|ms| ms >= action.observed_ms),
        "calibration actual action original binding/clock mismatch",
    )?;
    if case == authority::CalibrationCase::WorkflowStepCancel {
        require(
            timeout_completed_ms.is_some_and(|ms| action.observed_unix_ms >= ms),
            "outside-step supervisor survival not observed after authenticated original step cancellation",
        )?;
    }
    let serialized = serde_json::to_value(&action).map_err(|e| e.to_string())?;
    let mut records = observations
        .iter()
        .filter(|r| r["kind"] == "calibration-action");
    let raw = records
        .next()
        .ok_or("actual original calibration action observation absent")?;
    require(
        records.next().is_none() && raw["action"] == serialized,
        "calibration action missing/duplicated/substituted",
    )?;
    let stat = original_stat(&text_hex(&raw["originalStatHex"])?)?;
    let state = &raw["actualContainerState"];
    require(
        stat == (action.init_pid as u64, action.init_starttime)
            && state["Running"] == true
            && state["Restarting"] == false
            && state["Dead"] == false
            && state["OOMKilled"] == false
            && state["Pid"] == action.init_pid
            && raw["restartCount"] == 0,
        "calibration action did not observe original live init/noOOM/noRestart",
    )?;
    Ok(())
}
fn confinement_receipt(raw: Value) -> Result<Value> {
    let typed: crate::recorder::confinement::Receipt =
        serde_json::from_value(raw).map_err(|e| e.to_string())?;
    typed.validate_enforced(typed.parent_pid)?;
    require(
        typed.parent_pid > 1,
        "original writer namespace PID must follow original init",
    )?;
    serde_json::to_value(typed).map_err(|e| e.to_string())
}
fn host_custody(
    files: &Files,
    b: &ExportBinding,
    grant: &RootGrant,
    observations: &[Value],
) -> Result<()> {
    let measurement: authority::ConfinementMeasurement =
        authority::decode(file(files, "child-confinement-profile.json")?)?;
    measurement.validate(grant, &b.nonce, &b.boot_id, &b.daemon_id)?;
    let actual = serde_json::to_value(measurement).map_err(|e| e.to_string())?;
    let mut rows = observations
        .iter()
        .filter(|r| r["kind"] == "child-confinement-preflight");
    require(
        rows.next() == Some(&actual) && rows.next().is_none(),
        "independent original host profile/ABI observation missing/ambiguous",
    )?;
    Ok(())
}
fn writer_custody(
    files: &Files,
    b: &ExportBinding,
    grant: &RootGrant,
    observations: &[Value],
) -> Result<Value> {
    host_custody(files, b, grant, observations)?;
    let receipt = confinement_receipt(json_bytes(file(files, "capture/child-confinement.json")?)?)?;
    let manifest = json_bytes(file(files, "capture/manifest.json")?)?;
    let metadata = json_bytes(file(files, "capture/metadata.json")?)?;
    require(
        manifest["childConfinement"] == receipt && metadata["childConfinement"] == receipt,
        "original writer custody receipt/file/manifest/metadata mismatch",
    )?;
    Ok(receipt)
}

#[cfg(test)]
#[path = "receipt_tests.rs"]
mod receipt_tests;

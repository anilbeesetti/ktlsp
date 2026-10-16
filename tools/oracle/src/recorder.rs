//! Bounded raw stdio capture and PR174 input consistency validation.
use crate::{MAX_BYTES, MAX_MESSAGES, MAX_STDERR, envelope, id};
use ktlsp_protocol::{MAX_HEADER_BYTES, ProtocolError, read_frame, write_frame};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path};

#[path = "recorder/official.rs"]
mod official;
pub use official::record_pr174;
pub mod confinement;

enum CaptureContext {
    Synthetic,
    #[cfg(any(all(target_os = "linux", target_arch = "x86_64"), test))]
    Official {
        identity: Value,
        authorization: Value,
    },
}

impl CaptureContext {
    fn scope(&self) -> &'static str {
        match self {
            Self::Synthetic => "synthetic-only; official live execution blocked",
            #[cfg(any(all(target_os = "linux", target_arch = "x86_64"), test))]
            Self::Official { .. } => {
                "official-pr174-capture; runtime/containment/receiver unverified"
            }
        }
    }

    fn is_official(&self) -> bool {
        match self {
            Self::Synthetic => false,
            #[cfg(any(all(target_os = "linux", target_arch = "x86_64"), test))]
            Self::Official { .. } => true,
        }
    }

    fn provenance(&self) -> Value {
        match self {
            Self::Synthetic => json!({"archive":"none"}),
            #[cfg(any(all(target_os = "linux", target_arch = "x86_64"), test))]
            Self::Official { .. } => json!({
                "release":"kotlin-lsp/v263.6379.0",
                "archiveSha256":"ab8ca4455dc2fc5fe1a24db2bccc46c104254d2c465155c4251ee65df8f3f7cc"
            }),
        }
    }

    fn configure_command(&self, command: &mut std::process::Command) {
        let environment: &[(&str, &str)] = match self {
            Self::Synthetic => &[],
            #[cfg(any(all(target_os = "linux", target_arch = "x86_64"), test))]
            Self::Official { .. } => &[
                ("PATH", "/usr/bin:/bin"),
                ("LANG", "C.UTF-8"),
                ("LC_ALL", "C.UTF-8"),
            ],
        };
        command.envs(environment.iter().copied());
    }

    fn initial_manifest(&self, limits: CaptureLimits) -> Value {
        let report = json!({
            "schemaVersion":1,
            "scope":self.scope(),
            "complete":false,
            "failureReason":"capture interrupted before final manifest",
            "limits":limits,
            "officialAcceptance":false,
            "runtime":"unverified",
            "containment":"unverified",
            "receiver":"unverified",
            "provenance":self.provenance(),
            "export":{"complete":false,"records":0,"bytes":0,"failureReason":"export not completed"},
            "cleanup":{"containment":"isolated process group; escaped sessions/groups unsupported"}
        });
        match self {
            Self::Synthetic => report,
            #[cfg(any(all(target_os = "linux", target_arch = "x86_64"), test))]
            Self::Official {
                identity,
                authorization,
            } => {
                let mut report = report;
                report["identity"] = identity.clone();
                report["authorization"] = authorization.clone();
                report["environment"] = json!({
                    "PATH":"/usr/bin:/bin","LANG":"C.UTF-8","LC_ALL":"C.UTF-8"
                });
                report["childConfinement"] = json!({
                    "schemaVersion":1,"profile":confinement::profile(),
                    "landlockAbi":null,"parentPid":std::process::id(),
                    "parentDumpable":null,"hookOutcome":"NOT_RUN"
                });
                report
            }
        }
    }

    fn metadata(&self, returncode: i32) -> Value {
        let (release, sha256) = match self {
            Self::Synthetic => (
                "synthetic-only",
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
            #[cfg(any(all(target_os = "linux", target_arch = "x86_64"), test))]
            Self::Official { .. } => (
                "kotlin-lsp/v263.6379.0",
                "ab8ca4455dc2fc5fe1a24db2bccc46c104254d2c465155c4251ee65df8f3f7cc",
            ),
        };
        let metadata = json!({
            "release":release,"sha256":sha256,"returncode":returncode,
            "scope":self.scope(),"officialAcceptance":false,
            "provenance":self.provenance(),"runtime":"unverified",
            "containment":"unverified","receiver":"unverified"
        });
        match self {
            Self::Synthetic => metadata,
            #[cfg(any(all(target_os = "linux", target_arch = "x86_64"), test))]
            Self::Official {
                identity,
                authorization,
            } => {
                let mut metadata = metadata;
                metadata["identity"] = identity.clone();
                metadata["authorization"] = authorization.clone();
                metadata
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RawReference {
    direction: &'static str,
    frame_index: usize,
}

#[derive(Serialize)]
struct DecodedFrame {
    start: usize,
    end: usize,
    message: Value,
}

struct Transcript<'a> {
    chronology: &'a [RawReference],
    script: &'a [Value],
    decoded: &'a [DecodedFrame],
}

impl Serialize for Transcript<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        #[derive(Serialize)]
        struct Record<'a> {
            direction: &'static str,
            message: &'a Value,
        }
        let mut sequence = serializer.serialize_seq(Some(self.chronology.len()))?;
        for reference in self.chronology {
            let message = match reference.direction {
                "client" => &self.script[reference.frame_index],
                _ => &self.decoded[reference.frame_index].message,
            };
            sequence.serialize_element(&Record {
                direction: reference.direction,
                message,
            })?;
        }
        sequence.end()
    }
}

struct ExportWriter<W> {
    inner: W,
    bytes: usize,
    overflow: bool,
}

impl<W: std::io::Write> std::io::Write for ExportWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_BYTES - self.bytes {
            self.overflow = true;
            return Err(std::io::Error::other("transcript byte limit"));
        }
        let written = self.inner.write(bytes)?;
        self.bytes += written;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn write_transcript(
    output: &mut impl std::io::Write,
    transcript: &Transcript<'_>,
) -> Result<usize, String> {
    if transcript.chronology.len() > MAX_MESSAGES {
        return Err("transcript record limit".into());
    }
    let mut output = ExportWriter {
        inner: output,
        bytes: 0,
        overflow: false,
    };
    let result = serde_json::to_writer(&mut output, transcript);
    if output.overflow {
        return Err("transcript byte limit".into());
    }
    result.map_err(|e| format!("transcript export: {e}"))?;
    std::io::Write::flush(&mut output).map_err(|e| format!("transcript export: {e}"))?;
    Ok(output.bytes)
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaptureLimits {
    pub phase_ms: u64,
    pub session_ms: u64,
    pub cleanup_ms: u64,
    pub stream_bytes: usize,
    pub stderr_bytes: usize,
    pub messages: usize,
}
impl Default for CaptureLimits {
    fn default() -> Self {
        Self {
            phase_ms: 5_000,
            session_ms: 30_000,
            cleanup_ms: 1_000,
            stream_bytes: MAX_BYTES,
            stderr_bytes: MAX_STDERR,
            messages: MAX_MESSAGES,
        }
    }
}

/// Validate proposed PR174 inputs only; success grants no launch authority.
/// No output is created and no process is spawned.
pub fn validate_pr174_inputs(
    identity_path: &Path,
    preflight_path: &Path,
    fixture_directory: &Path,
) -> Result<(), String> {
    let identity: Pr174Identity =
        serde_json::from_slice(&preparation_input(identity_path, 16_384)?)
            .map_err(|e| format!("frozen identity: {e}"))?;
    identity.validate()?;
    let preflight: Pr174Preflight =
        serde_json::from_slice(&preparation_input(preflight_path, 16_384)?)
            .map_err(|e| format!("preflight: {e}"))?;
    if preflight.schema_version != 1 || preflight.identity != identity {
        return Err("preflight frozen identity mismatch".into());
    }
    let expected_pins = json!({
        "release": "kotlin-lsp/v263.6379.0",
        "platform": "linux-amd64",
        "archiveSha256": "ab8ca4455dc2fc5fe1a24db2bccc46c104254d2c465155c4251ee65df8f3f7cc",
        "launcherSha256": "adf02baec622b4fcd1e7db3ec96f7c7b3309d5ab59e170e60260d7511222878e",
        "scriptSha256": "930a87ea1fa9c3f6e6a6b6008d010dd9435082329ea8b897a42084d7369ab008",
        "declarationsSha256": "fa935b144a49838fd9e603bfc95462bcf3701eeb19877501d451ca069ce9e233",
        "errorNodeSha256": "c052b78e7603a88f3531f5b06f3a533f8df5394ce8ce9eeab3eb2b3ce3f4bd2c"
    });
    let expected_command = json!([
        "/tmp/ktlsp-t045-pr174-host/distribution/kotlin-server-263.6379.0/bin/intellij-server",
        "--stdio",
        "--data-sharing=none",
        "--region=apac",
        "--system-path=/tmp/ktlsp-t045-pr174-host/capture/system"
    ]);
    let limits = CaptureLimits {
        phase_ms: 60_000,
        session_ms: 300_000,
        cleanup_ms: 5_000,
        ..CaptureLimits::default()
    };
    if serde_json::to_value(preflight.pins).map_err(|e| e.to_string())? != expected_pins
        || preflight.command != expected_command
        || serde_json::to_value(preflight.limits).map_err(|e| e.to_string())?
            != serde_json::to_value(limits).map_err(|e| e.to_string())?
        || preflight.output_directory != "/tmp/ktlsp-t045-pr174-host/capture"
    {
        return Err("preflight PR174 pins, command, limits or output mismatch".into());
    }
    for (name, expected) in [
        (
            "declarations.kt",
            include_bytes!("../fixtures/official-pr174-symbols/declarations.kt").as_slice(),
        ),
        (
            "error-node.kt",
            include_bytes!("../fixtures/official-pr174-symbols/error-node.kt").as_slice(),
        ),
        (
            "script.json",
            include_bytes!("../fixtures/official-pr174-symbols/script.json").as_slice(),
        ),
    ] {
        if preparation_input(&fixture_directory.join(name), expected.len())? != expected {
            return Err(format!("PR174 fixture bytes mismatch: {name}"));
        }
    }
    // Hash claims are not authenticated until the separately admitted supervisor handoff exists.
    Ok(())
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Pr174Identity {
    schema_version: u8,
    contract_sha256: String,
    source_baseline: String,
    original_run: Pr174Run,
}
#[derive(Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Pr174Run {
    repository: String,
    event: String,
    attempt: u8,
    run_id: u64,
    job_id: u64,
    suite_id: u64,
    source_head: String,
    source_tree: String,
    workflow_sha256: String,
    container_id: String,
    image_sha256: String,
    nonce: String,
}
impl Pr174Identity {
    fn validate(&self) -> Result<(), String> {
        let run = &self.original_run;
        let hex = |s: &str, length| {
            s.len() == length
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        if self.schema_version != 1
            || self.contract_sha256
                != "65eb16c5bd73c99fe7fc000fddf0242bf6abab09283676612edf5ce9f285b61d"
            || self.source_baseline != "b4fa25286a5fa0d7600b0b0420a7bcae6d520edc"
            || run.repository != "anilbeesetti/ktlsp"
            || run.event != "workflow_dispatch"
            || run.attempt != 1
            || run.run_id == 0
            || run.job_id == 0
            || run.suite_id == 0
            || !hex(&run.source_head, 40)
            || !hex(&run.source_tree, 40)
            || !hex(&run.workflow_sha256, 64)
            || !hex(&run.container_id, 64)
            || !hex(&run.image_sha256, 64)
            || !hex(&run.nonce, 64)
        {
            return Err("frozen PR174 identity mismatch".into());
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Pr174Preflight {
    schema_version: u8,
    identity: Pr174Identity,
    pins: Pr174Pins,
    command: Value,
    limits: CaptureLimits,
    output_directory: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Pr174Pins {
    release: String,
    platform: String,
    archive_sha256: String,
    launcher_sha256: String,
    script_sha256: String,
    declarations_sha256: String,
    error_node_sha256: String,
}

fn preparation_input(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    use std::{fs, io::Read};
    if !fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("preparation input must be a regular file".into());
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        #[cfg(target_os = "linux")]
        const FLAGS: i32 = 0x800 | 0x20000; // O_NONBLOCK | O_NOFOLLOW
        #[cfg(not(target_os = "linux"))]
        const FLAGS: i32 = 4 | 0x100;
        options.custom_flags(FLAGS);
    }
    let input = options.open(path).map_err(|e| e.to_string())?;
    if !input.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("preparation input must be a regular file".into());
    }
    let mut bytes = Vec::new();
    input
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("preparation input byte limit".into());
    }
    Ok(bytes)
}

/// Fresh local evidence directory; command must be an absolute executable path.
/// Errors after launch are saved in manifest.json and returned as a partial report.
pub fn record(
    script: &[Value],
    command: &[String],
    directory: &Path,
    limits: CaptureLimits,
) -> Result<Value, String> {
    record_with_context(
        script,
        command,
        directory,
        limits,
        &CaptureContext::Synthetic,
    )
}

fn record_with_context(
    script: &[Value],
    command: &[String],
    directory: &Path,
    limits: CaptureLimits,
    context: &CaptureContext,
) -> Result<Value, String> {
    if command.is_empty() || !Path::new(&command[0]).is_absolute() {
        return Err("absolute executable path required".into());
    }
    if limits.phase_ms == 0
        || limits.session_ms == 0
        || limits.cleanup_ms == 0
        || limits.phase_ms > 300_000
        || limits.session_ms > 300_000
        || limits.cleanup_ms > 5_000
        || limits.stream_bytes == 0
        || limits.stream_bytes > MAX_BYTES
        || limits.stderr_bytes == 0
        || limits.stderr_bytes > MAX_STDERR
        || limits.messages == 0
        || limits.messages > MAX_MESSAGES
    {
        return Err("invalid capture limits".into());
    }
    if script.is_empty() || script.len() > limits.messages {
        return Err("invalid script count".into());
    }
    let mut seen = BTreeSet::new();
    let mut frames = Vec::new();
    let mut total = 0;
    for (index, message) in script.iter().enumerate() {
        envelope(message)?;
        let key = id(message)?;
        if message.get("method").is_none()
            || (message["method"] == "exit" && index + 1 != script.len())
        {
            return Err("script requires client methods and final exit only".into());
        }
        if let Some(key) = key
            && !seen.insert(key)
        {
            return Err("duplicate script ID".into());
        }
        let mut frame = Vec::new();
        write_frame(
            &mut frame,
            &serde_json::to_vec(message).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        total += frame.len();
        if total > limits.stream_bytes {
            return Err("script exceeds byte limit".into());
        }
        frames.push(frame);
    }
    if script.last().unwrap()["method"] != "exit" || id(script.last().unwrap())?.is_some() {
        return Err("script must end with exit notification".into());
    }
    platform::run(script, &frames, command, directory, limits, context)
}

// Completeness scheduling only; read_frame remains the authoritative grammar validator.
fn frame_size(bytes: &[u8]) -> Result<Option<usize>, String> {
    let Some(end) = bytes
        .windows(4)
        .position(|b| b == b"\r\n\r\n")
        .map(|p| p + 4)
    else {
        return if bytes.len() >= MAX_HEADER_BYTES {
            Err("frame header limit".into())
        } else {
            Ok(None)
        };
    };
    if end > MAX_HEADER_BYTES {
        return Err("frame header limit".into());
    }
    // Validate once per header (caller caches size); a truncated payload means valid headers.
    match read_frame(&mut std::io::Cursor::new(&bytes[..end])) {
        Ok(_) | Err(ProtocolError::TruncatedPayload) => (),
        Err(error) => return Err(error.to_string()),
    }
    let header = std::str::from_utf8(&bytes[..end]).map_err(|e| e.to_string())?;
    let length = header
        .split("\r\n")
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim_matches([' ', '\t']).parse::<usize>().ok())
        .ok_or("invalid length")?;
    Ok(Some(end + length))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod platform {
    use super::*;
    pub fn run(
        _: &[Value],
        _: &[Vec<u8>],
        _: &[String],
        _: &Path,
        _: CaptureLimits,
        _: &CaptureContext,
    ) -> Result<Value, String> {
        Err("recorder supervision supports Linux/macOS only; no process launched".into())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod platform {
    use super::*;
    use std::{
        fs::{self, File, OpenOptions},
        io::{self, Read, Write},
        os::{
            fd::AsRawFd,
            unix::{
                fs::{OpenOptionsExt, PermissionsExt},
                process::{CommandExt, ExitStatusExt},
            },
        },
        process::{Child, Command, Stdio},
        thread,
        time::{Duration, Instant},
    };

    unsafe extern "C" {
        fn fcntl(fd: i32, command: i32, ...) -> i32;
        fn kill(pid: i32, signal: i32) -> i32;
    }
    fn nonblocking(pipe: &impl AsRawFd) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        const NONBLOCK: i32 = 0x800;
        #[cfg(target_os = "macos")]
        const NONBLOCK: i32 = 4;
        // SAFETY: owned pipe descriptor, F_GETFL/F_SETFL with platform O_NONBLOCK.
        let flags = unsafe { fcntl(pipe.as_raw_fd(), 3) };
        if flags < 0 || unsafe { fcntl(pipe.as_raw_fd(), 4, flags | NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    struct Group {
        child: Child,
        pgid: i32,
        killed: bool,
    }
    impl Group {
        fn terminate(&mut self) -> io::Result<()> {
            if self.killed {
                return Ok(());
            }
            // SAFETY: positive group id assigned to our child by process_group(0).
            let result = unsafe { kill(-self.pgid, 9) };
            if result < 0 && io::Error::last_os_error().raw_os_error() != Some(3) {
                return Err(io::Error::last_os_error());
            }
            self.killed = true;
            Ok(())
        }
    }
    impl Drop for Group {
        fn drop(&mut self) {
            let _ = self.terminate();
            let _ = self.child.try_wait();
        }
    }
    fn file(path: &Path) -> io::Result<File> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
    }
    fn save_confinement(directory: &Path, receipt: &Value) -> io::Result<()> {
        let mut output = file(&directory.join("child-confinement.json"))?;
        serde_json::to_writer(&mut output, receipt)?;
        output.flush()
    }
    fn save(directory: &Path, report: &Value) -> io::Result<()> {
        let temporary = directory.join("manifest.next");
        let mut output = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temporary)?;
        serde_json::to_writer_pretty(&mut output, report)?;
        output.flush()?;
        fs::rename(temporary, directory.join("manifest.json"))
    }
    fn persist(file: &mut File, bytes: &[u8]) -> (usize, Option<io::Error>) {
        let mut persisted = 0;
        while persisted < bytes.len() {
            match file.write(&bytes[persisted..]) {
                Ok(0) => {
                    return (
                        persisted,
                        Some(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "evidence write zero",
                        )),
                    );
                }
                Ok(n) => persisted += n,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => (),
                Err(error) => return (persisted, Some(error)),
            }
        }
        (persisted, None)
    }
    struct Stream {
        file: File,
        bytes: Vec<u8>,
        eof: bool,
        truncated: bool,
        observed_bytes: usize,
        unpersisted_bytes: usize,
        storage_failed: bool,
    }
    impl Stream {
        fn read(&mut self, pipe: &mut impl Read, cap: usize) -> Result<(), String> {
            if self.eof || self.truncated || self.storage_failed {
                return Ok(());
            }
            let mut buffer = [0; 8192];
            // One bounded read per tick preserves fairness across all three pipes.
            let size = buffer.len().min(cap.saturating_sub(self.bytes.len()) + 1);
            match pipe.read(&mut buffer[..size]) {
                Ok(0) => self.eof = true,
                Ok(n) => {
                    let retained = n.min(cap.saturating_sub(self.bytes.len()));
                    self.observed_bytes += n;
                    let (persisted, error) = persist(&mut self.file, &buffer[..n]);
                    self.bytes.extend_from_slice(&buffer[..persisted]);
                    self.unpersisted_bytes += n - persisted;
                    if let Some(error) = error {
                        self.storage_failed = true;
                        return Err(format!("evidence write: {error}"));
                    }
                    if retained != n {
                        self.truncated = true;
                        return Err("stream byte limit (extra byte observed)".into());
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(e) => return Err(format!("pipe read: {e}")),
            }
            Ok(())
        }
    }
    pub fn run(
        script: &[Value],
        frames: &[Vec<u8>],
        command: &[String],
        directory: &Path,
        limits: CaptureLimits,
        context: &CaptureContext,
    ) -> Result<Value, String> {
        fs::create_dir(directory).map_err(|e| format!("fresh evidence directory: {e}"))?;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        let directory = fs::canonicalize(directory).map_err(|e| e.to_string())?;
        let mut report = context.initial_manifest(limits);
        save(&directory, &report).map_err(|e| e.to_string())?;
        let result = capture(
            script,
            frames,
            command,
            &directory,
            limits,
            context,
            &mut report,
        );
        if let Err(error) = result {
            report["complete"] = json!(false);
            report["failureReason"] = json!(error);
        }
        save(&directory, &report).map_err(|e| {
            format!("final manifest failed; initial incomplete manifest retained: {e}")
        })?;
        Ok(report)
    }
    fn capture(
        script: &[Value],
        frames: &[Vec<u8>],
        command: &[String],
        directory: &Path,
        limits: CaptureLimits,
        context: &CaptureContext,
        report: &mut Value,
    ) -> Result<(), String> {
        let mut intended =
            file(&directory.join("client-intended.raw")).map_err(|e| e.to_string())?;
        let mut stdout = Stream {
            file: file(&directory.join("stdout.raw")).map_err(|e| e.to_string())?,
            bytes: Vec::new(),
            eof: false,
            truncated: false,
            observed_bytes: 0,
            unpersisted_bytes: 0,
            storage_failed: false,
        };
        let mut stderr = Stream {
            file: file(&directory.join("stderr.raw")).map_err(|e| e.to_string())?,
            bytes: Vec::new(),
            eof: false,
            truncated: false,
            observed_bytes: 0,
            unpersisted_bytes: 0,
            storage_failed: false,
        };
        for name in ["home", "config", "system", "log", "tmp", "workspace"] {
            fs::create_dir(directory.join(name)).map_err(|e| e.to_string())?;
            if context.is_official() {
                fs::set_permissions(directory.join(name), fs::Permissions::from_mode(0o700))
                    .map_err(|e| e.to_string())?;
            }
        }
        let mut child_command = Command::new(&command[0]);
        child_command
            .args(&command[1..])
            .env_clear()
            .env("HOME", directory.join("home"))
            .env("XDG_CONFIG_HOME", directory.join("config"))
            .env("XDG_CACHE_HOME", directory.join("system"))
            .env("XDG_STATE_HOME", directory.join("log"))
            .env("TMPDIR", directory.join("tmp"))
            .env("TMP", directory.join("tmp"))
            .env("TEMP", directory.join("tmp"))
            .current_dir(directory.join("workspace"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        context.configure_command(&mut child_command);
        let (child, confinement_receipt) = if context.is_official() {
            match confinement::spawn(&mut child_command, directory) {
                Ok((child, receipt)) => (child, Some(receipt)),
                Err(failure) => {
                    report["childConfinement"] =
                        serde_json::to_value(failure.receipt).map_err(|e| e.to_string())?;
                    if let Err(storage) = save_confinement(directory, &report["childConfinement"]) {
                        return Err(format!(
                            "{}; confinement receipt storage: {storage}",
                            failure.error
                        ));
                    }
                    return Err(failure.error);
                }
            }
        } else {
            (
                child_command.spawn().map_err(|e| format!("spawn: {e}"))?,
                None,
            )
        };
        let pgid = i32::try_from(child.id()).map_err(|e| e.to_string())?;
        let mut group = Group {
            child,
            pgid,
            killed: false,
        };
        if let Some(receipt) = confinement_receipt {
            report["childConfinement"] =
                serde_json::to_value(receipt).map_err(|e| e.to_string())?;
            save_confinement(directory, &report["childConfinement"]).map_err(|e| e.to_string())?;
        }
        let mut input = group.child.stdin.take();
        let mut output = group.child.stdout.take().unwrap();
        let mut errors = group.child.stderr.take().unwrap();
        let setup_error = nonblocking(input.as_ref().unwrap())
            .and_then(|()| nonblocking(&output))
            .and_then(|()| nonblocking(&errors))
            .err()
            .map(|e| e.to_string());
        let started = Instant::now();
        let session = started + Duration::from_millis(limits.session_ms);
        let mut deadline = session.min(started + Duration::from_millis(limits.phase_ms));
        let mut cleanup_deadline = None;
        let pipes_ready = setup_error.is_none();
        let mut failure = setup_error;
        let mut cleanup_error = None;
        let mut status = None;
        let mut index = 0;
        let mut written = 0;
        let mut outgoing = Vec::<Value>::new();
        let mut decoded = Vec::<DecodedFrame>::new();
        let mut chronology = Vec::<RawReference>::new();
        let mut consumed = 0;
        let mut next_size = None;
        let mut pending = None;
        let mut script_done = false;
        let mut forced = false;
        let mut before_cleanup = Value::Null;
        loop {
            if failure.is_none() && Instant::now() >= deadline {
                failure = Some("phase/session deadline exceeded".into());
            }
            let tick: Result<(), String> = (|| {
                if status.is_none() {
                    status = group.child.try_wait().map_err(|e| e.to_string())?;
                }
                if !pipes_ready {
                    return Ok(());
                }
                stdout
                    .read(&mut output, limits.stream_bytes)
                    .map_err(|e| format!("stdout: {e}"))?;
                stderr
                    .read(&mut errors, limits.stderr_bytes)
                    .map_err(|e| format!("stderr: {e}"))?;
                if failure.is_some() {
                    return Ok(());
                }
                // At most 32 frames decoded per tick, with raw bytes persisted first.
                let mut decoded_this_tick = 0;
                for _ in 0..32 {
                    if consumed == stdout.bytes.len() {
                        break;
                    }
                    if next_size.is_none() {
                        next_size = frame_size(&stdout.bytes[consumed..])?;
                    }
                    let Some(size) = next_size else {
                        break;
                    };
                    if size > limits.stream_bytes {
                        return Err("frame exceeds capture byte limit".into());
                    }
                    if stdout.bytes.len() - consumed < size {
                        break;
                    }
                    let start = consumed;
                    let payload = read_frame(&mut std::io::Cursor::new(
                        &stdout.bytes[start..start + size],
                    ))
                    .map_err(|e| e.to_string())?
                    .unwrap();
                    next_size = None;
                    if decoded.len() == limits.messages {
                        return Err("stdout message limit".into());
                    }
                    let message: Value = serde_json::from_slice(&payload)
                        .map_err(|e| format!("stdout JSON: {e}"))?;
                    consumed += size;
                    decoded_this_tick += 1;
                    decoded.push(DecodedFrame {
                        start,
                        end: consumed,
                        message,
                    });
                    chronology.push(RawReference {
                        direction: "server",
                        frame_index: decoded.len() - 1,
                    });
                    let message = &decoded.last().unwrap().message;
                    envelope(message)?;
                    let key = id(message)?;
                    if message.get("method").is_some() {
                        if key.is_some() {
                            return Err("unexpected server request".into());
                        }
                    } else if key.is_none() || key != pending {
                        return Err("unexpected/duplicate response ID".into());
                    } else if index >= frames.len() || written != frames[index].len() {
                        return Err("response before client frame fully accepted".into());
                    } else {
                        pending = None;
                    }
                }
                if stdout.eof && consumed != stdout.bytes.len() && decoded_this_tick < 32 {
                    return Err("EOF with partial/undecoded stdout frame".into());
                }
                if stdout.eof && pending.is_some() && consumed == stdout.bytes.len() {
                    return Err("stdout EOF before response".into());
                }
                if index < frames.len() && outgoing.len() == index {
                    if pending.is_some() {
                        return Ok(());
                    }
                    let start = outgoing.last().map_or(0, |v| v["end"].as_u64().unwrap());
                    let (persisted, error) = persist(&mut intended, &frames[index]);
                    outgoing.push(json!({"start":start,"end":start + persisted as u64,"intendedLength":frames[index].len(),"persistedBytes":persisted,"acceptedBytes":0}));
                    if let Some(error) = error {
                        return Err(format!("intended evidence write: {error}"));
                    }
                    pending = id(&script[index])?;
                    if Instant::now() >= deadline {
                        return Err("phase/session deadline exceeded".into());
                    }
                    deadline = session.min(Instant::now() + Duration::from_millis(limits.phase_ms));
                }
                if index < frames.len() && written < frames[index].len() {
                    let end = frames[index].len().min(written + 8192);
                    match input.as_mut().unwrap().write(&frames[index][written..end]) {
                        Ok(0) => return Err("stdin write zero".into()),
                        Ok(n) => {
                            written += n;
                            outgoing[index]["acceptedBytes"] = json!(written);
                            if written == frames[index].len() {
                                chronology.push(RawReference {
                                    direction: "client",
                                    frame_index: index,
                                });
                            }
                        }
                        Err(e)
                            if matches!(
                                e.kind(),
                                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                            ) => {}
                        Err(e) => return Err(format!("stdin write: {e}")),
                    }
                }
                if index < frames.len() && written == frames[index].len() && pending.is_none() {
                    index += 1;
                    written = 0;
                    if Instant::now() >= deadline {
                        return Err("phase/session deadline exceeded".into());
                    }
                    deadline = session.min(Instant::now() + Duration::from_millis(limits.phase_ms));
                    if index == frames.len() {
                        script_done = true;
                        input.take();
                    }
                }
                Ok(())
            })();
            if failure.is_none() {
                failure = tick.err();
            }
            if failure.is_none() && Instant::now() >= deadline {
                failure = Some("phase/session deadline exceeded".into());
            }
            let normal_done = script_done
                && status.is_some()
                && stdout.eof
                && stderr.eof
                && consumed == stdout.bytes.len();
            if failure.is_none() && !normal_done && Instant::now() >= deadline {
                failure = Some("phase/session deadline exceeded".into());
            }
            if cleanup_deadline.is_none() && (failure.is_some() || normal_done) {
                before_cleanup = json!({"childExitObserved":status.is_some(),"stdoutEof":stdout.eof,"stderrEof":stderr.eof});
                forced = !normal_done;
                input.take();
                cleanup_error = group.terminate().err().map(|e| e.to_string());
                cleanup_deadline = Some(Instant::now() + Duration::from_millis(limits.cleanup_ms));
            }
            if let Some(end) = cleanup_deadline {
                if status.is_some() && stdout.eof && stderr.eof {
                    break;
                }
                if Instant::now() >= end {
                    cleanup_error
                        .get_or_insert("cleanup deadline; child or pipe EOF unconfirmed".into());
                    break;
                }
            }
            thread::sleep(Duration::from_millis(2));
        }
        let transcript = Transcript {
            chronology: &chronology,
            script,
            decoded: &decoded,
        };
        let transport_complete = failure.is_none() && cleanup_error.is_none() && script_done;
        let exported = if transport_complete {
            export(
                directory,
                &transcript,
                context,
                status.as_ref(),
                report.get("childConfinement"),
            )
        } else {
            Err(format!(
                "consumer export withheld: {}",
                failure
                    .as_ref()
                    .or(cleanup_error.as_ref())
                    .map_or("script incomplete", String::as_str)
            ))
        };
        report["export"] = match &exported {
            Ok(bytes) => json!({
                "complete":true,"records":chronology.len(),"bytes":bytes,"failureReason":null
            }),
            Err(error) => json!({
                "complete":false,"records":chronology.len(),"bytes":0,"failureReason":error
            }),
        };
        if context.is_official()
            && let Err(error) = exported
        {
            failure.get_or_insert(error);
        }
        let complete = failure.is_none() && cleanup_error.is_none() && script_done;
        report["complete"] = json!(complete);
        report["failureReason"] = json!(failure.as_ref().or(cleanup_error.as_ref()));
        report["scriptCompleted"] = json!(script_done);
        report["clientFrames"] = Value::Array(outgoing);
        report["stdoutFrames"] = Value::Array(
            decoded
                .into_iter()
                .map(|frame| {
                    Value::Object(serde_json::Map::from_iter([
                        ("start".into(), Value::from(frame.start)),
                        ("end".into(), Value::from(frame.end)),
                        ("message".into(), frame.message),
                    ]))
                })
                .collect(),
        );
        report["chronology"] = serde_json::to_value(chronology).map_err(|e| e.to_string())?;
        report["stdout"] = json!({"bytes":stdout.bytes.len(),"observedBytes":stdout.observed_bytes,"unpersistedBytes":stdout.unpersisted_bytes,"storageFailed":stdout.storage_failed,"eof":stdout.eof,"truncated":stdout.truncated,"undecodedStart":consumed,"undecodedEnd":stdout.bytes.len()});
        report["stderr"] = json!({"bytes":stderr.bytes.len(),"observedBytes":stderr.observed_bytes,"unpersistedBytes":stderr.unpersisted_bytes,"storageFailed":stderr.storage_failed,"eof":stderr.eof,"truncated":stderr.truncated});
        report["exit"] = json!({"observed":status.is_some(),"code":status.and_then(|s| s.code()),"signal":status.and_then(|s| s.signal())});
        report["cleanup"]["before"] = before_cleanup;
        report["cleanup"]["forcedTermination"] = json!(forced);
        report["cleanup"]["groupKillAttempted"] = json!(group.killed);
        report["cleanup"]["error"] = json!(cleanup_error);
        report["elapsedMs"] = json!(started.elapsed().as_millis() as u64);
        Ok(())
    }

    fn export(
        directory: &Path,
        transcript: &Transcript<'_>,
        context: &CaptureContext,
        status: Option<&std::process::ExitStatus>,
        confinement_receipt: Option<&Value>,
    ) -> Result<usize, String> {
        let returncode = status
            .and_then(|status| {
                status
                    .code()
                    .or_else(|| status.signal().map(|signal| -signal))
            })
            .ok_or("export requires observed child returncode")?;
        let mut evidence_metadata = context.metadata(returncode);
        if context.is_official() {
            let receipt =
                confinement_receipt.ok_or("official export requires child confinement")?;
            let typed = confinement::Receipt::deserialize(receipt).map_err(|e| e.to_string())?;
            typed.validate_enforced(std::process::id())?;
            evidence_metadata["childConfinement"] = receipt.clone();
        }
        let temporary = directory.join("transcript.next");
        let result = (|| {
            let mut output = file(&temporary).map_err(|e| format!("transcript export: {e}"))?;
            let bytes = write_transcript(&mut output, transcript)?;
            let mut metadata = file(&directory.join("metadata.json"))
                .map_err(|e| format!("metadata export: {e}"))?;
            serde_json::to_writer(&mut metadata, &evidence_metadata)
                .map_err(|e| format!("metadata export: {e}"))?;
            metadata
                .flush()
                .map_err(|e| format!("metadata export: {e}"))?;
            fs::rename(&temporary, directory.join("transcript.json"))
                .map_err(|e| format!("transcript export: {e}"))?;
            Ok(bytes)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    #[test]
    fn official_export_refuses_missing_unenforced_or_mismatched_custody() {
        // Fabricated receipts are refusal inputs only, never runtime evidence.
        let root =
            std::env::temp_dir().join(format!("ktlsp-custody-export-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let context = CaptureContext::Official {
            identity: json!({"test":"not official evidence"}),
            authorization: json!({"test":"not authorization"}),
        };
        let script = [json!({"jsonrpc":"2.0","method":"exit"})];
        let chronology = [RawReference {
            direction: "client",
            frame_index: 0,
        }];
        let transcript = Transcript {
            chronology: &chronology,
            script: &script,
            decoded: &[],
        };
        let baseline = json!({
            "schemaVersion":1,"profile":confinement::profile(),
            "landlockAbi":6,"parentPid":std::process::id(),"parentDumpable":0,
            "hookOutcome":"ENFORCED_BEFORE_EXEC"
        });
        let mut receipts = vec![None];
        for (pointer, value) in [
            ("/schemaVersion", json!(2)),
            ("/landlockAbi", json!(null)),
            ("/landlockAbi", json!(5)),
            ("/parentPid", json!(0)),
            ("/parentDumpable", json!(1)),
            ("/hookOutcome", json!("NOT_RUN")),
            ("/hookOutcome", json!("REFUSED_NO_OFFICIAL_EXEC")),
            ("/profile/allowedAccessFs", json!(0xffff)),
        ] {
            let mut changed = baseline.clone();
            *changed.pointer_mut(pointer).unwrap() = value;
            receipts.push(Some(changed));
        }
        for (index, receipt) in receipts.iter().enumerate() {
            let directory = root.join(index.to_string());
            fs::create_dir(&directory).unwrap();
            assert!(
                export(
                    &directory,
                    &transcript,
                    &context,
                    Some(&std::process::ExitStatus::from_raw(0)),
                    receipt.as_ref()
                )
                .is_err()
            );
            assert!(!directory.join("transcript.json").exists());
            assert!(!directory.join("transcript.next").exists());
            assert!(!directory.join("metadata.json").exists());
            assert!(crate::Evidence::load(&directory).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn combined_export_byte_budget_refuses_consumer_files_without_transport_timing() {
        let root = std::env::temp_dir().join(format!("ktlsp-export-budget-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let mut script = [
            json!({"jsonrpc":"2.0","id":1,"method":"shutdown","params":{"text":""}}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let mut decoded = [DecodedFrame {
            start: 0,
            end: 0,
            message: json!({"jsonrpc":"2.0","id":1,"result":""}),
        }];
        let chronology = [
            RawReference {
                direction: "client",
                frame_index: 0,
            },
            RawReference {
                direction: "server",
                frame_index: 0,
            },
            RawReference {
                direction: "client",
                frame_index: 1,
            },
        ];
        let overhead = write_transcript(
            &mut io::sink(),
            &Transcript {
                chronology: &chronology,
                script: &script,
                decoded: &decoded,
            },
        )
        .unwrap();
        let client_length = (MAX_BYTES - overhead) / 2;
        script[0]["params"]["text"] = Value::String("x".repeat(client_length));
        for extra in [0, 1] {
            let directory = root.join(extra.to_string());
            fs::create_dir(&directory).unwrap();
            decoded[0].message["result"] =
                Value::String("x".repeat(MAX_BYTES - overhead - client_length + extra));
            assert!(serde_json::to_vec(&script).unwrap().len() < MAX_BYTES);
            assert!(serde_json::to_vec(&decoded[0].message).unwrap().len() < MAX_BYTES);
            let result = export(
                &directory,
                &Transcript {
                    chronology: &chronology,
                    script: &script,
                    decoded: &decoded,
                },
                &CaptureContext::Synthetic,
                Some(&std::process::ExitStatus::from_raw(0)),
                None,
            );
            if extra == 0 {
                // Same filesystem, valid sequential evidence, exactly at the
                // consumer cap; the second case adds only one ASCII byte.
                assert_eq!(result.unwrap(), MAX_BYTES);
                assert_eq!(
                    fs::metadata(directory.join("transcript.json"))
                        .unwrap()
                        .len(),
                    MAX_BYTES as u64
                );
                assert!(crate::Evidence::load(&directory).is_ok());
            } else {
                assert!(result.is_err());
                assert!(!directory.join("transcript.json").exists());
                assert!(!directory.join("transcript.next").exists());
                assert!(!directory.join("metadata.json").exists());
                assert!(crate::Evidence::load(&directory).is_err());
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Write};

    #[test]
    fn export_preserves_chronological_references() {
        let script = [
            json!({"jsonrpc":"2.0","id":1,"method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let decoded = [DecodedFrame {
            start: 0,
            end: 42,
            message: json!({"jsonrpc":"2.0","id":1,"result":null}),
        }];
        let chronology = [
            RawReference {
                direction: "client",
                frame_index: 0,
            },
            RawReference {
                direction: "server",
                frame_index: 0,
            },
            RawReference {
                direction: "client",
                frame_index: 1,
            },
        ];
        let mut output = Vec::new();
        let bytes = write_transcript(
            &mut output,
            &Transcript {
                chronology: &chronology,
                script: &script,
                decoded: &decoded,
            },
        )
        .unwrap();
        assert_eq!(bytes, output.len());
        assert_eq!(
            serde_json::from_slice::<Value>(&output).unwrap(),
            json!([
                {"direction":"client","message":script[0]},
                {"direction":"server","message":decoded[0].message},
                {"direction":"client","message":script[1]}
            ])
        );
        let metadata = serde_json::to_vec(&CaptureContext::Synthetic.metadata(0)).unwrap();
        assert!(crate::Evidence::parse(&output, &metadata).is_ok());
    }

    #[test]
    fn export_combines_directional_record_limits() {
        let script = [json!({"jsonrpc":"2.0","method":"exit"})];
        let decoded = [DecodedFrame {
            start: 0,
            end: 1,
            message: json!({"jsonrpc":"2.0","method":"notice"}),
        }];
        let mut chronology: Vec<_> = (0..MAX_MESSAGES)
            .map(|index| RawReference {
                direction: if index % 2 == 0 { "client" } else { "server" },
                frame_index: 0,
            })
            .collect();
        assert!(
            write_transcript(
                &mut io::sink(),
                &Transcript {
                    chronology: &chronology,
                    script: &script,
                    decoded: &decoded
                },
            )
            .is_ok()
        );
        chronology.push(RawReference {
            direction: "client",
            frame_index: 0,
        });
        assert_eq!(
            write_transcript(
                &mut io::sink(),
                &Transcript {
                    chronology: &chronology,
                    script: &script,
                    decoded: &decoded
                },
            )
            .unwrap_err(),
            "transcript record limit"
        );
    }

    #[test]
    fn export_byte_boundary_and_storage_errors_are_not_success() {
        let mut script = [json!({"jsonrpc":"2.0","method":"exit","params":{"text":""}})];
        let chronology = [RawReference {
            direction: "client",
            frame_index: 0,
        }];
        let base = write_transcript(
            &mut io::sink(),
            &Transcript {
                chronology: &chronology,
                script: &script,
                decoded: &[],
            },
        )
        .unwrap();
        script[0]["params"]["text"] = json!("x".repeat(MAX_BYTES - base));
        assert_eq!(
            write_transcript(
                &mut io::sink(),
                &Transcript {
                    chronology: &chronology,
                    script: &script,
                    decoded: &[]
                },
            )
            .unwrap(),
            MAX_BYTES
        );
        script[0]["params"]["text"] = json!("x".repeat(MAX_BYTES - base + 1));
        assert_eq!(
            write_transcript(
                &mut io::sink(),
                &Transcript {
                    chronology: &chronology,
                    script: &script,
                    decoded: &[]
                },
            )
            .unwrap_err(),
            "transcript byte limit"
        );

        struct FailedStorage;
        impl Write for FailedStorage {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("storage unavailable"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert!(
            write_transcript(
                &mut FailedStorage,
                &Transcript {
                    chronology: &chronology,
                    script: &script,
                    decoded: &[]
                },
            )
            .unwrap_err()
            .contains("storage unavailable")
        );
    }

    #[test]
    fn initial_official_manifest_is_configured_and_unverified() {
        let context = CaptureContext::Official {
            identity: json!({"identity":"frozen"}),
            authorization: json!({"authorization":"bound"}),
        };
        let report = context.initial_manifest(CaptureLimits::default());
        assert_eq!(report["identity"], json!({"identity":"frozen"}));
        assert_eq!(report["authorization"], json!({"authorization":"bound"}));
        let initial: confinement::Receipt =
            serde_json::from_value(report["childConfinement"].clone()).unwrap();
        assert_eq!(initial.profile, confinement::profile());
        assert_eq!(initial.parent_pid, std::process::id());
        assert_eq!(initial.landlock_abi, None);
        assert_eq!(initial.parent_dumpable, None);
        assert_eq!(initial.hook_outcome, "NOT_RUN");
        assert!(initial.validate_enforced(std::process::id()).is_err());
        assert_eq!(
            report["environment"],
            json!({
                "PATH":"/usr/bin:/bin","LANG":"C.UTF-8","LC_ALL":"C.UTF-8"
            })
        );
        let mut command = std::process::Command::new("/nonexistent/not-spawned");
        command.env_clear();
        context.configure_command(&mut command);
        let environment: std::collections::BTreeMap<_, _> = command
            .get_envs()
            .map(|(key, value)| (key.to_str().unwrap(), value.unwrap().to_str().unwrap()))
            .collect();
        assert_eq!(
            environment,
            std::collections::BTreeMap::from([
                ("LANG", "C.UTF-8"),
                ("LC_ALL", "C.UTF-8"),
                ("PATH", "/usr/bin:/bin"),
            ])
        );
        let mut synthetic_command = std::process::Command::new("/nonexistent/not-spawned");
        synthetic_command.env_clear();
        CaptureContext::Synthetic.configure_command(&mut synthetic_command);
        assert_eq!(synthetic_command.get_envs().count(), 0);
        assert_eq!(report["officialAcceptance"], false);
        assert_eq!(report["complete"], false);
        for key in ["runtime", "containment", "receiver"] {
            assert_eq!(report[key], "unverified");
        }
        let synthetic = CaptureContext::Synthetic.initial_manifest(CaptureLimits::default());
        assert!(synthetic.get("environment").is_none());
        assert_eq!(
            synthetic["scope"],
            "synthetic-only; official live execution blocked"
        );
        assert_eq!(synthetic["provenance"]["archive"], "none");
        assert_eq!(
            CaptureContext::Synthetic.metadata(0)["release"],
            "synthetic-only"
        );
        assert_eq!(context.metadata(0)["officialAcceptance"], false);
    }
}

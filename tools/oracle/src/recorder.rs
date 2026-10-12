//! Synthetic-only raw stdio capture. Official execution requires a separate live gate.
use crate::{MAX_BYTES, MAX_MESSAGES, MAX_STDERR, envelope, id};
use ktlsp_protocol::{MAX_HEADER_BYTES, ProtocolError, read_frame, write_frame};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path};

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

/// Fresh local evidence directory; command must be an absolute executable path.
/// Errors after launch are saved in manifest.json and returned as a partial report.
pub fn record(
    script: &[Value],
    command: &[String],
    directory: &Path,
    limits: CaptureLimits,
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
    platform::run(script, &frames, command, directory, limits)
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
    ) -> Result<Value, String> {
        fs::create_dir(directory).map_err(|e| format!("fresh evidence directory: {e}"))?;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        let directory = fs::canonicalize(directory).map_err(|e| e.to_string())?;
        let mut report = json!({"schemaVersion":1,"scope":"synthetic-only; official live execution blocked", "complete":false,"failureReason":"capture interrupted before final manifest", "limits":limits, "cleanup":{"containment":"isolated process group; escaped sessions/groups unsupported"}});
        save(&directory, &report).map_err(|e| e.to_string())?;
        let result = capture(script, frames, command, &directory, limits, &mut report);
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
        }
        let child = Command::new(&command[0])
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
            .process_group(0)
            .spawn()
            .map_err(|e| format!("spawn: {e}"))?;
        let pgid = i32::try_from(child.id()).map_err(|e| e.to_string())?;
        let mut group = Group {
            child,
            pgid,
            killed: false,
        };
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
        let mut decoded = Vec::<Value>::new();
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
                    decoded.push(json!({"start":start,"end":consumed,"message":message}));
                    envelope(&message)?;
                    let key = id(&message)?;
                    if message.get("method").is_some() {
                        if key.is_some() {
                            return Err("unexpected server request".into());
                        }
                    } else if key.is_none() || key != pending {
                        return Err("unexpected/duplicate response ID".into());
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
        let complete = failure.is_none() && cleanup_error.is_none() && script_done;
        report["complete"] = json!(complete);
        report["failureReason"] = json!(failure.as_ref().or(cleanup_error.as_ref()));
        report["scriptCompleted"] = json!(script_done);
        report["clientFrames"] = json!(outgoing);
        report["stdoutFrames"] = json!(decoded);
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
}

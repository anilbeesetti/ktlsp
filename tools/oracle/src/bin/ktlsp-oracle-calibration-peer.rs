//! Finite Rust-only containment calibration, never official oracle evidence.
//! Host handshake: observe `capture/calibration-ready.json` and original /proc
//! identities/resources, then exclusively create `calibration-observer-ready`
//! under the writable state mount as root:root, mode 0444, bytes `observed\n`.
//! Reports are bounded self-observations; only the host can attest teardown,
//! host namespace isolation, the actual kill, or authenticated step cancellation.
#[cfg(any(target_os = "linux", test))]
mod peer {
use ktlsp_oracle_replay::calibration_peer::{
    Case, Outcome, ProcessIdentity, Report, absent_or_denied, egress_denied,
    exhaustion_reached, mountpoint_denied, namespace_denied, permission_denied,
};
#[cfg(target_os = "linux")]
use ktlsp_oracle_replay::calibration_peer::{Exhaustion, Probe};

fn parse_case(value: &str) -> Result<Case, String> {
    Case::ALL.into_iter().find(|case| case.as_str() == value)
        .ok_or_else(|| "unknown calibration case".into())
}
fn descendant_case(case: Case) -> bool {
    matches!(case, Case::SameGroup | Case::Setsid | Case::Setpgid)
}
#[derive(Debug, Eq, PartialEq)]
enum Mode {
    Peer(Case),
    Descendant(Case),
}
fn arguments(args: &[String]) -> Result<Mode, String> {
    if args.len() != 2 {
        return Err("expected exactly --case CASE".into());
    }
    let case = parse_case(&args[1])?;
    match args[0].as_str() {
        "--case" => Ok(Mode::Peer(case)),
        "--descendant" if descendant_case(case) => Ok(Mode::Descendant(case)),
        _ => Err("only --case CASE or fixed private descendant mode is accepted".into()),
    }
}

fn process_stat(raw: &str, executable_device: u64, executable_inode: u64) -> Result<ProcessIdentity, String> {
    let (pid, _) = raw.split_once(" (").ok_or("process stat prefix")?;
    let (_, rest) = raw.rsplit_once(") ").ok_or("process stat comm")?;
    let mut fields = rest.split_whitespace();
    fields.next().ok_or("short process stat state")?;
    let narrow = |field: Option<&str>| -> Result<u32, String> {
        field.ok_or("short process stat")?.parse().map_err(|_| "process stat PID integer".into())
    };
    let identity = ProcessIdentity {
        pid: pid.parse().map_err(|_| "process stat PID")?,
        parent_pid: narrow(fields.next())?,
        process_group: narrow(fields.next())?,
        session: narrow(fields.next())?,
        start_time_ticks: fields.nth(15).ok_or("short process stat start time")?
            .parse().map_err(|_| "process stat start time integer")?,
        executable_device,
        executable_inode,
    };
    if identity.pid == 0 || identity.start_time_ticks == 0 || executable_inode == 0 {
        return Err("zero original process identity".into());
    }
    Ok(identity)
}
fn report_bytes(report: &Report) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec(report).map_err(|e| e.to_string())?;
    if bytes.len() > 16_384 {
        return Err("calibration report exceeds 16384 bytes".into());
    }
    Ok(bytes)
}
fn first_failure(report: &mut Report, error: String) {
    if report.first_failure.is_none() {
        report.first_failure = Some(error);
    }
    report.outcome = Outcome::Failed;
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::{
        ffi::{CString, c_char, c_int, c_ulong, c_void},
        fs::{self, File, OpenOptions},
        io::{Read, Seek, SeekFrom, Write},
        os::{fd::{AsRawFd, FromRawFd, OwnedFd}, unix::fs::{MetadataExt, OpenOptionsExt}},
        path::{Path, PathBuf},
        process::{Child, Command, Stdio},
        thread,
        time::{Duration, Instant},
    };
    const INNER: &str = "/tmp/ktlsp-t045-pr174-host";
    const CAPTURE: &str = "/tmp/ktlsp-t045-pr174-host/capture";
    const PHASE: Duration = Duration::from_secs(60);
    // The host exercises its 300s session timeout first; this independent finite fallback
    // stays inside the shared 540s watchdog trigger, never races the intended host kill.
    const SESSION: Duration = Duration::from_secs(530);
    const NOFOLLOW: i32 = 0x20000;
    const NONBLOCK: i32 = 0x800;
    unsafe extern "C" {
        fn getuid() -> u32;
        fn geteuid() -> u32;
        fn getgid() -> u32;
        fn getegid() -> u32;
        fn setsid() -> c_int;
        fn setpgid(pid: c_int, pgid: c_int) -> c_int;
        fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
        fn socket(domain: c_int, kind: c_int, protocol: c_int) -> c_int;
        fn connect(fd: c_int, address: *const c_void, length: u32) -> c_int;
        fn umount2(target: *const c_char, flags: c_int) -> c_int;
        fn mount(source: *const c_char, target: *const c_char, kind: *const c_char, flags: c_ulong, data: *const c_void) -> c_int;
        fn unshare(flags: c_int) -> c_int;
    }
    fn error(operation: &str, e: std::io::Error) -> String {
        format!("{operation}: {e} (errno {:?})", e.raw_os_error())
    }
    fn identity(pid: u32) -> Result<ProcessIdentity, String> {
        let root = PathBuf::from(format!("/proc/{pid}"));
        let before = bounded_read(&root.join("stat"), 4096)?;
        let exe = fs::metadata(root.join("exe")).map_err(|e| error("original executable metadata", e))?;
        let after = bounded_read(&root.join("stat"), 4096)?;
        let before = process_stat(std::str::from_utf8(&before).map_err(|_| "stat encoding")?, exe.dev(), exe.ino())?;
        let after = process_stat(std::str::from_utf8(&after).map_err(|_| "stat encoding")?, exe.dev(), exe.ino())?;
        if before.pid != pid || before.pid != after.pid || before.start_time_ticks != after.start_time_ticks {
            return Err("original process identity changed while sampled".into());
        }
        Ok(after)
    }
    fn bounded_read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
        let file = File::open(path).map_err(|e| error("open bounded input", e))?;
        let mut bytes = Vec::new();
        file.take(limit as u64 + 1).read_to_end(&mut bytes).map_err(|e| error("read bounded input", e))?;
        if bytes.len() > limit {
            return Err("bounded input overflow".into());
        }
        Ok(bytes)
    }
    struct ReservedReport {
        file: File,
        temporary: PathBuf,
    }
    impl ReservedReport {
        fn new() -> Result<Self, String> {
            let temporary = Path::new(CAPTURE).join(".calibration-peer.json.next");
            let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600)
                .custom_flags(NOFOLLOW | NONBLOCK).open(&temporary).map_err(|e| error("reserve report inode", e))?;
            // Real allocated pages, not a sparse set_len reservation. Keep the FD
            // open through ENOSPC, overwrite before truncating, then rename.
            file.write_all(&[0; 16_384]).map_err(|e| error("reserve report bytes", e))?;
            Ok(Self { file, temporary })
        }
        fn publish(&mut self, report: &Report) -> Result<(), String> {
            let bytes = report_bytes(report)?;
            self.file.seek(SeekFrom::Start(0)).map_err(|e| error("seek reserved report", e))?;
            self.file.write_all(&bytes).map_err(|e| error("write reserved report", e))?;
            self.file.set_len(bytes.len() as u64).map_err(|e| error("truncate reserved report", e))?;
            self.file.sync_all().map_err(|e| error("sync reserved report", e))?;
            fs::rename(&self.temporary, Path::new(CAPTURE).join("calibration-peer.json")).map_err(|e| error("rename reserved report", e))?;
            Ok(())
        }
    }
    fn publish_fresh(name: &str, bytes: &[u8]) -> Result<(), String> {
        let target = Path::new(CAPTURE).join(name);
        let temporary = Path::new(CAPTURE).join(format!(".{name}.next"));
        let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600)
            .custom_flags(NOFOLLOW | NONBLOCK).open(&temporary).map_err(|e| error("create fresh readiness", e))?;
        file.write_all(bytes).map_err(|e| error("write readiness", e))?;
        file.sync_all().map_err(|e| error("sync readiness", e))?;
        fs::rename(temporary, target).map_err(|e| error("rename readiness", e))
    }
    fn require_identity() -> Result<(), String> {
        // Refuse accidental host/root execution before creating any files.
        if unsafe { getuid() != 65532 || geteuid() != 65532 || getgid() != 65532 || getegid() != 65532 } {
            return Err("calibration requires real/effective UID/GID 65532".into());
        }
        Ok(())
    }
    fn descendant(case: Case) -> Result<(), String> {
        let started = Instant::now();
        let result = match case {
            Case::Setsid => unsafe { setsid() },
            Case::Setpgid => unsafe { setpgid(0, 0) },
            Case::SameGroup => 0,
            _ => return Err("non-descendant case".into()),
        };
        if result < 0 {
            return Err(error("descendant group/session syscall", std::io::Error::last_os_error()));
        }
        let identity = identity(std::process::id())?;
        publish_fresh(&format!("child-{}.json", case.as_str()), &serde_json::to_vec(&identity).map_err(|e| e.to_string())?)?;
        linger(started);
        Err("finite descendant lifetime expired without host cleanup".into())
    }
    fn linger(started: Instant) {
        while started.elapsed() < SESSION {
            thread::yield_now();
        }
    }
    fn spawn_descendant(case: Case, parent: &ProcessIdentity) -> Result<(Child, ProcessIdentity), String> {
        let executable = std::env::current_exe().map_err(|e| error("current peer executable", e))?;
        let mut child = Command::new(executable).args(["--descendant", case.as_str()])
            .env_clear().stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
            .spawn().map_err(|e| error("spawn fixed descendant", e))?;
        let started = Instant::now();
        let path = Path::new(CAPTURE).join(format!("child-{}.json", case.as_str()));
        loop {
            if let Some(status) = child.try_wait().map_err(|e| error("poll fixed descendant", e))? {
                return Err(format!("fixed descendant exited before observation: {status}"));
            }
            match fs::symlink_metadata(&path) {
                Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => {
                    let ready: ProcessIdentity = serde_json::from_slice(&bounded_read(&path, 4096)?).map_err(|e| e.to_string())?;
                    let actual = identity(child.id())?;
                    if ready.pid != actual.pid || ready.start_time_ticks != actual.start_time_ticks
                        || actual.parent_pid != parent.pid || actual.executable_device != parent.executable_device
                        || actual.executable_inode != parent.executable_inode {
                        return Err("fixed descendant original identity mismatch".into());
                    }
                    let group_ok = match case {
                        Case::SameGroup => actual.process_group == parent.process_group && actual.session == parent.session,
                        Case::Setsid => actual.process_group == actual.pid && actual.session == actual.pid,
                        Case::Setpgid => actual.process_group == actual.pid && actual.session == parent.session,
                        _ => false,
                    };
                    if !group_ok {
                        return Err("descendant real group/session mismatch".into());
                    }
                    return Ok((child, actual));
                }
                Ok(_) => return Err("invalid descendant readiness file".into()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(error("descendant readiness metadata", e)),
            }
            if started.elapsed() >= PHASE {
                return Err("descendant readiness phase expired".into());
            }
            thread::yield_now();
        }
    }
    fn observer_gate() -> Result<(), String> {
        let started = Instant::now();
        let path = Path::new(INNER).join("calibration-observer-ready");
        loop {
            match OpenOptions::new().read(true).custom_flags(NOFOLLOW | NONBLOCK).open(&path) {
                Ok(file) => {
                    let m = file.metadata().map_err(|e| error("observer gate metadata", e))?;
                    if !m.is_file() || m.uid() != 0 || m.gid() != 0 || m.mode() & 0o7777 != 0o444 || m.nlink() != 1 || m.len() != 9 {
                        return Err("observer gate is not the exact root-owned regular gate".into());
                    }
                    let mut bytes = Vec::new();
                    file.take(10).read_to_end(&mut bytes).map_err(|e| error("read observer gate", e))?;
                    if bytes != b"observed\n" {
                        return Err("observer gate payload mismatch".into());
                    }
                    return Ok(());
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(error("open observer gate", e)),
            }
            if started.elapsed() >= PHASE {
                return Err("host observer gate phase expired".into());
            }
            thread::yield_now();
        }
    }
    fn record(report: &mut Report, name: &str, operation: &str, result: i64, errno: Option<i32>, accepted: bool) -> Result<(), String> {
        report.probes.push(Probe { name: name.into(), operation: operation.into(), return_value: result, errno, accepted });
        if accepted { Ok(()) } else { Err(format!("boundary probe {name} failed: return {result}, errno {errno:?}")) }
    }
    fn syscall_probe(report: &mut Report, name: &str, operation: &str, result: c_int, classify: fn(Option<i32>) -> bool) -> Result<(), String> {
        let errno = if result < 0 { std::io::Error::last_os_error().raw_os_error() } else { None };
        record(report, name, operation, i64::from(result), errno, result < 0 && classify(errno))
    }
    fn io_probe<T>(report: &mut Report, name: &str, operation: &str, result: std::io::Result<T>, classify: fn(Option<i32>) -> bool) -> Result<(), String> {
        match result {
            Ok(_) => record(report, name, operation, 0, None, false),
            Err(e) => record(report, name, operation, -1, e.raw_os_error(), classify(e.raw_os_error())),
        }
    }
    fn open_socket(report: &mut Report, family: c_int, name: &str) -> Result<Option<OwnedFd>, String> {
        let result = unsafe { socket(family, 1 | 0x800 | 0x80000, 0) }; // STREAM, NONBLOCK, CLOEXEC.
        let errno = if result < 0 { std::io::Error::last_os_error().raw_os_error() } else { None };
        record(report, name, "socket", i64::from(result), errno, result >= 0 || namespace_denied(errno))?;
        if result < 0 { Ok(None) } else { Ok(Some(unsafe { OwnedFd::from_raw_fd(result) })) }
    }
    fn boundary_denials(report: &mut Report) -> Result<(), String> {
        io_probe(report, "host-proc-unavailable", "open /host/proc/1/stat", File::open("/host/proc/1/stat"), absent_or_denied)?;
        let namespace = File::open("/proc/self/ns/pid").map_err(|e| error("open own PID namespace", e))?;
        let result = unsafe { ioctl(namespace.as_raw_fd(), 0xb702) }; // NS_GET_PARENT.
        if result >= 0 {
            let _unexpected_parent_namespace = unsafe { OwnedFd::from_raw_fd(result) };
        }
        syscall_probe(report, "ancestor-pid-namespace", "ioctl NS_GET_PARENT", result, namespace_denied)?;
        if let Some(socket) = open_socket(report, 2, "egress-socket")? {
            #[repr(C)]
            struct InetAddress { family: u16, port: u16, address: [u8; 4], zero: [u8; 8] }
            let address = InetAddress { family: 2, port: 443u16.to_be(), address: [203, 0, 113, 1], zero: [0; 8] };
            let result = unsafe { connect(socket.as_raw_fd(), (&address as *const InetAddress).cast(), 16) };
            syscall_probe(report, "network-egress", "connect 203.0.113.1:443", result, egress_denied)?;
        }
        for path in ["/run/docker.sock", "/var/run/docker.sock"] {
            if let Some(socket) = open_socket(report, 1, &format!("daemon-socket-{path}"))? {
                #[repr(C)]
                struct UnixAddress { family: u16, path: [u8; 108] }
                let mut address = UnixAddress { family: 1, path: [0; 108] };
                address.path[..path.len()].copy_from_slice(path.as_bytes());
                let result = unsafe { connect(socket.as_raw_fd(), (&address as *const UnixAddress).cast(), (2 + path.len() + 1) as u32) };
                syscall_probe(report, &format!("daemon-connect-{path}"), "connect fixed daemon socket", result, absent_or_denied)?;
            }
        }
        for file in ["fixture/declarations.kt", "fixture/error-node.kt", "preflight/identity.json"] {
            // No truncation: if writable open unexpectedly succeeds, report the
            // breach without gratuitously destroying the sealed input bytes.
            io_probe(report, file, "open existing input O_WRONLY", OpenOptions::new().write(true).custom_flags(NOFOLLOW | NONBLOCK).open(Path::new(INNER).join(file)), permission_denied)?;
        }
        for root in ["fixture", "distribution", "tool", "preflight"] {
            let path = Path::new(INNER).join(root);
            let cpath = CString::new(path.as_os_str().as_encoded_bytes()).map_err(|_| "fixed path NUL")?;
            io_probe(report, &format!("rename-{root}"), "rename mountpoint", fs::rename(&path, Path::new(INNER).join(format!("calibration-renamed-{root}"))), mountpoint_denied)?;
            io_probe(report, &format!("remove-{root}"), "rmdir mountpoint", fs::remove_dir(&path), mountpoint_denied)?;
            let result = unsafe { umount2(cpath.as_ptr(), 0) };
            syscall_probe(report, &format!("unmount-{root}"), "umount2 mountpoint", result, namespace_denied)?;
            let source = CString::new(CAPTURE).map_err(|_| "fixed source NUL")?;
            let result = unsafe { mount(source.as_ptr(), cpath.as_ptr(), std::ptr::null(), 4096, std::ptr::null()) }; // MS_BIND.
            syscall_probe(report, &format!("bind-over-{root}"), "mount MS_BIND over input", result, namespace_denied)?;
        }
        for (name, flags) in [
            ("user", 0x10000000), ("mount", 0x00020000), ("pid", 0x20000000),
            ("network", 0x40000000), ("ipc", 0x08000000), ("uts", 0x04000000), ("cgroup", 0x02000000),
        ] {
            let result = unsafe { unshare(flags) };
            syscall_probe(report, &format!("unshare-{name}"), "unshare namespace", result, namespace_denied)?;
        }
        Ok(())
    }
    fn exhaust(report: &mut Report, inode: bool) -> Result<(), String> {
        let started = Instant::now();
        let root = Path::new(CAPTURE).join("exhaustion");
        fs::create_dir(&root).map_err(|e| error("create exhaustion directory", e))?;
        let mut observation = Exhaustion { bytes_written: 0, files_created: 0, terminal_errno: None, reached_enospc: false };
        let result = if inode {
            let mut result = Err("inode bound reached without ENOSPC".into());
            for index in 0..16_384u32 {
                if started.elapsed() >= PHASE { result = Err("inode exhaustion phase expired".into()); break; }
                match OpenOptions::new().write(true).create_new(true).mode(0o600).open(root.join(format!("{index:05}"))) {
                    Ok(_) => observation.files_created += 1,
                    Err(e) => {
                        observation.terminal_errno = e.raw_os_error();
                        observation.reached_enospc = exhaustion_reached(e.raw_os_error());
                        result = if observation.reached_enospc { Ok(()) } else { Err(error("inode exhaustion", e)) };
                        break;
                    }
                }
                thread::yield_now();
            }
            result
        } else {
            let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(root.join("data"))
                .map_err(|e| error("create storage exhaustion file", e))?;
            observation.files_created = 1;
            let buffer = [0x5au8; 65_536];
            let mut result = Err("512MiB storage bound reached without ENOSPC".into());
            // Never write more than 512MiB. Reserved report pages consume real
            // space, so this exact template must reach ENOSPC before the cap.
            while observation.bytes_written < 536_870_912 {
                if started.elapsed() >= PHASE { result = Err("storage exhaustion phase expired".into()); break; }
                let count = (536_870_912 - observation.bytes_written).min(buffer.len() as u64) as usize;
                match file.write(&buffer[..count]) {
                    Ok(0) => { result = Err("storage exhaustion write returned zero".into()); break; }
                    Ok(count) => observation.bytes_written += count as u64,
                    Err(e) => {
                        observation.terminal_errno = e.raw_os_error();
                        observation.reached_enospc = exhaustion_reached(e.raw_os_error());
                        result = if observation.reached_enospc { Ok(()) } else { Err(error("storage exhaustion", e)) };
                        break;
                    }
                }
                thread::yield_now();
            }
            result
        };
        report.exhaustion = Some(observation);
        result
    }
    fn peer(case: Case) -> Result<(), String> {
        let started = Instant::now();
        let original_process = identity(std::process::id())?;
        let gate = Path::new(INNER).join("calibration-observer-ready");
        match fs::symlink_metadata(gate) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(error("check fresh observer gate", e)),
            Ok(_) => return Err("observer gate already exists before peer readiness".into()),
        }
        fs::create_dir(CAPTURE).map_err(|e| error("create exclusive capture directory", e))?;
        let mut output = ReservedReport::new()?;
        let mut report = Report {
            schema_version: 1,
            scope: "PR174_RUST_CALIBRATION_PEER".into(),
            case,
            outcome: Outcome::AwaitingObserver,
            original_process,
            children: Vec::new(),
            observer_gate_observed: false,
            probes: Vec::new(),
            exhaustion: None,
            first_failure: None,
            elapsed_ms: 0,
        };
        let mut child = None;
        let run = (|| {
            if descendant_case(case) {
                let (handle, identity) = spawn_descendant(case, &report.original_process)?;
                child = Some(handle);
                report.children.push(identity);
            }
            report.elapsed_ms = started.elapsed().as_millis() as u64;
            publish_fresh("calibration-ready.json", &report_bytes(&report)?)?;
            observer_gate()?;
            report.observer_gate_observed = true;
            if let Some(child) = &mut child {
                if let Some(status) = child.try_wait().map_err(|e| error("recheck observed descendant", e))? {
                    return Err(format!("descendant exited before host observer gate: {status}"));
                }
            }
            match case {
                Case::BoundaryDenials => boundary_denials(&mut report)?,
                Case::StorageExhaustion => exhaust(&mut report, false)?,
                Case::InodeExhaustion => exhaust(&mut report, true)?,
                _ => {}
            }
            report.outcome = if case.forced() { Outcome::AwaitingHostAction } else { Outcome::Completed };
            report.elapsed_ms = started.elapsed().as_millis() as u64;
            report.validate_case(case)?;
            Ok::<(), String>(())
        })();
        if let Err(error) = &run { first_failure(&mut report, error.clone()); }
        report.elapsed_ms = started.elapsed().as_millis() as u64;
        if let Err(publish_error) = output.publish(&report) {
            // Publication/cleanup must never replace the original first error.
            return Err(match run {
                Err(original) => format!("{original}; report publication also failed: {publish_error}"),
                Ok(()) => publish_error,
            });
        }
        run?;
        if case.forced() {
            linger(started);
            let original = "finite peer lifetime expired without host action".to_string();
            first_failure(&mut report, original.clone());
            report.elapsed_ms = started.elapsed().as_millis() as u64;
            // A second fresh reserved file atomically replaces the awaiting
            // report if the host never acts; natural expiry is never success.
            let published = ReservedReport::new().and_then(|mut output| output.publish(&report));
            return Err(match published {
                Ok(()) => original,
                Err(e) => format!("{original}; expiry report publication also failed: {e}"),
            });
        }
        Ok(())
    }
    pub(super) fn run(mode: Mode) -> Result<(), String> {
        require_identity()?;
        match mode {
            Mode::Peer(case) => peer(case),
            Mode::Descendant(case) => descendant(case),
        }
    }
}

#[cfg(target_os = "linux")]
pub(super) fn run() -> Result<(), String> {
    let mode = arguments(&std::env::args().skip(1).collect::<Vec<_>>())?;
    linux::run(mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn argv(values: &[&str]) -> Vec<String> { values.iter().map(|v| (*v).into()).collect() }
    fn synthetic_report() -> Report {
        Report {
            schema_version: 1, scope: "PR174_RUST_CALIBRATION_PEER".into(), case: Case::Normal,
            outcome: Outcome::AwaitingObserver,
            original_process: ProcessIdentity { pid: 2, parent_pid: 1, process_group: 2, session: 2, start_time_ticks: 9, executable_device: 1, executable_inode: 2 },
            children: Vec::new(), observer_gate_observed: false, probes: Vec::new(), exhaustion: None,
            first_failure: None, elapsed_ms: 0,
        }
    }
    #[test]
    fn synthetic_closed_arguments_never_launch_a_peer() {
        for name in ["normal", "same-group", "setsid", "setpgid", "timeout", "kill", "workflow-step-cancel", "storage-exhaustion", "inode-exhaustion", "boundary-denials"] {
            assert!(matches!(arguments(&argv(&["--case", name])), Ok(Mode::Peer(_))));
        }
        for args in [vec![], vec!["--case"], vec!["--case", "normal", "extra"], vec!["--case=normal"], vec!["--case", "/bin/sh"], vec!["--output", "normal"], vec!["--descendant", "timeout"]] {
            assert!(arguments(&argv(&args)).is_err());
        }
        assert_eq!(arguments(&argv(&["--descendant", "setsid"])), Ok(Mode::Descendant(Case::Setsid)));
    }
    #[test]
    fn synthetic_classifiers_require_the_actual_expected_errno() {
        assert!(permission_denied(Some(30)));
        assert!(!permission_denied(Some(2)));
        assert!(!namespace_denied(Some(22)));
        assert!(namespace_denied(Some(1)));
        assert!(absent_or_denied(Some(2)));
        assert!(!absent_or_denied(Some(111)));
        assert!(egress_denied(Some(101)));
        assert!(!egress_denied(Some(115))); // EINPROGRESS is not denial.
        assert!(!egress_denied(None));
        assert!(exhaustion_reached(Some(28)));
        assert!(!exhaustion_reached(Some(122)));
        assert!(!exhaustion_reached(None));
        assert!(mountpoint_denied(Some(16)));
        assert!(!mountpoint_denied(Some(2)));
    }
    #[test]
    fn synthetic_schema_and_first_failure_are_bounded_and_honest() {
        let mut report = synthetic_report();
        first_failure(&mut report, "original ENOSPC-related failure".into());
        first_failure(&mut report, "later publication error".into());
        assert_eq!(report.first_failure.as_deref(), Some("original ENOSPC-related failure"));
        let bytes = report_bytes(&report).unwrap();
        let decoded: Report = serde_json::from_slice(&bytes).unwrap();
        assert!(matches!(decoded.outcome, Outcome::Failed));
        assert_eq!(decoded.scope, "PR174_RUST_CALIBRATION_PEER");
        assert!(!decoded.observer_gate_observed);
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["hostClosureConfirmed"] = true.into();
        assert!(serde_json::from_value::<Report>(value).is_err());
        report.first_failure = Some("x".repeat(16_384));
        assert!(report_bytes(&report).is_err());
    }
    #[test]
    fn synthetic_proc_stat_handles_parentheses_and_original_start_time() {
        let stat = format!("42 (peer ) unusual) S 7 42 9 {} 123 0\n", ["0"; 15].join(" "));
        let identity = process_stat(&stat, 1, 2).unwrap();
        assert_eq!((identity.pid, identity.parent_pid, identity.process_group, identity.session, identity.start_time_ticks), (42, 7, 42, 9, 123));
        assert!(process_stat("42 (truncated", 1, 2).is_err());
        assert!(process_stat(&stat, 1, 0).is_err());
    }
}
}

fn main() {
    #[cfg(target_os = "linux")]
    let result = peer::run();
    #[cfg(not(target_os = "linux"))]
    let result: Result<(), String> = Err("real containment calibration requires Linux; no emulation or synthetic fallback".into());
    if let Err(error) = result {
        eprintln!("Rust containment calibration peer: {error}");
        std::process::exit(1);
    }
}

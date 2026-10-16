//! Finite Rust-only containment calibration, never official oracle evidence.
//! Host handshake: observe `capture/calibration-ready.json` and original /proc
//! identities/resources, then exclusively create `calibration-observer-ready`
//! under the writable state mount as root:root, mode 0444, bytes `observed\n`.
//! Reports are bounded self-observations; only the host can attest teardown,
//! host namespace isolation, the actual kill, or authenticated step cancellation.
//! Boundary custody uses the same production launcher before readiness. Its
//! fixed child receives only a parent-owned memory address and fixed FD slots
//! through bounded private stdio after the host gate, never a target PID/path.
//! Actual fork+setsid descendants inherit that production restriction domain.
#[cfg(any(target_os = "linux", test))]
mod peer {
    use ktlsp_oracle_replay::calibration_peer::{
        Case, Outcome, ProcessIdentity, Report, absent_or_denied, egress_denied,
        exhaustion_reached, mountpoint_denied, namespace_denied, permission_denied,
    };
    #[cfg(target_os = "linux")]
    use ktlsp_oracle_replay::calibration_peer::{
        CustodyAction, CustodyChild, CustodyFile, CustodyProbe, CustodyReceipt, CustodyRole,
        CustodyTarget, Exhaustion, Probe,
    };

    fn parse_case(value: &str) -> Result<Case, String> {
        Case::ALL
            .into_iter()
            .find(|case| case.as_str() == value)
            .ok_or_else(|| "unknown calibration case".into())
    }
    fn descendant_case(case: Case) -> bool {
        matches!(case, Case::SameGroup | Case::Setsid | Case::Setpgid)
    }
    #[derive(Debug, Eq, PartialEq)]
    enum Mode {
        Peer(Case),
        Descendant(Case),
        CustodyChild,
    }
    fn arguments(args: &[String]) -> Result<Mode, String> {
        if args.len() != 2 {
            return Err("expected exactly --case CASE".into());
        }
        if args[0] == "--custody-child" && args[1] == "direct" {
            return Ok(Mode::CustodyChild);
        }
        let case = parse_case(&args[1])?;
        match args[0].as_str() {
            "--case" => Ok(Mode::Peer(case)),
            "--descendant" if descendant_case(case) => Ok(Mode::Descendant(case)),
            _ => Err("only --case CASE or fixed private descendant mode is accepted".into()),
        }
    }

    fn process_stat(
        raw: &str,
        executable_device: u64,
        executable_inode: u64,
    ) -> Result<ProcessIdentity, String> {
        let (pid, _) = raw.split_once(" (").ok_or("process stat prefix")?;
        let (_, rest) = raw.rsplit_once(") ").ok_or("process stat comm")?;
        let mut fields = rest.split_whitespace();
        fields.next().ok_or("short process stat state")?;
        let narrow = |field: Option<&str>| -> Result<u32, String> {
            field
                .ok_or("short process stat")?
                .parse()
                .map_err(|_| "process stat PID integer".into())
        };
        let identity = ProcessIdentity {
            pid: pid.parse().map_err(|_| "process stat PID")?,
            parent_pid: narrow(fields.next())?,
            process_group: narrow(fields.next())?,
            session: narrow(fields.next())?,
            start_time_ticks: fields
                .nth(15)
                .ok_or("short process stat start time")?
                .parse()
                .map_err(|_| "process stat start time integer")?,
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
            os::{
                fd::{AsRawFd, FromRawFd, OwnedFd},
                unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
            },
            path::{Path, PathBuf},
            process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio},
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
            fn mount(
                source: *const c_char,
                target: *const c_char,
                kind: *const c_char,
                flags: c_ulong,
                data: *const c_void,
            ) -> c_int;
            fn unshare(flags: c_int) -> c_int;
        }
        fn error(operation: &str, e: std::io::Error) -> String {
            format!("{operation}: {e} (errno {:?})", e.raw_os_error())
        }
        fn identity(pid: u32) -> Result<ProcessIdentity, String> {
            let root = PathBuf::from(format!("/proc/{pid}"));
            let before = bounded_read(&root.join("stat"), 4096)?;
            let exe = fs::metadata(root.join("exe"))
                .map_err(|e| error("original executable metadata", e))?;
            let after = bounded_read(&root.join("stat"), 4096)?;
            let before = process_stat(
                std::str::from_utf8(&before).map_err(|_| "stat encoding")?,
                exe.dev(),
                exe.ino(),
            )?;
            let after = process_stat(
                std::str::from_utf8(&after).map_err(|_| "stat encoding")?,
                exe.dev(),
                exe.ino(),
            )?;
            if before.pid != pid
                || before.pid != after.pid
                || before.start_time_ticks != after.start_time_ticks
            {
                return Err("original process identity changed while sampled".into());
            }
            Ok(after)
        }
        fn bounded_read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
            let file = File::open(path).map_err(|e| error("open bounded input", e))?;
            let mut bytes = Vec::new();
            file.take(limit as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| error("read bounded input", e))?;
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
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .custom_flags(NOFOLLOW | NONBLOCK)
                    .open(&temporary)
                    .map_err(|e| error("reserve report inode", e))?;
                // Real allocated pages, not a sparse set_len reservation. Keep the FD
                // open through ENOSPC, overwrite before truncating, then rename.
                file.write_all(&[0; 16_384])
                    .map_err(|e| error("reserve report bytes", e))?;
                Ok(Self { file, temporary })
            }
            fn publish(&mut self, report: &Report) -> Result<(), String> {
                let bytes = report_bytes(report)?;
                self.file
                    .seek(SeekFrom::Start(0))
                    .map_err(|e| error("seek reserved report", e))?;
                self.file
                    .write_all(&bytes)
                    .map_err(|e| error("write reserved report", e))?;
                self.file
                    .set_len(bytes.len() as u64)
                    .map_err(|e| error("truncate reserved report", e))?;
                self.file
                    .sync_all()
                    .map_err(|e| error("sync reserved report", e))?;
                fs::rename(
                    &self.temporary,
                    Path::new(CAPTURE).join("calibration-peer.json"),
                )
                .map_err(|e| error("rename reserved report", e))?;
                Ok(())
            }
        }
        fn publish_fresh(name: &str, bytes: &[u8]) -> Result<(), String> {
            let target = Path::new(CAPTURE).join(name);
            let temporary = Path::new(CAPTURE).join(format!(".{name}.next"));
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(NOFOLLOW | NONBLOCK)
                .open(&temporary)
                .map_err(|e| error("create fresh readiness", e))?;
            file.write_all(bytes)
                .map_err(|e| error("write readiness", e))?;
            file.sync_all().map_err(|e| error("sync readiness", e))?;
            fs::rename(temporary, target).map_err(|e| error("rename readiness", e))
        }
        fn require_identity() -> Result<(), String> {
            // Refuse accidental host/root execution before creating any files.
            if unsafe {
                getuid() != 65532 || geteuid() != 65532 || getgid() != 65532 || getegid() != 65532
            } {
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
                return Err(error(
                    "descendant group/session syscall",
                    std::io::Error::last_os_error(),
                ));
            }
            let identity = identity(std::process::id())?;
            publish_fresh(
                &format!("child-{}.json", case.as_str()),
                &serde_json::to_vec(&identity).map_err(|e| e.to_string())?,
            )?;
            linger(started);
            Err("finite descendant lifetime expired without host cleanup".into())
        }
        fn linger(started: Instant) {
            while started.elapsed() < SESSION {
                thread::yield_now();
            }
        }
        fn spawn_descendant(
            case: Case,
            parent: &ProcessIdentity,
        ) -> Result<(Child, ProcessIdentity), String> {
            let executable =
                std::env::current_exe().map_err(|e| error("current peer executable", e))?;
            let mut child = Command::new(executable)
                .args(["--descendant", case.as_str()])
                .env_clear()
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| error("spawn fixed descendant", e))?;
            let started = Instant::now();
            let path = Path::new(CAPTURE).join(format!("child-{}.json", case.as_str()));
            loop {
                if let Some(status) = child
                    .try_wait()
                    .map_err(|e| error("poll fixed descendant", e))?
                {
                    return Err(format!(
                        "fixed descendant exited before observation: {status}"
                    ));
                }
                match fs::symlink_metadata(&path) {
                    Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => {
                        let ready: ProcessIdentity =
                            serde_json::from_slice(&bounded_read(&path, 4096)?)
                                .map_err(|e| e.to_string())?;
                        let actual = identity(child.id())?;
                        if ready.pid != actual.pid
                            || ready.start_time_ticks != actual.start_time_ticks
                            || actual.parent_pid != parent.pid
                            || actual.executable_device != parent.executable_device
                            || actual.executable_inode != parent.executable_inode
                        {
                            return Err("fixed descendant original identity mismatch".into());
                        }
                        let group_ok = match case {
                            Case::SameGroup => {
                                actual.process_group == parent.process_group
                                    && actual.session == parent.session
                            }
                            Case::Setsid => {
                                actual.process_group == actual.pid && actual.session == actual.pid
                            }
                            Case::Setpgid => {
                                actual.process_group == actual.pid
                                    && actual.session == parent.session
                            }
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
                match OpenOptions::new()
                    .read(true)
                    .custom_flags(NOFOLLOW | NONBLOCK)
                    .open(&path)
                {
                    Ok(file) => {
                        let m = file
                            .metadata()
                            .map_err(|e| error("observer gate metadata", e))?;
                        if !m.is_file()
                            || m.uid() != 0
                            || m.gid() != 0
                            || m.mode() & 0o7777 != 0o444
                            || m.nlink() != 1
                            || m.len() != 9
                        {
                            return Err(
                                "observer gate is not the exact root-owned regular gate".into()
                            );
                        }
                        let mut bytes = Vec::new();
                        file.take(10)
                            .read_to_end(&mut bytes)
                            .map_err(|e| error("read observer gate", e))?;
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
        fn record(
            report: &mut Report,
            name: &str,
            operation: &str,
            result: i64,
            errno: Option<i32>,
            accepted: bool,
        ) -> Result<(), String> {
            report.probes.push(Probe {
                name: name.into(),
                operation: operation.into(),
                return_value: result,
                errno,
                accepted,
            });
            if accepted {
                Ok(())
            } else {
                Err(format!(
                    "boundary probe {name} failed: return {result}, errno {errno:?}"
                ))
            }
        }
        fn syscall_probe(
            report: &mut Report,
            name: &str,
            operation: &str,
            result: c_int,
            classify: fn(Option<i32>) -> bool,
        ) -> Result<(), String> {
            let errno = if result < 0 {
                std::io::Error::last_os_error().raw_os_error()
            } else {
                None
            };
            record(
                report,
                name,
                operation,
                i64::from(result),
                errno,
                result < 0 && classify(errno),
            )
        }
        fn io_probe<T>(
            report: &mut Report,
            name: &str,
            operation: &str,
            result: std::io::Result<T>,
            classify: fn(Option<i32>) -> bool,
        ) -> Result<(), String> {
            match result {
                Ok(_) => record(report, name, operation, 0, None, false),
                Err(e) => record(
                    report,
                    name,
                    operation,
                    -1,
                    e.raw_os_error(),
                    classify(e.raw_os_error()),
                ),
            }
        }
        fn open_socket(
            report: &mut Report,
            family: c_int,
            name: &str,
        ) -> Result<Option<OwnedFd>, String> {
            let result = unsafe { socket(family, 1 | 0x800 | 0x80000, 0) }; // STREAM, NONBLOCK, CLOEXEC.
            let errno = if result < 0 {
                std::io::Error::last_os_error().raw_os_error()
            } else {
                None
            };
            record(
                report,
                name,
                "socket",
                i64::from(result),
                errno,
                result >= 0 || namespace_denied(errno),
            )?;
            if result < 0 {
                Ok(None)
            } else {
                Ok(Some(unsafe { OwnedFd::from_raw_fd(result) }))
            }
        }
        fn boundary_denials(report: &mut Report) -> Result<(), String> {
            io_probe(
                report,
                "host-proc-unavailable",
                "open /host/proc/1/stat",
                File::open("/host/proc/1/stat"),
                absent_or_denied,
            )?;
            let namespace =
                File::open("/proc/self/ns/pid").map_err(|e| error("open own PID namespace", e))?;
            let result = unsafe { ioctl(namespace.as_raw_fd(), 0xb702) }; // NS_GET_PARENT.
            if result >= 0 {
                let _unexpected_parent_namespace = unsafe { OwnedFd::from_raw_fd(result) };
            }
            syscall_probe(
                report,
                "ancestor-pid-namespace",
                "ioctl NS_GET_PARENT",
                result,
                namespace_denied,
            )?;
            if let Some(socket) = open_socket(report, 2, "egress-socket")? {
                #[repr(C)]
                struct InetAddress {
                    family: u16,
                    port: u16,
                    address: [u8; 4],
                    zero: [u8; 8],
                }
                let address = InetAddress {
                    family: 2,
                    port: 443u16.to_be(),
                    address: [203, 0, 113, 1],
                    zero: [0; 8],
                };
                let result = unsafe {
                    connect(
                        socket.as_raw_fd(),
                        (&address as *const InetAddress).cast(),
                        16,
                    )
                };
                syscall_probe(
                    report,
                    "network-egress",
                    "connect 203.0.113.1:443",
                    result,
                    egress_denied,
                )?;
            }
            for path in ["/run/docker.sock", "/var/run/docker.sock"] {
                if let Some(socket) = open_socket(report, 1, &format!("daemon-socket-{path}"))? {
                    #[repr(C)]
                    struct UnixAddress {
                        family: u16,
                        path: [u8; 108],
                    }
                    let mut address = UnixAddress {
                        family: 1,
                        path: [0; 108],
                    };
                    address.path[..path.len()].copy_from_slice(path.as_bytes());
                    let result = unsafe {
                        connect(
                            socket.as_raw_fd(),
                            (&address as *const UnixAddress).cast(),
                            (2 + path.len() + 1) as u32,
                        )
                    };
                    syscall_probe(
                        report,
                        &format!("daemon-connect-{path}"),
                        "connect fixed daemon socket",
                        result,
                        absent_or_denied,
                    )?;
                }
            }
            for file in [
                "fixture/declarations.kt",
                "fixture/error-node.kt",
                "preflight/identity.json",
            ] {
                // No truncation: if writable open unexpectedly succeeds, report the
                // breach without gratuitously destroying the sealed input bytes.
                io_probe(
                    report,
                    file,
                    "open existing input O_WRONLY",
                    OpenOptions::new()
                        .write(true)
                        .custom_flags(NOFOLLOW | NONBLOCK)
                        .open(Path::new(INNER).join(file)),
                    permission_denied,
                )?;
            }
            for root in ["fixture", "distribution", "tool", "preflight"] {
                let path = Path::new(INNER).join(root);
                let cpath = CString::new(path.as_os_str().as_encoded_bytes())
                    .map_err(|_| "fixed path NUL")?;
                io_probe(
                    report,
                    &format!("rename-{root}"),
                    "rename mountpoint",
                    fs::rename(
                        &path,
                        Path::new(INNER).join(format!("calibration-renamed-{root}")),
                    ),
                    mountpoint_denied,
                )?;
                io_probe(
                    report,
                    &format!("remove-{root}"),
                    "rmdir mountpoint",
                    fs::remove_dir(&path),
                    mountpoint_denied,
                )?;
                let result = unsafe { umount2(cpath.as_ptr(), 0) };
                syscall_probe(
                    report,
                    &format!("unmount-{root}"),
                    "umount2 mountpoint",
                    result,
                    namespace_denied,
                )?;
                let source = CString::new(CAPTURE).map_err(|_| "fixed source NUL")?;
                let result = unsafe {
                    mount(
                        source.as_ptr(),
                        cpath.as_ptr(),
                        std::ptr::null(),
                        4096,
                        std::ptr::null(),
                    )
                }; // MS_BIND.
                syscall_probe(
                    report,
                    &format!("bind-over-{root}"),
                    "mount MS_BIND over input",
                    result,
                    namespace_denied,
                )?;
            }
            for (name, flags) in [
                ("user", 0x10000000),
                ("mount", 0x00020000),
                ("pid", 0x20000000),
                ("network", 0x40000000),
                ("ipc", 0x08000000),
                ("uts", 0x04000000),
                ("cgroup", 0x02000000),
            ] {
                let result = unsafe { unshare(flags) };
                syscall_probe(
                    report,
                    &format!("unshare-{name}"),
                    "unshare namespace",
                    result,
                    namespace_denied,
                )?;
            }
            Ok(())
        }
        use ktlsp_oracle_replay::recorder::confinement;
        use serde::{Deserialize, Serialize};
        use std::os::{
            linux::net::SocketAddrExt,
            unix::net::{SocketAddr, UnixListener, UnixStream},
        };
        const CUSTODY_BYTES: usize = 10_240;
        const CUSTODY_FD_BASE: i32 = 100;
        const CUSTODY_ABSTRACT_SOCKET: &[u8] = b"ktlsp-pr174-calibration-parent";
        #[derive(Deserialize, Serialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct CustodyStart {
            schema_version: u8,
            memory_address: u64,
            evidence_fds: [i32; 5],
            run: bool,
        }
        #[derive(Deserialize, Serialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct CustodyFrame {
            children: Vec<CustodyChild>,
            first_failure: Option<String>,
        }
        impl CustodyFrame {
            fn from_result(result: Result<Vec<CustodyChild>, String>) -> Self {
                match result {
                    Ok(children) => Self {
                        children,
                        first_failure: None,
                    },
                    Err(error) => Self {
                        children: Vec::new(),
                        first_failure: Some(error),
                    },
                }
            }
            fn decode(
                bytes: &[u8],
                exit: Option<bool>,
                count: usize,
            ) -> Result<Vec<CustodyChild>, String> {
                let frame: Self = serde_json::from_slice(bytes)
                    .map_err(|e| format!("custody child JSON: {e}"))?;
                frame.into_children(exit, count)
            }
            fn into_children(
                self,
                exit: Option<bool>,
                count: usize,
            ) -> Result<Vec<CustodyChild>, String> {
                if let Some(original) = self.first_failure {
                    return Err(original);
                }
                if exit != Some(true) || self.children.len() != count {
                    return Err("custody child lacked complete original exit-zero cohort".into());
                }
                Ok(self.children)
            }
        }
        #[repr(C)]
        struct IoVector {
            base: *mut c_void,
            length: usize,
        }
        unsafe extern "C" {
            fn getppid() -> c_int;
            fn fork() -> c_int;
            fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
            fn _exit(status: c_int) -> !;
            fn fcntl(fd: c_int, command: c_int, ...) -> c_int;
            fn syscall(number: std::ffi::c_long, ...) -> std::ffi::c_long;
            fn ptrace(request: u32, ...) -> std::ffi::c_long;
            fn process_vm_writev(
                pid: c_int,
                local: *const IoVector,
                local_count: c_ulong,
                remote: *const IoVector,
                remote_count: c_ulong,
                flags: c_ulong,
            ) -> isize;
            fn kill(pid: c_int, signal: c_int) -> c_int;
            fn truncate(path: *const c_char, length: i64) -> c_int;
        }
        fn nonblocking_fd(fd: c_int) -> Result<(), String> {
            let flags = unsafe { fcntl(fd, 3) }; // F_GETFL; retain all existing flags.
            if flags < 0 {
                return Err(error("custody F_GETFL", std::io::Error::last_os_error()));
            }
            if unsafe { fcntl(fd, 4, flags | NONBLOCK) } < 0 {
                return Err(error(
                    "custody F_SETFL O_NONBLOCK",
                    std::io::Error::last_os_error(),
                ));
            }
            Ok(())
        }
        fn stream_pair() -> Result<(UnixStream, UnixStream), String> {
            let (a, b) = UnixStream::pair().map_err(|e| error("create bounded custody pipe", e))?;
            a.set_nonblocking(true)
                .map_err(|e| error("nonblocking custody pipe", e))?;
            b.set_nonblocking(true)
                .map_err(|e| error("nonblocking custody pipe", e))?;
            Ok((a, b))
        }
        fn bounded_pipe_write(writer: &mut impl Write, bytes: &[u8]) -> Result<(), String> {
            if bytes.len() > CUSTODY_BYTES {
                return Err("custody pipe output bound".into());
            }
            let started = Instant::now();
            let mut written = 0;
            while written < bytes.len() {
                if started.elapsed() >= PHASE {
                    return Err("custody pipe write deadline".into());
                }
                match writer.write(&bytes[written..]) {
                    Ok(0) => return Err("custody pipe write zero".into()),
                    Ok(count) => written += count,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => thread::yield_now(),
                    Err(e) => return Err(error("write bounded custody pipe", e)),
                }
            }
            Ok(())
        }
        fn bounded_pipe_read(
            reader: &mut impl Read,
            limit: usize,
            mut poll: impl FnMut() -> Result<Option<bool>, String>,
        ) -> Result<Vec<u8>, String> {
            let started = Instant::now();
            let mut bytes = Vec::new();
            let mut eof = false;
            let mut exit = None;
            let mut buffer = [0u8; 1024];
            while !eof || exit.is_none() {
                if started.elapsed() >= PHASE {
                    return Err("custody pipe/read-child deadline".into());
                }
                if !eof {
                    let remaining = (limit + 1 - bytes.len()).min(buffer.len());
                    match reader.read(&mut buffer[..remaining]) {
                        Ok(0) => eof = true,
                        Ok(count) => {
                            bytes.extend_from_slice(&buffer[..count]);
                            if bytes.len() > limit {
                                return Err("custody pipe input overflow".into());
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(e) => return Err(error("read bounded custody pipe", e)),
                    }
                }
                if exit.is_none() {
                    exit = poll()?;
                }
                if exit == Some(false) {
                    return Err("custody child exited nonzero".into());
                }
                thread::yield_now();
            }
            Ok(bytes)
        }
        #[derive(Default)]
        struct CustodyIo {
            stdout: Vec<u8>,
            stderr: Vec<u8>,
            stdout_seen: u64,
            stderr_seen: u64,
            stdout_eof: bool,
            stderr_eof: bool,
            exit: Option<bool>,
            first_failure: Option<String>,
        }
        impl CustodyIo {
            fn remember(&mut self, failure: String) {
                if self.first_failure.is_none() {
                    self.first_failure = Some(failure);
                }
            }
            // One nonblocking read per stream per turn keeps stdin and status polls
            // live even if a failed child floods one pipe. Only error-free EOF is EOF.
            fn drain(&mut self, reader: &mut impl Read, stderr: bool) -> bool {
                let mut buffer = [0u8; 1024];
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        if stderr {
                            self.stderr_eof = true;
                        } else {
                            self.stdout_eof = true;
                        }
                    }
                    Ok(count) => {
                        if stderr {
                            self.stderr_seen = self.stderr_seen.saturating_add(count as u64);
                        } else {
                            self.stdout_seen = self.stdout_seen.saturating_add(count as u64);
                        }
                        if stderr {
                            let keep = count.min(512 - self.stderr.len());
                            // An overflowing stdout must not erase the original
                            // stderr prefix. Evict only stdout's retained tail.
                            let evict = (self.stdout.len() + self.stderr.len() + keep)
                                .saturating_sub(CUSTODY_BYTES);
                            self.stdout.truncate(self.stdout.len() - evict);
                            self.stderr.extend_from_slice(&buffer[..keep]);
                        } else {
                            let available = CUSTODY_BYTES - self.stdout.len() - self.stderr.len();
                            self.stdout
                                .extend_from_slice(&buffer[..count.min(available)]);
                        }
                        if self.stdout_seen.saturating_add(self.stderr_seen) > CUSTODY_BYTES as u64
                        {
                            self.remember(
                                "custody stdout+stderr combined byte bound exceeded".into(),
                            );
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => {
                        self.remember(error(
                            if stderr {
                                "read actual custody stderr"
                            } else {
                                "read actual custody stdout"
                            },
                            e,
                        ));
                        return true; // Do not retry a failed descriptor.
                    }
                }
                false
            }
            fn finish(self) -> Result<Vec<CustodyChild>, String> {
                let parsed = serde_json::from_slice::<CustodyFrame>(&self.stdout);
                let child_failed = parsed
                    .as_ref()
                    .is_ok_and(|frame| frame.first_failure.is_some());
                let decoded = parsed
                    .map_err(|e| format!("custody child JSON: {e}"))
                    .and_then(|frame| frame.into_children(self.exit, 2));
                let original = match decoded {
                    Err(child) if child_failed => match self.first_failure {
                        Some(parent) => format!("{child}; parentPipeFirstFailure={parent}"),
                        None => child,
                    },
                    Err(child) => self.first_failure.unwrap_or(child),
                    Ok(children) => {
                        if let Some(parent) = self.first_failure {
                            parent
                        } else if !self.stdout_eof || !self.stderr_eof || self.exit != Some(true) {
                            "custody requires both actual pipe EOFs and reaped original exit zero"
                                .into()
                        } else if self.stderr_seen != 0 {
                            "custody child wrote unexpected stderr".into()
                        } else {
                            return Ok(children);
                        }
                    }
                };
                let mut hex = String::with_capacity(self.stderr.len() * 2);
                const HEX: &[u8; 16] = b"0123456789abcdef";
                for byte in self.stderr {
                    hex.push(HEX[(byte >> 4) as usize] as char);
                    hex.push(HEX[(byte & 15) as usize] as char);
                }
                Err(format!(
                    "{original}; stdoutObservedBytes={} stdoutEof={} stderrObservedBytes={} stderrEof={} childReaped={} stderrPrefixHex={hex}",
                    self.stdout_seen,
                    self.stdout_eof,
                    self.stderr_seen,
                    self.stderr_eof,
                    self.exit.is_some()
                ))
            }
        }

        struct CustodyEvidence {
            target: CustodyTarget,
            file: File,
            before: Vec<u8>,
            device: u64,
            inode: u64,
        }
        struct CustodyPending {
            child: Child,
            input: Option<ChildStdin>,
            output: ChildStdout,
            error: ChildStderr,
            deadline: Instant,
            receipt: confinement::Receipt,
            files: Vec<CustodyEvidence>,
            // High, fixed CLOEXEC duplicate slots avoid confusing post-exec pipe FD
            // reuse with inherited raw-evidence descriptors in the finite audit.
            descriptors: Vec<OwnedFd>,
            marker: Box<[u8; 64]>,
            _abstract_listener: UnixListener,
        }
        impl CustodyPending {
            fn spawn() -> Result<Self, String> {
                let deadline = Instant::now() + PHASE;
                for leaf in confinement::profile().write_leaves {
                    fs::DirBuilder::new()
                        .mode(0o700)
                        .create(Path::new(CAPTURE).join(leaf))
                        .map_err(|e| error("create production-profile private leaf", e))?;
                }
                let address = SocketAddr::from_abstract_name(CUSTODY_ABSTRACT_SOCKET)
                    .map_err(|e| error("fixed abstract custody address", e))?;
                let abstract_listener = UnixListener::bind_addr(&address)
                    .map_err(|e| error("bind parent abstract custody endpoint", e))?;
                let mut files = Vec::with_capacity(5);
                let mut descriptors = Vec::with_capacity(5);
                for (index, target) in CustodyTarget::ALL.into_iter().enumerate() {
                    let filename = if target == CustodyTarget::ReadyReport {
                        ".calibration-ready.json.next"
                    } else {
                        target.filename()
                    };
                    let mut file = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .custom_flags(NOFOLLOW | NONBLOCK)
                        .open(Path::new(CAPTURE).join(filename))
                        .map_err(|e| error("create calibration-only parent evidence target", e))?;
                    let before = if target == CustodyTarget::ReadyReport {
                        Vec::new()
                    } else {
                        b"CALIBRATION-ONLY parent-owned denial target; not official protocol or manifest evidence\n".to_vec()
                    };
                    file.write_all(&before)
                        .map_err(|e| error("write calibration-only parent target", e))?;
                    let metadata = file
                        .metadata()
                        .map_err(|e| error("parent target metadata", e))?;
                    let expected = CUSTODY_FD_BASE + index as i32;
                    let fd = unsafe { fcntl(file.as_raw_fd(), 1030, expected) }; // F_DUPFD_CLOEXEC.
                    if fd < 0 {
                        return Err(error(
                            "reserve actual parent FD audit slot",
                            std::io::Error::last_os_error(),
                        ));
                    }
                    let descriptor = unsafe { OwnedFd::from_raw_fd(fd) };
                    if fd != expected {
                        return Err("fixed parent FD audit slots already occupied".into());
                    }
                    descriptors.push(descriptor);
                    files.push(CustodyEvidence {
                        target,
                        file,
                        before,
                        device: metadata.dev(),
                        inode: metadata.ino(),
                    });
                }
                let mut command = Command::new(
                    std::env::current_exe().map_err(|e| error("custody current executable", e))?,
                );
                command.args(["--custody-child", "direct"]).env_clear();
                // The single actual production launcher verifies nondumpable parent,
                // exact policy/ABI, descriptor closure and child-only restrictions.
                let (mut child, receipt) = confinement::spawn(&mut command, Path::new(CAPTURE))
                    .map_err(|failure| {
                        format!("production custody launcher refused: {}", failure.error)
                    })?;
                receipt.validate_enforced(std::process::id())?;
                // Only the production launcher creates stdio. Its returned pipe
                // handles, not caller-provided sockets, are the actual child path.
                let input = child
                    .stdin
                    .take()
                    .ok_or("production custody stdin pipe missing")?;
                let output = child
                    .stdout
                    .take()
                    .ok_or("production custody stdout pipe missing")?;
                let error = child
                    .stderr
                    .take()
                    .ok_or("production custody stderr pipe missing")?;
                nonblocking_fd(input.as_raw_fd())?;
                nonblocking_fd(output.as_raw_fd())?;
                nonblocking_fd(error.as_raw_fd())?;
                Ok(Self {
                    child,
                    input: Some(input),
                    output,
                    error,
                    deadline,
                    receipt,
                    files,
                    descriptors,
                    marker: Box::new([0x53; 64]),
                    _abstract_listener: abstract_listener,
                })
            }
            fn publish_ready(&mut self, bytes: &[u8]) -> Result<(), String> {
                let evidence = self
                    .files
                    .iter_mut()
                    .find(|file| file.target == CustodyTarget::ReadyReport)
                    .ok_or("reserved ready evidence missing")?;
                evidence
                    .file
                    .write_all(bytes)
                    .map_err(|e| error("write protected parent readiness", e))?;
                evidence
                    .file
                    .sync_all()
                    .map_err(|e| error("sync protected parent readiness", e))?;
                fs::rename(
                    Path::new(CAPTURE).join(".calibration-ready.json.next"),
                    Path::new(CAPTURE).join("calibration-ready.json"),
                )
                .map_err(|e| error("publish protected parent readiness", e))?;
                evidence.before = bytes.to_vec();
                Ok(())
            }
            fn exchange(&mut self, frame: &[u8]) -> Result<Vec<CustodyChild>, String> {
                let mut observed = CustodyIo::default();
                let mut written = 0;
                let mut stdout_failed = false;
                let mut stderr_failed = false;
                let mut wait_failed = false;
                if frame.len() > 512 {
                    observed.remember("fixed custody start frame exceeds 512 bytes".into());
                    drop(self.input.take());
                }
                loop {
                    if Instant::now() >= self.deadline {
                        observed.remember("custody original phase deadline expired".into());
                        break;
                    }
                    if let Some(input) = &mut self.input {
                        match input.write(&frame[written..]) {
                            Ok(0) => {
                                observed
                                    .remember("actual custody stdin write returned zero".into());
                                drop(self.input.take());
                            }
                            Ok(count) => {
                                written += count;
                                if written == frame.len() {
                                    drop(self.input.take());
                                }
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                            Err(e) => {
                                observed.remember(error("write actual custody stdin", e));
                                drop(self.input.take()); // Broken pipe never skips stdout/stderr collection.
                            }
                        }
                    }
                    if !observed.stdout_eof && !stdout_failed {
                        stdout_failed = observed.drain(&mut self.output, false);
                    }
                    if !observed.stderr_eof && !stderr_failed {
                        stderr_failed = observed.drain(&mut self.error, true);
                    }
                    if observed.first_failure.is_some() {
                        drop(self.input.take());
                    }
                    if observed.exit.is_none() && !wait_failed {
                        match self.child.try_wait() {
                            Ok(status) => observed.exit = status.map(|status| status.success()),
                            Err(e) => {
                                observed.remember(error("poll actual custody child", e));
                                wait_failed = true;
                            }
                        }
                    }
                    if self.input.is_none()
                        && (observed.stdout_eof || stdout_failed)
                        && (observed.stderr_eof || stderr_failed)
                        && (observed.exit.is_some() || wait_failed)
                    {
                        break;
                    }
                    thread::yield_now();
                }
                drop(self.input.take());
                observed.finish()
            }
            fn finish(mut self) -> Result<CustodyReceipt, String> {
                let direct_original = identity(self.child.id());
                let evidence_fds: [i32; 5] =
                    std::array::from_fn(|index| self.descriptors[index].as_raw_fd());
                let start = CustodyStart {
                    schema_version: 1,
                    memory_address: self.marker.as_ptr() as usize as u64,
                    evidence_fds,
                    run: true,
                };
                let bytes = serde_json::to_vec(&start).map_err(|e| e.to_string())?;
                let children = self.exchange(&bytes)?;
                let direct_original = direct_original?;
                if children.first().map(|child| &child.original_process) != Some(&direct_original) {
                    return Err(
                        "custody direct child did not match original measured /proc identity"
                            .into(),
                    );
                }
                let mut files = Vec::with_capacity(5);
                for evidence in &mut self.files {
                    let held = evidence
                        .file
                        .metadata()
                        .map_err(|e| error("read held custody target identity", e))?;
                    let named =
                        fs::symlink_metadata(Path::new(CAPTURE).join(evidence.target.filename()));
                    evidence
                        .file
                        .seek(SeekFrom::Start(0))
                        .map_err(|e| error("seek original custody target", e))?;
                    let mut after = Vec::new();
                    (&mut evidence.file)
                        .take(16_385)
                        .read_to_end(&mut after)
                        .map_err(|e| error("read original custody target bytes", e))?;
                    let unchanged = held.dev() == evidence.device
                        && held.ino() == evidence.inode
                        && held.nlink() == 1
                        && after == evidence.before
                        && named.is_ok_and(|m| {
                            m.is_file()
                                && !m.file_type().is_symlink()
                                && m.dev() == evidence.device
                                && m.ino() == evidence.inode
                                && m.nlink() == 1
                        });
                    files.push(CustodyFile {
                        target: evidence.target,
                        device: evidence.device,
                        inode: evidence.inode,
                        bytes: evidence.before.len() as u64,
                        unchanged,
                    });
                }
                Ok(CustodyReceipt {
                    confinement: self.receipt,
                    files,
                    memory_unchanged: *self.marker == [0x53; 64],
                    children,
                })
            }
        }
        fn custody_result(
            probes: &mut Vec<CustodyProbe>,
            action: CustodyAction,
            target: Option<CustodyTarget>,
            result: i64,
        ) {
            let errno = if result < 0 {
                std::io::Error::last_os_error().raw_os_error()
            } else {
                None
            };
            probes.push(CustodyProbe {
                action,
                target,
                return_value: result,
                errno,
                readback: None,
            });
        }
        fn custody_io<T>(
            probes: &mut Vec<CustodyProbe>,
            action: CustodyAction,
            target: CustodyTarget,
            result: std::io::Result<T>,
        ) {
            match result {
                Ok(_) => probes.push(CustodyProbe {
                    action,
                    target: Some(target),
                    return_value: 0,
                    errno: None,
                    readback: None,
                }),
                Err(e) => probes.push(CustodyProbe {
                    action,
                    target: Some(target),
                    return_value: -1,
                    errno: e.raw_os_error(),
                    readback: None,
                }),
            }
        }
        fn custody_attacks(
            role: CustodyRole,
            recorder: u32,
            recorder_start: u64,
            start: &CustodyStart,
        ) -> Result<CustodyChild, String> {
            let original_process = identity(std::process::id())?;
            let mut probes = Vec::with_capacity(44);
            for (index, target) in CustodyTarget::ALL.into_iter().enumerate() {
                let result = unsafe { fcntl(start.evidence_fds[index], 1) }; // F_GETFD.
                custody_result(
                    &mut probes,
                    CustodyAction::InheritedFd,
                    Some(target),
                    i64::from(result),
                );
            }
            for target in CustodyTarget::ALL {
                let path = Path::new(CAPTURE).join(target.filename());
                match OpenOptions::new()
                    .write(true)
                    .custom_flags(NOFOLLOW | NONBLOCK)
                    .open(&path)
                {
                    Ok(mut file) => match file.write(&[0xa7]) {
                        Ok(count) => probes.push(CustodyProbe {
                            action: CustodyAction::Write,
                            target: Some(target),
                            return_value: count as i64,
                            errno: None,
                            readback: None,
                        }),
                        Err(e) => probes.push(CustodyProbe {
                            action: CustodyAction::Write,
                            target: Some(target),
                            return_value: -1,
                            errno: e.raw_os_error(),
                            readback: None,
                        }),
                    },
                    Err(e) => probes.push(CustodyProbe {
                        action: CustodyAction::Write,
                        target: Some(target),
                        return_value: -1,
                        errno: e.raw_os_error(),
                        readback: None,
                    }),
                }
                let cpath = CString::new(path.as_os_str().as_encoded_bytes())
                    .map_err(|_| "custody fixed path NUL")?;
                let result = unsafe { truncate(cpath.as_ptr(), 0) };
                custody_result(
                    &mut probes,
                    CustodyAction::Truncate,
                    Some(target),
                    i64::from(result),
                );
                let alternate = Path::new(CAPTURE).join("tmp").join(format!(
                    "calibration-custody-{:?}-{}",
                    role,
                    target.filename()
                ));
                custody_io(
                    &mut probes,
                    CustodyAction::Rename,
                    target,
                    fs::rename(&path, &alternate),
                );
                custody_io(
                    &mut probes,
                    CustodyAction::Unlink,
                    target,
                    fs::remove_file(&path),
                );
                custody_io(
                    &mut probes,
                    CustodyAction::Hardlink,
                    target,
                    fs::hard_link(&path, &alternate),
                );
            }
            let raw_fd = start.evidence_fds[0];
            custody_io(
                &mut probes,
                CustodyAction::ProcFd,
                CustodyTarget::ClientIntended,
                OpenOptions::new()
                    .write(true)
                    .custom_flags(NONBLOCK)
                    .open(format!("/proc/{recorder}/fd/{raw_fd}")),
            );
            let pidfd = unsafe { syscall(434, recorder as c_int, 0u32) }; // pidfd_open.
            custody_result(&mut probes, CustodyAction::PidfdOpen, None, pidfd as i64);
            if pidfd < 0 {
                return Err(format!(
                    "actual recorder pidfd_open failed: return {pidfd}, errno {:?}",
                    probes.last().and_then(|probe| probe.errno)
                ));
            }
            let pidfd = unsafe { OwnedFd::from_raw_fd(pidfd as c_int) };
            let result = unsafe { syscall(438, pidfd.as_raw_fd(), raw_fd, 0u32) }; // pidfd_getfd.
            custody_result(
                &mut probes,
                CustodyAction::PidfdGetfd,
                Some(CustodyTarget::ClientIntended),
                result as i64,
            );
            if result >= 0 {
                drop(unsafe { OwnedFd::from_raw_fd(result as c_int) });
            }
            let result = unsafe {
                syscall(
                    424,
                    pidfd.as_raw_fd(),
                    18 as c_int,
                    std::ptr::null::<c_void>(),
                    0u32,
                )
            }; // pidfd_send_signal SIGCONT.
            custody_result(&mut probes, CustodyAction::PidfdSignal, None, result as i64);
            let result = unsafe {
                ptrace(
                    0x4206,
                    recorder as c_int,
                    std::ptr::null_mut::<c_void>(),
                    std::ptr::null_mut::<c_void>(),
                )
            }; // PTRACE_SEIZE, no stop.
            custody_result(&mut probes, CustodyAction::Ptrace, None, result as i64);
            let mut replacement = 0xa7u8;
            let local = IoVector {
                base: (&mut replacement as *mut u8).cast(),
                length: 1,
            };
            let remote = IoVector {
                base: start.memory_address as usize as *mut c_void,
                length: 1,
            };
            let result = unsafe { process_vm_writev(recorder as c_int, &local, 1, &remote, 1, 0) };
            custody_result(
                &mut probes,
                CustodyAction::ProcessVmWrite,
                None,
                result as i64,
            );
            let result = unsafe { kill(recorder as c_int, 18) }; // Actual SIGCONT: harmless to a running recorder if wrongly allowed.
            custody_result(&mut probes, CustodyAction::Signal, None, i64::from(result));
            #[repr(C)]
            struct AbstractAddress {
                family: u16,
                path: [u8; 108],
            }
            let fd = unsafe { socket(1, 1 | NONBLOCK | 0x80000, 0) };
            if fd < 0 {
                return Err(error(
                    "abstract custody socket creation",
                    std::io::Error::last_os_error(),
                ));
            }
            let socket = unsafe { OwnedFd::from_raw_fd(fd) };
            let mut address = AbstractAddress {
                family: 1,
                path: [0; 108],
            };
            address.path[1..1 + CUSTODY_ABSTRACT_SOCKET.len()]
                .copy_from_slice(CUSTODY_ABSTRACT_SOCKET);
            let result = unsafe {
                connect(
                    socket.as_raw_fd(),
                    (&address as *const AbstractAddress).cast(),
                    (3 + CUSTODY_ABSTRACT_SOCKET.len()) as u32,
                )
            };
            custody_result(
                &mut probes,
                CustodyAction::AbstractUnix,
                None,
                i64::from(result),
            );
            for (target, leaf) in CustodyTarget::WRITE_LEAVES
                .into_iter()
                .zip(confinement::profile().write_leaves)
            {
                if target.filename() != leaf {
                    return Err("closed custody leaf enum differs from production profile".into());
                }
                let path = Path::new(CAPTURE).join(leaf).join(match role {
                    CustodyRole::Direct => "calibration-direct-write",
                    CustodyRole::InheritedSetsid => "calibration-inherited-write",
                });
                let mut file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .custom_flags(NOFOLLOW | NONBLOCK)
                    .open(path)
                    .map_err(|e| error("allowed private-leaf create", e))?;
                let result = file
                    .write(&[0x5a])
                    .map_err(|e| error("allowed private-leaf write", e))?;
                file.seek(SeekFrom::Start(0))
                    .map_err(|e| error("allowed private-leaf seek", e))?;
                let mut readback = [0u8; 2];
                if file
                    .read(&mut readback)
                    .map_err(|e| error("allowed private-leaf read", e))?
                    != 1
                    || readback[0] != 0x5a
                {
                    return Err("allowed private-leaf original readback mismatch".into());
                }
                probes.push(CustodyProbe {
                    action: CustodyAction::PrivateWrite,
                    target: Some(target),
                    return_value: result as i64,
                    errno: None,
                    readback: Some(readback[0]),
                });
            }
            Ok(CustodyChild {
                role,
                original_process,
                recorder_pid: recorder,
                recorder_start_time_ticks: recorder_start,
                probes,
            })
        }
        fn custody_child_cohort() -> Result<Vec<CustodyChild>, String> {
            let recorder = unsafe { getppid() };
            if recorder < 2 {
                return Err("custody direct child has no original recorder parent".into());
            }
            let own = identity(std::process::id())?;
            let raw = bounded_read(Path::new(&format!("/proc/{recorder}/stat")), 4096)?;
            let recorder_identity = process_stat(
                std::str::from_utf8(&raw).map_err(|_| "recorder stat encoding")?,
                own.executable_device,
                own.executable_inode,
            )?;
            if recorder_identity.pid != recorder as u32 {
                return Err("custody original recorder stat PID mismatch".into());
            }
            let mut input = unsafe { File::from_raw_fd(0) };
            let bytes = bounded_pipe_read(&mut input, 512, || Ok(Some(true)))?;
            let start: CustodyStart = serde_json::from_slice(&bytes)
                .map_err(|e| format!("fixed custody start frame: {e}"))?;
            if start.schema_version != 1
                || !start.run
                || start.memory_address == 0
                || start.memory_address > usize::MAX as u64
                || start.evidence_fds != std::array::from_fn(|index| CUSTODY_FD_BASE + index as i32)
            {
                return Err("invalid fixed custody start frame".into());
            }
            let direct = custody_attacks(
                CustodyRole::Direct,
                recorder as u32,
                recorder_identity.start_time_ticks,
                &start,
            )?;
            let (mut reader, mut writer) = stream_pair()?;
            let child = unsafe { fork() };
            if child < 0 {
                return Err(error(
                    "actual inherited custody fork",
                    std::io::Error::last_os_error(),
                ));
            }
            if child == 0 {
                drop(reader);
                let result = (|| {
                    if unsafe { setsid() } < 0 {
                        return Err(error(
                            "actual inherited custody setsid",
                            std::io::Error::last_os_error(),
                        ));
                    }
                    Ok(vec![custody_attacks(
                        CustodyRole::InheritedSetsid,
                        recorder as u32,
                        recorder_identity.start_time_ticks,
                        &start,
                    )?])
                })();
                let succeeded = result.is_ok();
                let frame = CustodyFrame::from_result(result);
                let sent = serde_json::to_vec(&frame)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| bounded_pipe_write(&mut writer, &bytes));
                unsafe { _exit(if succeeded && sent.is_ok() { 0 } else { 1 }) }
            }
            drop(writer);
            let mut child_exit = None;
            let inherited = bounded_pipe_read(&mut reader, CUSTODY_BYTES, || {
                let mut status = 0;
                let result = unsafe { waitpid(child, &mut status, 1) }; // WNOHANG; never unbounded wait.
                if result < 0 {
                    return Err(error(
                        "poll inherited custody child",
                        std::io::Error::last_os_error(),
                    ));
                }
                if result != 0 {
                    child_exit = Some(status & 0x7f == 0 && status >> 8 == 0);
                }
                Ok(child_exit.map(|_| true))
            })?;
            let mut inherited = CustodyFrame::decode(&inherited, child_exit, 1)?;
            let inherited = inherited.pop().ok_or("missing inherited custody child")?;
            if inherited.original_process.pid != child as u32 {
                return Err("actual fork custody PID mismatch".into());
            }
            Ok(vec![direct, inherited])
        }
        fn custody_child() -> Result<(), String> {
            for fd in [0, 1, 2] {
                nonblocking_fd(fd)?;
            }
            let frame = CustodyFrame::from_result(custody_child_cohort());
            let mut output = unsafe { File::from_raw_fd(1) };
            let bytes = serde_json::to_vec(&frame).map_err(|e| e.to_string())?;
            bounded_pipe_write(&mut output, &bytes)?;
            match frame.first_failure {
                Some(original) => Err(original),
                None => Ok(()),
            }
        }

        fn exhaust(report: &mut Report, inode: bool) -> Result<(), String> {
            let started = Instant::now();
            let root = Path::new(CAPTURE).join("exhaustion");
            fs::create_dir(&root).map_err(|e| error("create exhaustion directory", e))?;
            let mut observation = Exhaustion {
                bytes_written: 0,
                files_created: 0,
                terminal_errno: None,
                reached_enospc: false,
            };
            let result = if inode {
                let mut result = Err("inode bound reached without ENOSPC".into());
                for index in 0..16_384u32 {
                    if started.elapsed() >= PHASE {
                        result = Err("inode exhaustion phase expired".into());
                        break;
                    }
                    match OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .open(root.join(format!("{index:05}")))
                    {
                        Ok(_) => observation.files_created += 1,
                        Err(e) => {
                            observation.terminal_errno = e.raw_os_error();
                            observation.reached_enospc = exhaustion_reached(e.raw_os_error());
                            result = if observation.reached_enospc {
                                Ok(())
                            } else {
                                Err(error("inode exhaustion", e))
                            };
                            break;
                        }
                    }
                    thread::yield_now();
                }
                result
            } else {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(root.join("data"))
                    .map_err(|e| error("create storage exhaustion file", e))?;
                observation.files_created = 1;
                let buffer = [0x5au8; 65_536];
                let mut result = Err("512MiB storage bound reached without ENOSPC".into());
                // Never write more than 512MiB. Reserved report pages consume real
                // space, so this exact template must reach ENOSPC before the cap.
                while observation.bytes_written < 536_870_912 {
                    if started.elapsed() >= PHASE {
                        result = Err("storage exhaustion phase expired".into());
                        break;
                    }
                    let count =
                        (536_870_912 - observation.bytes_written).min(buffer.len() as u64) as usize;
                    match file.write(&buffer[..count]) {
                        Ok(0) => {
                            result = Err("storage exhaustion write returned zero".into());
                            break;
                        }
                        Ok(count) => observation.bytes_written += count as u64,
                        Err(e) => {
                            observation.terminal_errno = e.raw_os_error();
                            observation.reached_enospc = exhaustion_reached(e.raw_os_error());
                            result = if observation.reached_enospc {
                                Ok(())
                            } else {
                                Err(error("storage exhaustion", e))
                            };
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
            fs::DirBuilder::new()
                .mode(0o700)
                .create(CAPTURE)
                .map_err(|e| error("create exclusive capture directory", e))?;
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
                custody: None,
                first_failure: None,
                elapsed_ms: 0,
            };
            let mut child = None;
            let mut custody = None;
            let run = (|| {
                if descendant_case(case) {
                    let (handle, identity) = spawn_descendant(case, &report.original_process)?;
                    child = Some(handle);
                    report.children.push(identity);
                }
                if case == Case::BoundaryDenials {
                    custody = Some(CustodyPending::spawn()?);
                }
                report.elapsed_ms = started.elapsed().as_millis() as u64;
                let ready = report_bytes(&report)?;
                if let Some(pending) = &mut custody {
                    pending.publish_ready(&ready)?;
                } else {
                    publish_fresh("calibration-ready.json", &ready)?;
                }
                observer_gate()?;
                report.observer_gate_observed = true;
                if let Some(child) = &mut child {
                    if let Some(status) = child
                        .try_wait()
                        .map_err(|e| error("recheck observed descendant", e))?
                    {
                        return Err(format!(
                            "descendant exited before host observer gate: {status}"
                        ));
                    }
                }
                match case {
                    Case::BoundaryDenials => boundary_denials(&mut report)?,
                    Case::StorageExhaustion => exhaust(&mut report, false)?,
                    Case::InodeExhaustion => exhaust(&mut report, true)?,
                    _ => {}
                }
                if let Some(pending) = custody.take() {
                    report.custody = Some(pending.finish()?);
                }
                report.outcome = if case.forced() {
                    Outcome::AwaitingHostAction
                } else {
                    Outcome::Completed
                };
                report.elapsed_ms = started.elapsed().as_millis() as u64;
                report.validate_case(case)?;
                Ok::<(), String>(())
            })();
            if let Err(error) = &run {
                first_failure(&mut report, error.clone());
            }
            report.elapsed_ms = started.elapsed().as_millis() as u64;
            if let Err(publish_error) = output.publish(&report) {
                // Publication/cleanup must never replace the original first error.
                return Err(match run {
                    Err(original) => {
                        format!("{original}; report publication also failed: {publish_error}")
                    }
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
                let published =
                    ReservedReport::new().and_then(|mut output| output.publish(&report));
                return Err(match published {
                    Ok(()) => original,
                    Err(e) => format!("{original}; expiry report publication also failed: {e}"),
                });
            }
            Ok(())
        }
        #[cfg(test)]
        mod pipe_state_tests {
            use super::*;
            fn synthetic_child(pid: u32) -> CustodyChild {
                CustodyChild {
                    role: CustodyRole::Direct,
                    original_process: ProcessIdentity {
                        pid,
                        parent_pid: 2,
                        process_group: 1,
                        session: 1,
                        start_time_ticks: 10,
                        executable_device: 1,
                        executable_inode: 2,
                    },
                    recorder_pid: 2,
                    recorder_start_time_ticks: 9,
                    probes: Vec::new(),
                }
            }
            fn synthetic_io() -> CustodyIo {
                let frame = CustodyFrame {
                    children: vec![synthetic_child(3), synthetic_child(4)],
                    first_failure: None,
                };
                let stdout = serde_json::to_vec(&frame).unwrap();
                CustodyIo {
                    stdout_seen: stdout.len() as u64,
                    stdout,
                    stdout_eof: true,
                    stderr_eof: true,
                    exit: Some(true),
                    ..CustodyIo::default()
                }
            }
            #[test]
            fn synthetic_success_requires_both_eofs_and_reaped_original_exit_zero() {
                assert_eq!(synthetic_io().finish().unwrap().len(), 2);
                let mut missing_stderr_eof = synthetic_io();
                missing_stderr_eof.stderr_eof = false;
                assert!(
                    missing_stderr_eof
                        .finish()
                        .unwrap_err()
                        .contains("stderrEof=false")
                );
                let mut missing_stdout_eof = synthetic_io();
                missing_stdout_eof.stdout_eof = false;
                assert!(
                    missing_stdout_eof
                        .finish()
                        .unwrap_err()
                        .contains("stdoutEof=false")
                );
                let mut unreaped = synthetic_io();
                unreaped.exit = None;
                assert!(unreaped.finish().unwrap_err().contains("childReaped=false"));
                let mut nonzero = synthetic_io();
                nonzero.exit = Some(false);
                assert!(nonzero.finish().is_err());
            }
            #[test]
            fn synthetic_broken_pipe_keeps_original_child_failure_and_actual_stderr_prefix() {
                let stdout = serde_json::to_vec(&CustodyFrame::from_result(Err(
                    "original child syscall failure".into(),
                )))
                .unwrap();
                let mut observed = CustodyIo {
                    stdout_seen: stdout.len() as u64,
                    stdout,
                    stdout_eof: true,
                    stderr_eof: true,
                    exit: Some(false),
                    ..CustodyIo::default()
                };
                observed.remember("stdin broken pipe".into());
                observed.remember("later cleanup failure".into());
                let mut stderr = std::io::Cursor::new(b"first\n");
                assert!(!observed.drain(&mut stderr, true));
                let error = observed.finish().unwrap_err();
                assert!(error.starts_with("original child syscall failure"));
                assert!(error.contains("parentPipeFirstFailure=stdin broken pipe"));
                assert!(error.contains("stderrPrefixHex=66697273740a"));
                assert!(!error.contains("later cleanup failure"));
            }
            #[test]
            fn synthetic_combined_cap_retains_bounded_prefix_without_faking_eof() {
                let mut observed = CustodyIo::default();
                let mut stderr = std::io::Cursor::new(vec![b'e'; 513]);
                assert!(!observed.drain(&mut stderr, true));
                assert_eq!(observed.stderr.len(), 512);
                assert!(!observed.stderr_eof);
                let mut stdout = std::io::Cursor::new(vec![b'x'; CUSTODY_BYTES]);
                while !observed.stdout_eof {
                    assert!(!observed.drain(&mut stdout, false));
                }
                assert!(observed.stdout.len() + observed.stderr.len() <= CUSTODY_BYTES);
                assert!(observed.stdout_seen + observed.stderr_seen > CUSTODY_BYTES as u64);
                let error = observed.finish().unwrap_err();
                assert!(error.contains("combined byte bound exceeded"));
                assert!(error.contains("stderrEof=false"));
                assert!(error.contains("childReaped=false"));
                let mut reversed = CustodyIo::default();
                let mut stdout = std::io::Cursor::new(vec![b'x'; CUSTODY_BYTES]);
                while !reversed.stdout_eof {
                    assert!(!reversed.drain(&mut stdout, false));
                }
                let mut stderr = std::io::Cursor::new(vec![b'e'; 513]);
                assert!(!reversed.drain(&mut stderr, true));
                assert_eq!(reversed.stderr, vec![b'e'; 512]);
                assert!(reversed.stdout.len() + reversed.stderr.len() <= CUSTODY_BYTES);
                assert!(
                    reversed
                        .finish()
                        .unwrap_err()
                        .contains("stderrPrefixHex=656565")
                );
            }
        }

        pub(super) fn run(mode: Mode) -> Result<(), String> {
            require_identity()?;
            match mode {
                Mode::Peer(case) => peer(case),
                Mode::Descendant(case) => descendant(case),
                Mode::CustodyChild => custody_child(),
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
        fn argv(values: &[&str]) -> Vec<String> {
            values.iter().map(|v| (*v).into()).collect()
        }
        fn synthetic_report() -> Report {
            Report {
                schema_version: 1,
                scope: "PR174_RUST_CALIBRATION_PEER".into(),
                case: Case::Normal,
                outcome: Outcome::AwaitingObserver,
                original_process: ProcessIdentity {
                    pid: 2,
                    parent_pid: 1,
                    process_group: 2,
                    session: 2,
                    start_time_ticks: 9,
                    executable_device: 1,
                    executable_inode: 2,
                },
                children: Vec::new(),
                observer_gate_observed: false,
                probes: Vec::new(),
                exhaustion: None,
                custody: None,
                first_failure: None,
                elapsed_ms: 0,
            }
        }
        #[test]
        fn synthetic_closed_arguments_never_launch_a_peer() {
            for name in [
                "normal",
                "same-group",
                "setsid",
                "setpgid",
                "timeout",
                "kill",
                "workflow-step-cancel",
                "storage-exhaustion",
                "inode-exhaustion",
                "boundary-denials",
            ] {
                assert!(matches!(
                    arguments(&argv(&["--case", name])),
                    Ok(Mode::Peer(_))
                ));
            }
            for args in [
                vec![],
                vec!["--case"],
                vec!["--case", "normal", "extra"],
                vec!["--case=normal"],
                vec!["--case", "/bin/sh"],
                vec!["--output", "normal"],
                vec!["--descendant", "timeout"],
            ] {
                assert!(arguments(&argv(&args)).is_err());
            }
            assert_eq!(
                arguments(&argv(&["--custody-child", "direct"])),
                Ok(Mode::CustodyChild)
            );
            assert!(arguments(&argv(&["--custody-child", "setsid"])).is_err());
            assert_eq!(
                arguments(&argv(&["--descendant", "setsid"])),
                Ok(Mode::Descendant(Case::Setsid))
            );
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
            assert_eq!(
                report.first_failure.as_deref(),
                Some("original ENOSPC-related failure")
            );
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
            let stat = format!(
                "42 (peer ) unusual) S 7 42 9 {} 123 0\n",
                ["0"; 15].join(" ")
            );
            let identity = process_stat(&stat, 1, 2).unwrap();
            assert_eq!(
                (
                    identity.pid,
                    identity.parent_pid,
                    identity.process_group,
                    identity.session,
                    identity.start_time_ticks
                ),
                (42, 7, 42, 9, 123)
            );
            assert!(process_stat("42 (truncated", 1, 2).is_err());
            assert!(process_stat(&stat, 1, 0).is_err());
        }
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    let result = peer::run();
    #[cfg(not(target_os = "linux"))]
    let result: Result<(), String> = Err(
        "real containment calibration requires Linux; no emulation or synthetic fallback".into(),
    );
    if let Err(error) = result {
        eprintln!("Rust containment calibration peer: {error}");
        std::process::exit(1);
    }
}

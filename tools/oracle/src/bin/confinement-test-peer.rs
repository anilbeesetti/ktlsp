//! NONOFFICIAL native mechanics only: no JVM, parser, container, or calibration claim.
use ktlsp_oracle_replay::recorder::confinement::{abi, profile, spawn};
use serde_json::json;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{io::{Read, Write}, process::Child, sync::mpsc, time::{Duration, Instant}};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const ORIGINAL: &[u8] = b"parent evidence must remain intact\n";
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const LIMIT: u64 = 64 * 1024;

fn main() {
    if let Err(error) = run() {
        eprintln!("NONOFFICIAL confinement mechanics: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("entry") {
        fs::write(args.get(2).ok_or("missing entry marker")?, b"CHILD_ENTERED")?;
        return Ok(());
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    if matches!(args.get(1).map(String::as_str), Some("probe" | "descendant")) {
        return linux::probe(&args);
    }
    let mode = args.get(1).ok_or("missing driver mode")?;
    let root = PathBuf::from(args.get(2).ok_or("missing root")?);
    let capture = root.join("capture");
    private_dir(&capture)?;
    for leaf in profile().write_leaves {
        private_dir(&capture.join(leaf))?;
    }
    let marker = root.join("child-entry");
    if mode == "invalid-leaf" {
        fs::remove_dir(capture.join("tmp"))?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(&root, capture.join("tmp"))?;
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    if mode == "driver" && abi().is_ok_and(|value| value >= 6) {
        return linux::driver(&root, &capture);
    }
    let observed_abi = abi().ok();
    let mut command = Command::new(std::env::current_exe()?);
    command.arg("entry").arg(&marker);
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    match spawn(&mut command, &capture) {
        Ok((mut child, receipt)) => {
            let _ = child.kill();
            return Err(format!("unexpected launch on unsupported/invalid setup: {receipt:?}").into());
        }
        Err(failure) => {
            assert!(!marker.exists(), "refusal must happen before child entry");
            println!("{}", json!({
                "kind":"NONOFFICIAL_NATIVE_MECHANICS", "actualDenials":false,
                "outcome":"REFUSED_NO_CHILD_ENTRY", "observedAbi":observed_abi,
                "receipt":failure.receipt, "childEntered":false
            }));
        }
    }
    Ok(())
}

fn private_dir(path: &Path) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

// Pipe readers have an output cap and a channel deadline; neither read nor wait
// can hold the driver indefinitely, including an unexpectedly surviving descendant.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn bounded_output(child: &mut Child) -> Result<Value, Box<dyn std::error::Error>> {
    fn reader(stream: impl Read + Send + 'static) -> mpsc::Receiver<std::io::Result<Vec<u8>>> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = stream.take(LIMIT + 1).read_to_end(&mut bytes).map(|_| bytes);
            let _ = tx.send(result);
        });
        rx
    }
    drop(child.stdin.take());
    let stdout = reader(child.stdout.take().ok_or("missing stdout pipe")?);
    let stderr = reader(child.stderr.take().ok_or("missing stderr pipe")?);
    let deadline = Instant::now() + Duration::from_secs(6);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            return Err("peer process deadline".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let out = stdout.recv_timeout(deadline.saturating_duration_since(Instant::now()))??;
    let err = stderr.recv_timeout(deadline.saturating_duration_since(Instant::now()))??;
    if out.len() as u64 > LIMIT || err.len() as u64 > LIMIT {
        return Err("peer output limit".into());
    }
    if !status.success() {
        return Err(format!("peer exit {status}: {}", String::from_utf8_lossy(&err)).into());
    }
    Ok(serde_json::from_slice(&out)?)
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux {
    use super::*;
    use std::{
        os::{fd::{AsRawFd, FromRawFd}, linux::net::SocketAddrExt, unix::{net::{SocketAddr, UnixListener, UnixStream}, process::CommandExt}},
        sync::atomic::{AtomicUsize, Ordering},
    };

    unsafe extern "C" {
        fn syscall(number: i64, ...) -> i64;
        fn signal(number: i32, handler: usize) -> usize;
        fn fork() -> i32;
        fn waitpid(pid: i32, status: *mut i32, flags: i32) -> i32;
        fn kill(pid: i32, signal: i32) -> i32;
        fn setsid() -> i32;
        fn _exit(status: i32) -> !;
    }
    static SIGNALS: AtomicUsize = AtomicUsize::new(0);
    extern "C" fn benign_signal(_: i32) {
        SIGNALS.fetch_add(1, Ordering::Relaxed);
    }
    fn errno() -> i32 {
        std::io::Error::last_os_error().raw_os_error().unwrap_or(-1)
    }
    fn denied(results: &mut serde_json::Map<String, Value>, name: &str, value: i64) {
        let error = (value < 0).then(errno);
        results.insert(name.into(), json!({"return":value,"errno":error}));
        assert_eq!(value, -1, "{name} unexpectedly allowed");
        assert!(matches!(error, Some(1 | 13 | 18)), "{name}: unexpected errno {error:?}");
    }
    fn denied_io<T>(results: &mut serde_json::Map<String, Value>, name: &str, result: std::io::Result<T>) {
        match result {
            Ok(_) => panic!("{name} unexpectedly allowed"),
            Err(error) => {
                let number = error.raw_os_error();
                assert!(matches!(number, Some(1 | 13 | 18)), "{name}: {error}");
                results.insert(name.into(), json!({"return":-1,"errno":number}));
            }
        }
    }

    pub(super) fn driver(root: &Path, capture: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // Kernel deadlines also terminate an orphaned probe if its driver fails.
        unsafe { syscall(37, 12); }
        let evidence = capture.join("evidence.raw");
        fs::write(&evidence, ORIGINAL)?;
        let outside = root.join("outside.raw");
        fs::write(&outside, ORIGINAL)?;
        // Deliberately make a writable evidence descriptor inheritable. The actual
        // production pre-exec hook, not this test, must close it on exec.
        let evidence_fd = fs::OpenOptions::new().read(true).write(true).open(&evidence)?;
        assert_eq!(unsafe { syscall(72, evidence_fd.as_raw_fd(), 2, 0) }, 0);
        let mut target = Box::new([b'P'; 16]);
        let address = target.as_mut_ptr() as usize;
        assert_ne!(unsafe { signal(10, benign_signal as *const () as usize) }, usize::MAX);
        let socket_name = format!("ktlsp-confinement-{}", std::process::id());
        let socket_address = SocketAddr::from_abstract_name(socket_name.as_bytes())?;
        let listener = UnixListener::bind_addr(&socket_address)?;
        listener.set_nonblocking(true)?;
        let parent = std::process::id();
        let parent_uid = unsafe { syscall(102) };
        let mut command = Command::new(std::env::current_exe()?);
        command.arg("probe").arg(capture).arg(parent.to_string())
            .arg(evidence_fd.as_raw_fd().to_string()).arg(address.to_string())
            .arg(&socket_name).arg(&outside).arg(parent_uid.to_string());
        // The production launcher must replace even deliberately supplied raw
        // evidence stdio, not merely trust callers to configure fresh pipes.
        command.stdin(Stdio::from(evidence_fd.try_clone()?))
            .stdout(Stdio::from(evidence_fd.try_clone()?))
            .stderr(Stdio::from(evidence_fd.try_clone()?));
        let observed_abi = abi()?;
        let (mut child, receipt) = match spawn(&mut command, capture) {
            Ok(value) => value,
            Err(failure) => {
                assert!(!capture.join("tmp/child-entry").exists());
                println!("{}", json!({"kind":"NONOFFICIAL_NATIVE_MECHANICS", "actualDenials":false,
                    "outcome":"REFUSED_NO_CHILD_ENTRY", "observedAbi":observed_abi,
                    "receipt":failure.receipt, "childEntered":false}));
                return Ok(());
            }
        };
        let probes = bounded_output(&mut child)?;
        assert_eq!(fs::read(&evidence)?, ORIGINAL);
        assert_eq!(fs::read(&outside)?, ORIGINAL);
        assert_eq!(*target, [b'P'; 16]);
        assert_eq!(SIGNALS.load(Ordering::Relaxed), 0);
        assert_eq!(unsafe { kill(parent as i32, 0) }, 0);
        assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
        assert_eq!(unsafe { syscall(157, 3, 0, 0, 0, 0) }, 0);
        assert_eq!(receipt.parent_pid, parent);
        assert_eq!(receipt.parent_dumpable, Some(0));
        assert_eq!(receipt.landlock_abi, Some(observed_abi));
        assert_eq!(receipt.profile, profile());
        assert_eq!(receipt.hook_outcome, "ENFORCED_BEFORE_EXEC");
        println!("{}", json!({"kind":"NONOFFICIAL_NATIVE_MECHANICS", "actualDenials":true,
            "outcome":"ENFORCED", "observedAbi":observed_abi, "receipt":receipt,
            "parentAlive":true, "parentUid":parent_uid, "evidenceUnchanged":true, "parentMemoryUnchanged":true,
            "parentSignals":SIGNALS.load(Ordering::Relaxed), "probes":probes}));
        Ok(())
    }

    fn audit_fds() -> Result<Vec<i32>, Box<dyn std::error::Error>> {
        use std::os::unix::fs::FileTypeExt;
        for fd in 0..=2 {
            assert!(fs::metadata(format!("/proc/self/fd/{fd}"))?.file_type().is_fifo(),
                "stdio must be fresh pipes, never inherited evidence handles");
        }
        let mut enumeration = Vec::new();
        let mut leaked = Vec::new();
        let own_directory = PathBuf::from(format!("/proc/{}/fd", std::process::id()));
        for entry in fs::read_dir("/proc/self/fd")? {
            let entry = entry?;
            let fd: i32 = entry.file_name().to_string_lossy().parse()?;
            if fd <= 2 { continue; }
            if fs::read_link(entry.path())? == own_directory {
                enumeration.push(fd);
            } else {
                leaked.push(fd);
            }
        }
        assert_eq!(enumeration.len(), 1, "identify only the temporary enumeration FD");
        for fd in enumeration {
            assert_eq!(unsafe { syscall(72, fd, 1) }, -1);
            assert_eq!(errno(), 9, "enumeration FD must be closed");
        }
        assert!(leaked.is_empty(), "leaked nonstdio descriptors: {leaked:?}");
        Ok(leaked)
    }

    // SEIZE attaches without stopping the target. If unexpectedly allowed, the
    // short-lived fork helper exits without EXITKILL; the recorder stays alive.
    fn ptrace_probe(parent: i32) -> Result<i32, Box<dyn std::error::Error>> {
        let pid = unsafe { fork() };
        if pid < 0 { return Err(std::io::Error::last_os_error().into()); }
        if pid == 0 {
            let result = unsafe { syscall(101, 0x4206, parent, 0, 0) };
            let status = if result == -1 { errno() } else { 0 };
            unsafe { _exit(status) };
        }
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let mut status = 0;
            let result = unsafe { waitpid(pid, &mut status, 1) };
            if result == pid {
                assert_eq!(status & 0x7f, 0, "ptrace helper exited normally");
                return Ok((status >> 8) & 0xff);
            }
            if result < 0 { return Err(std::io::Error::last_os_error().into()); }
            if Instant::now() >= deadline {
                unsafe { kill(pid, 9); }
                return Err("ptrace helper deadline".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    pub(super) fn probe(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        unsafe { syscall(37, 4); }
        let descendant = args[1] == "descendant";
        let capture = PathBuf::from(&args[2]);
        let parent: i32 = args[3].parse()?;
        let parent_fd: i32 = args[4].parse()?;
        let address: usize = args[5].parse()?;
        let leaked = audit_fds()?;
        let evidence = capture.join("evidence.raw");
        let mut results = serde_json::Map::new();
        assert_eq!(unsafe { syscall(157, 39, 0, 0, 0, 0) }, 1, "actual no_new_privs");
        // This FD was deliberately inheritable in the recorder. If the hook
        // leaked it, an unexpected write still uses identical evidence bytes.
        let inherited_write = unsafe { syscall(18, parent_fd, ORIGINAL.as_ptr(), ORIGINAL.len(), 0) };
        let inherited_errno = errno();
        assert_eq!(inherited_write, -1, "inherited evidence FD must not survive exec");
        assert_eq!(inherited_errno, 9);
        results.insert("inheritedEvidenceFd".into(), json!({"return":inherited_write,"errno":inherited_errno}));
        let session = unsafe { syscall(124, 0) };
        if descendant { assert_eq!(session, std::process::id() as i64, "setsid survived exec"); }
        // All probes run as the same UID as the unrestricted recorder driver.
        let uid = unsafe { syscall(102) };
        assert_eq!(uid, args[8].parse::<i64>()?, "actual child and recorder UIDs must match");
        for leaf in profile().write_leaves {
            let path = capture.join(&leaf).join(if descendant { "descendant-write" } else { "normal-write" });
            fs::write(&path, b"allowed")?;
            let mut append = fs::OpenOptions::new().append(true).open(&path)?;
            append.write_all(b" append")?;
            assert_eq!(fs::read(&path)?, b"allowed append");
        }
        fs::write(capture.join("tmp/child-entry"), b"entered")?;
        denied_io(&mut results, "evidenceWrite", fs::OpenOptions::new().write(true).open(&evidence));
        denied_io(&mut results, "evidenceTruncate", fs::OpenOptions::new().write(true).truncate(true).open(&evidence));
        denied_io(&mut results, "evidenceUnlink", fs::remove_file(&evidence));
        denied_io(&mut results, "evidenceRename", fs::rename(&evidence, capture.join("tmp/moved-evidence")));
        denied_io(&mut results, "evidenceHardlink", fs::hard_link(&evidence, capture.join("tmp/evidence-link")));
        denied_io(&mut results, "captureRootCreate", fs::write(capture.join("forbidden-new-file"), b"bad"));
        denied_io(&mut results, "outsideWrite", fs::OpenOptions::new().write(true).open(&args[7]));
        let link = capture.join("tmp").join(if descendant { "descendant-symlink" } else { "evidence-symlink" });
        std::os::unix::fs::symlink(&evidence, &link)?;
        denied_io(&mut results, "symlinkEvidenceWrite", fs::OpenOptions::new().write(true).truncate(true).open(&link));
        denied_io(&mut results, "captureRootSymlink", std::os::unix::fs::symlink(&evidence, capture.join("forbidden-link")));
        denied_io(&mut results, "procParentFd", fs::OpenOptions::new().write(true).open(format!("/proc/{parent}/fd/{parent_fd}")));
        denied_io(&mut results, "procParentMem", fs::OpenOptions::new().write(true).open(format!("/proc/{parent}/mem")));
        let pidfd = unsafe { syscall(434, parent, 0) };
        assert!(pidfd >= 0, "pidfd_open must be available for the actual getfd probe");
        let pidfd = unsafe { fs::File::from_raw_fd(pidfd as i32) };
        let duplicated = unsafe { syscall(438, pidfd.as_raw_fd(), parent_fd, 0) };
        if duplicated >= 0 { drop(unsafe { fs::File::from_raw_fd(duplicated as i32) }); }
        denied(&mut results, "pidfdGetfd", duplicated);
        denied(&mut results, "pidfdSignalZero", unsafe {
            syscall(424, pidfd.as_raw_fd(), 0, std::ptr::null::<std::ffi::c_void>(), 0)
        });
        denied(&mut results, "pidfdSignalBenign", unsafe {
            syscall(424, pidfd.as_raw_fd(), 10, std::ptr::null::<std::ffi::c_void>(), 0)
        });
        let ptrace_error = ptrace_probe(parent)?;
        assert!(matches!(ptrace_error, 1 | 13), "ptrace unexpectedly allowed or failed differently: {ptrace_error}");
        results.insert("ptraceSeize".into(), json!({"return":-1,"errno":ptrace_error}));
        #[repr(C)]
        struct Iovec { base: *const u8, len: usize }
        let bytes = [b'P'; 16];
        let local = Iovec { base: bytes.as_ptr(), len: bytes.len() };
        let remote = Iovec { base: address as *const u8, len: bytes.len() };
        // Even an unexpected allow only writes identical bytes to a dedicated buffer.
        denied(&mut results, "processVmWritev", unsafe { syscall(311, parent, &local as *const Iovec, 1, &remote as *const Iovec, 1, 0) });
        denied(&mut results, "signalZero", unsafe { kill(parent, 0) } as i64);
        denied(&mut results, "signalBenign", unsafe { kill(parent, 10) } as i64);
        let socket = SocketAddr::from_abstract_name(args[6].as_bytes())?;
        denied_io(&mut results, "abstractUnixConnect", UnixStream::connect_addr(&socket));
        let mut report = json!({"sameUid":uid,"leakedFds":leaked,"denials":results,
            "noNewPrivs":true,"detachedSession":descendant,"sessionId":session,
            "pid":std::process::id(),"allowedWriteLeaves":profile().write_leaves});
        if !descendant {
            let mut command = Command::new(std::env::current_exe()?);
            command.arg("descendant").args(&args[2..]);
            command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
            // Command forks and execs; setsid detaches the descendant's session.
            unsafe {
                command.pre_exec(|| {
                    if setsid() == -1 { return Err(std::io::Error::last_os_error()); }
                    Ok(())
                });
            }
            let mut child = command.spawn()?;
            report["descendant"] = bounded_output(&mut child)?;
        }
        println!("{report}");
        Ok(())
    }

}

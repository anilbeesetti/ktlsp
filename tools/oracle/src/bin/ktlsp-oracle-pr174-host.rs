//! Optional Linux host controller. Protected original-job custody is a trust assumption;
//! copied receipts and synthetic peers never authenticate an official observation.
#[cfg(unix)]
mod host {
    use serde::{Deserialize, Serialize};
    use serde_json::{Value, json};
    use std::{
        collections::BTreeSet,
        fs::{self, File, OpenOptions},
        io::{ErrorKind, Read, Write},
        os::{
            fd::OwnedFd,
            unix::{
                fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
                net::UnixStream,
                process::ExitStatusExt,
            },
        },
        path::{Component, Path, PathBuf},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };

    const CONTRACT: &str = "65eb16c5bd73c99fe7fc000fddf0242bf6abab09283676612edf5ce9f285b61d";
    const BASELINE: &str = "b4fa25286a5fa0d7600b0b0420a7bcae6d520edc";
    const ROOT: &str = "/run/ktlsp-pr174";
    const INNER: &str = "/tmp/ktlsp-t045-pr174-host";
    const RELEASE: &str = "kotlin-server-263.6379.0";
    const ARCHIVE: &str = "ab8ca4455dc2fc5fe1a24db2bccc46c104254d2c465155c4251ee65df8f3f7cc";
    const LAUNCHER: &str = "adf02baec622b4fcd1e7db3ec96f7c7b3309d5ab59e170e60260d7511222878e";
    const JVM: &str = "ba231c43718732491a965b742b54a1f9206c2415c61502311c0172b88d6c79d5";
    const OBSERVER_BYTES: usize = 786_432;
    const CLEANUP_BYTES: usize = 262_144;
    const TERMINAL_BYTES: usize = 2_048;
    const INPUT_BYTES: usize = 16_384;
    const FIXTURES: [(&str, u64, &str); 3] = [
        (
            "declarations.kt",
            146,
            "fa935b144a49838fd9e603bfc95462bcf3701eeb19877501d451ca069ce9e233",
        ),
        (
            "error-node.kt",
            44,
            "c052b78e7603a88f3531f5b06f3a533f8df5394ce8ce9eeab3eb2b3ce3f4bd2c",
        ),
        (
            "script.json",
            2351,
            "930a87ea1fa9c3f6e6a6b6008d010dd9435082329ea8b897a42084d7369ab008",
        ),
    ];
    const MEMBERS: [(&str, &str); 8] = [
        (
            "kotlin-lsp.sh",
            "8d994fea993aa9f995e715369ef727cec15920f4502c1b326f3869fb2d1bdb43",
        ),
        ("bin/intellij-server", LAUNCHER),
        (
            "bin/intellij-server.vmoptions",
            "e2bf45f82006a58fdf5a5ae87d36f8a031d89b2c7eda743a29d6ca7a1381c1b4",
        ),
        (
            "jbr/bin/java",
            "eeeaadc9662c5faed35be8ccfdc90b74e1de37ad672d219673aff294276072fc",
        ),
        (
            "jbr/release",
            "5fb59d126d35d14775f7ab86e1c60e1624ff9e236f2f89f3752a869263f498bd",
        ),
        ("jbr/lib/server/libjvm.so", JVM),
        (
            "jbr/lib/libjli.so",
            "fce140d52f8945dfe0e16fa91313f9b5629b68fbeade937771951f1e69d49b7f",
        ),
        (
            "product-info.json",
            "d7ffe547c1529603396ba9243aec138488ea1a2f7838173bd028b1d4ae454de4",
        ),
    ];
    type Result<T> = std::result::Result<T, String>;
    include!("pr174/bootstrap.rs");
    include!("pr174/calibration.rs");
    include!("pr174/original.rs");
    include!("pr174/export.rs");
    include!("pr174/collect.rs");
    include!("pr174/storage.rs");
    include!("pr174/action.rs");
    include!("pr174/confinement.rs");
    include!("pr174/workflow.rs");
    mod materializer { include!("pr174/materializer.rs"); }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct Admission {
        schema_version: u8,
        scope: String,
        contract_sha256: String,
        root_authorization_sha256: String,
        calibration_sha256: String,
        nonce: String,
        run: OriginalRun,
        boot_id: String,
        daemon_id: String,
        preparation_deadline_ms: u64,
        image: String,
        image_id: String,
        host_binary_sha256: String,
        adapter_sha256: String,
        extraction_manifest_sha256: String,
        workflow_sha256: String,
        tools: Tools,
    }
    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct Tools {
        docker: PathBuf,
        systemctl: PathBuf,
        sha256sum: PathBuf,
        gh: PathBuf,
        mount: PathBuf,
        findmnt: PathBuf,
        stat: PathBuf,
        curl: PathBuf,
    }
    #[derive(Deserialize, Serialize, Clone)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct Binding {
        schema_version: u8,
        nonce: String,
        container_id: String,
        daemon_id: String,
        boot_id: String,
        preparation_deadline_ms: u64,
        armed_ms: Option<u64>,
        #[serde(skip)]
        uncertain: bool,
        supervisor_cgroup: String,
        supervisor_cgroup_device: u64,
        supervisor_cgroup_inode: u64,
        daemon_pid: u32,
        daemon_start: u64,
        curl: PathBuf,
        docker: PathBuf,
        systemctl: PathBuf,
    }
    #[derive(Deserialize, Serialize, Clone)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct ProcessIdentity {
        init_pid: u32,
        init_start: u64,
        cgroup_device: u64,
        cgroup_inode: u64,
        pid_namespace: String,
        cgroup: String,
    }

    fn hex(s: &str, len: usize) -> bool {
        s.len() == len
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
    fn digest(s: &str) -> bool {
        hex(s, 64) && s.bytes().any(|b| b != b'0')
    }
    fn safe_absolute(p: &Path) -> bool {
        p.is_absolute()
            && p.components()
                .all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
            && !p.as_os_str().as_encoded_bytes().contains(&b'\n')
    }
    fn directory(s: &str) -> Result<PathBuf> {
        let p = PathBuf::from(s);
        if p.parent() != Some(Path::new(ROOT))
            || !p
                .file_name()
                .and_then(|v| v.to_str())
                .is_some_and(|v| hex(v, 64))
        {
            return Err(
                "admission directory must be the fixed protected root plus a 64-hex nonce".into(),
            );
        }
        protected(&p, true)?;
        Ok(p)
    }
    fn protected(p: &Path, directory: bool) -> Result<()> {
        if !safe_absolute(p) {
            return Err("noncanonical protected path".into());
        }
        for a in p.ancestors() {
            let m = fs::symlink_metadata(a).map_err(|_| "protected path unavailable")?;
            if m.file_type().is_symlink() || m.uid() != 0 || m.mode() & 0o022 != 0 {
                return Err(
                    "protected path has symlink, non-root owner or writable ancestry".into(),
                );
            }
            if a != p && !m.is_dir() {
                return Err("protected ancestor is not a directory".into());
            }
        }
        let m = fs::symlink_metadata(p).map_err(|e| e.to_string())?;
        if (directory && !m.is_dir()) || (!directory && !m.is_file()) {
            return Err("protected input type".into());
        }
        Ok(())
    }
    fn read_file(p: &Path, cap: usize) -> Result<Vec<u8>> {
        let mut o = OpenOptions::new();
        o.read(true);
        #[cfg(target_os = "linux")]
        o.custom_flags(0x800 | 0x20000); // Nonblocking, no final symlink.
        #[cfg(not(target_os = "linux"))]
        o.custom_flags(4 | 0x100);
        let f = o.open(p).map_err(|e| e.to_string())?;
        if !f.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("input is not a regular file".into());
        }
        let mut bytes = Vec::new();
        f.take(cap as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > cap {
            return Err("input byte limit".into());
        }
        Ok(bytes)
    }
    fn load<T: for<'de> Deserialize<'de>>(p: &Path) -> Result<T> {
        protected(p, false)?;
        parse(&read_file(p, INPUT_BYTES)?)
    }
    // Reject duplicate keys even in daemon/API JSON subsequently projected into Value.
    struct Unique(Value);
    impl<'de> Deserialize<'de> for Unique {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = Unique;
                fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str("JSON without duplicate keys")
                }
                fn visit_bool<E: serde::de::Error>(
                    self,
                    v: bool,
                ) -> std::result::Result<Unique, E> {
                    Ok(Unique(json!(v)))
                }
                fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Unique, E> {
                    Ok(Unique(json!(v)))
                }
                fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Unique, E> {
                    Ok(Unique(json!(v)))
                }
                fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Unique, E> {
                    Ok(Unique(json!(v)))
                }
                fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<Unique, E> {
                    Ok(Unique(json!(v)))
                }
                fn visit_string<E: serde::de::Error>(
                    self,
                    v: String,
                ) -> std::result::Result<Unique, E> {
                    Ok(Unique(json!(v)))
                }
                fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Unique, E> {
                    Ok(Unique(Value::Null))
                }
                fn visit_none<E: serde::de::Error>(self) -> std::result::Result<Unique, E> {
                    Ok(Unique(Value::Null))
                }
                fn visit_seq<A: serde::de::SeqAccess<'de>>(
                    self,
                    mut a: A,
                ) -> std::result::Result<Unique, A::Error> {
                    let mut v = Vec::new();
                    while let Some(Unique(x)) = a.next_element()? {
                        v.push(x);
                    }
                    Ok(Unique(Value::Array(v)))
                }
                fn visit_map<A: serde::de::MapAccess<'de>>(
                    self,
                    mut a: A,
                ) -> std::result::Result<Unique, A::Error> {
                    let mut v = serde_json::Map::new();
                    while let Some((k, Unique(x))) = a.next_entry::<String, Unique>()? {
                        if v.insert(k, x).is_some() {
                            return Err(serde::de::Error::custom("duplicate JSON key"));
                        }
                    }
                    Ok(Unique(Value::Object(v)))
                }
            }
            d.deserialize_any(Visitor)
        }
    }
    fn parse<T: for<'de> Deserialize<'de>>(b: &[u8]) -> Result<T> {
        let Unique(v): Unique = serde_json::from_slice(b).map_err(|e| e.to_string())?;
        serde_json::from_value(v).map_err(|e| e.to_string())
    }
    fn mono_ms() -> Result<u64> {
        #[repr(C)]
        struct Timespec {
            sec: std::os::raw::c_long,
            nsec: std::os::raw::c_long,
        }
        unsafe extern "C" {
            fn clock_gettime(clock: i32, ts: *mut Timespec) -> i32;
        }
        let mut ts = Timespec { sec: 0, nsec: 0 };
        // Linux host mode agrees with systemd; macOS only runs synthetic peers.
        #[cfg(target_os = "macos")]
        const CLOCK_MONOTONIC: i32 = 6;
        #[cfg(not(target_os = "macos"))]
        const CLOCK_MONOTONIC: i32 = 1;
        if unsafe { clock_gettime(CLOCK_MONOTONIC, &mut ts) } != 0
            || ts.sec < 0
            || !(0..1_000_000_000).contains(&ts.nsec)
        {
            return Err("monotonic clock unavailable".into());
        }
        (ts.sec as u64)
            .checked_mul(1000)
            .and_then(|v| v.checked_add(ts.nsec as u64 / 1_000_000))
            .ok_or("clock overflow".into())
    }
    fn deadline(end_ms: u64) -> Result<Instant> {
        let remain = end_ms
            .checked_sub(mono_ms()?)
            .filter(|v| *v > 0)
            .ok_or("original deadline expired")?;
        Instant::now()
            .checked_add(Duration::from_millis(remain.min(10_000)))
            .ok_or("deadline overflow".into())
    }
    fn raw_hex(b: &[u8]) -> String {
        const H: &[u8] = b"0123456789abcdef";
        let mut s = String::with_capacity(b.len() * 2);
        for &v in b {
            s.push(H[(v >> 4) as usize] as char);
            s.push(H[(v & 15) as usize] as char);
        }
        s
    }
    #[derive(Default)]
    struct Output {
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        code: Option<i32>,
        signal: Option<i32>,
        eof: [bool; 2],
        timeout: bool,
        truncated: bool,
        reaped: bool,
    }
    impl Output {
        fn ok(&self) -> bool {
            self.code == Some(0)
                && self.signal.is_none()
                && self.eof == [true, true]
                && !self.timeout
                && !self.truncated
                && self.reaped
        }
        fn json(&self) -> Value {
            json!({"stdoutHex":raw_hex(&self.stdout),"stderrHex":raw_hex(&self.stderr),"exitCode":self.code,"signal":self.signal,"EOF":self.eof,"timeout":self.timeout,"truncated":self.truncated,"reaped":self.reaped})
        }
    }
    fn bounded_command(mut c: Command, cap: usize, end: Instant) -> Result<Output> {
        if Instant::now() >= end {
            return Err("command deadline expired before spawn".into());
        }
        let (mut out, child_out) = UnixStream::pair().map_err(|e| e.to_string())?;
        let (mut err, child_err) = UnixStream::pair().map_err(|e| e.to_string())?;
        out.set_nonblocking(true).map_err(|e| e.to_string())?;
        err.set_nonblocking(true).map_err(|e| e.to_string())?;
        c.stdin(Stdio::null())
            .stdout(Stdio::from(OwnedFd::from(child_out)))
            .stderr(Stdio::from(OwnedFd::from(child_err)));
        let mut child = c.spawn().map_err(|e| format!("command spawn: {e}"))?;
        drop(c);
        let mut r = Output::default();
        let mut buf = [0u8; 4096];
        loop {
            for (i, s) in [&mut out, &mut err].into_iter().enumerate() {
                if r.eof[i] {
                    continue;
                }
                match s.read(&mut buf) {
                    Ok(0) => r.eof[i] = true,
                    Ok(n) => {
                        let remaining = cap.saturating_sub(r.stdout.len() + r.stderr.len());
                        let keep = n.min(remaining);
                        if keep < n {
                            r.truncated = true;
                        }
                        if i == 0 {
                            r.stdout.extend_from_slice(&buf[..keep]);
                        } else {
                            r.stderr.extend_from_slice(&buf[..keep]);
                        }
                    }
                    Err(e)
                        if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => {}
                    Err(_) => {
                        r.truncated = true;
                        r.eof[i] = true;
                    }
                }
            }
            match child.try_wait() {
                Ok(Some(s)) => {
                    r.code = s.code();
                    r.signal = s.signal();
                    r.reaped = true;
                }
                Ok(None) => {}
                Err(_) => {
                    r.truncated = true;
                }
            }
            if r.reaped && r.eof == [true, true] {
                break;
            }
            if Instant::now() >= end {
                r.timeout = true;
                let _ = child.kill();
                if let Ok(Some(s)) = child.try_wait() {
                    r.code = s.code();
                    r.signal = s.signal();
                    r.reaped = true;
                }
                break;
            }
            std::thread::yield_now();
        }
        Ok(r)
    }
    struct Evidence {
        file: Option<File>,
        pending: Vec<u8>,
        used: usize,
        limit: usize,
        failed: bool,
    }
    impl Evidence {
        fn preflight() -> Self {
            Self {
                file: None,
                pending: Vec::new(),
                used: 0,
                limit: OBSERVER_BYTES,
                failed: false,
            }
        }
        fn commit(&mut self, dir: &Path) -> Result<()> {
            let p = dir.join("observer.evidence");
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(p)
                .map_err(|e| e.to_string())?;
            f.write_all(&self.pending)
                .and_then(|()| f.flush())
                .map_err(|e| e.to_string())?;
            self.pending.clear();
            self.file = Some(f);
            Ok(())
        }
        fn open(dir: &Path, cleanup: bool) -> Result<Self> {
            let p = dir.join(if cleanup {
                "cleanup.evidence"
            } else {
                "observer.evidence"
            });
            let mut o = OpenOptions::new();
            o.write(true).append(true).create(true).mode(0o600);
            #[cfg(target_os = "linux")]
            o.custom_flags(0x20000);
            #[cfg(not(target_os = "linux"))]
            o.custom_flags(0x100);
            let file = o.open(&p).map_err(|e| e.to_string())?;
            let m = file.metadata().map_err(|e| e.to_string())?;
            if !m.is_file() || m.uid() != 0 || m.mode() & 0o077 != 0 {
                return Err("evidence custody".into());
            }
            let limit = if cleanup {
                CLEANUP_BYTES
            } else {
                OBSERVER_BYTES
            };
            let mut used = usize::try_from(m.len()).map_err(|_| "evidence length overflow")?;
            if used > limit {
                return Err("persisted evidence over cap".into());
            }
            let failed = used != 0;
            if failed {
                used = limit;
            }
            Ok(Self {
                file: Some(file),
                pending: Vec::new(),
                used,
                limit,
                failed,
            })
        }
        fn available(&self) -> usize {
            self.limit.saturating_sub(self.used + TERMINAL_BYTES)
        }
        fn append(&mut self, v: &Value, terminal: bool) -> Result<()> {
            let mut b = serde_json::to_vec(v).map_err(|e| e.to_string())?;
            b.push(b'\n');
            let cap = if terminal {
                self.limit.saturating_sub(self.used)
            } else {
                self.available()
            };
            if b.len() > cap {
                self.failed = true;
                return Err("aggregate host evidence cap".into());
            }
            // Charge before write: partial writes and a restarted service never reset the cap.
            self.used += b.len();
            if let Some(f) = &mut self.file {
                if let Err(e) = f.write_all(&b).and_then(|()| f.flush()) {
                    self.failed = true;
                    return Err(e.to_string());
                }
            } else {
                self.pending.extend_from_slice(&b);
            }
            Ok(())
        }
        fn command(
            &mut self,
            program: &Path,
            args: &[String],
            end_ms: u64,
            token: Option<&str>,
        ) -> Result<Output> {
            let mut actual = args.to_vec();
            if program.file_name().is_some_and(|s| s == "docker") {
                actual.insert(0, "--host=unix:///run/docker.sock".into());
            }
            if token.is_some_and(str::is_empty) {
                return Err("empty credential refused".into());
            }
            let mut c = Command::new(program);
            c.args(&actual)
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("LANG", "C.UTF-8")
                .env("LC_ALL", "C.UTF-8");
            if let Some(t) = token {
                c.env("GH_TOKEN", t);
            }
            let r = bounded_command(c, self.available() / 4, deadline(end_ms)?);
            match r {
                Ok(mut o) => {
                    if token.is_some_and(|t| {
                        o.stdout.windows(t.len()).any(|s| s == t.as_bytes())
                            || o.stderr.windows(t.len()).any(|s| s == t.as_bytes())
                    }) {
                        o.stdout.clear();
                        o.stderr.clear();
                        o.truncated = true;
                    }
                    let saved=self.append(&json!({"kind":"command","program":program,"args":actual,"result":o.json()}),false);
                    if saved.is_err() {
                        self.failed = true;
                    }
                    Ok(o)
                }
                Err(e) => {
                    let _ = self.append(
                        &json!({"kind":"command-error","program":program,"args":actual,"error":e}),
                        false,
                    );
                    Err(e)
                }
            }
        }
        fn checked(&mut self, p: &Path, args: &[String], end: u64) -> Result<Vec<u8>> {
            let o = self.command(p, args, end, None)?;
            if !o.ok() {
                return Err("incomplete/nonzero host command".into());
            }
            Ok(o.stdout)
        }
    }
    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).into()).collect()
    }
    fn make_file(p: &Path, b: &[u8], mode: u32) -> Result<()> {
        make_file_with(p, b, mode, |f, bytes| {
            f.write_all(bytes).and_then(|()| f.sync_all())
        })
    }
    fn make_file_with(
        p: &Path,
        b: &[u8],
        mode: u32,
        write: impl FnOnce(&mut File, &[u8]) -> std::io::Result<()>,
    ) -> Result<()> {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(p)
            .map_err(|e| e.to_string())?;
        f.set_permissions(fs::Permissions::from_mode(mode)).map_err(|e| e.to_string())?;
        // Failed writes remain tombstones: removing a cancellation latch could permit START.
        write(&mut f, b).map_err(|e| e.to_string())
    }
    fn publish_complete_no_replace(staged: &Path, final_path: &Path) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let staged = std::ffi::CString::new(staged.as_os_str().as_encoded_bytes())
                .map_err(|_| "staged publication path contains NUL")?;
            let final_path = std::ffi::CString::new(final_path.as_os_str().as_encoded_bytes())
                .map_err(|_| "final publication path contains NUL")?;
            #[cfg(target_os = "linux")]
            let status = unsafe {
                libc::renameat2(
                    libc::AT_FDCWD,
                    staged.as_ptr(),
                    libc::AT_FDCWD,
                    final_path.as_ptr(),
                    libc::RENAME_NOREPLACE,
                )
            };
            #[cfg(target_os = "macos")]
            let status = unsafe {
                libc::renamex_np(staged.as_ptr(), final_path.as_ptr(), libc::RENAME_EXCL)
            };
            if status != 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(())
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (staged, final_path);
            Err("atomic no-replace publication unsupported on this OS".into())
        }
    }
    fn unit(nonce: &str, role: &str) -> String {
        format!("ktlsp-pr174-{nonce}-{role}")
    }
    fn admission(dir: &Path) -> Result<Admission> {
        let a: Admission = load(&dir.join("admission.json"))?;
        protected(&std::env::current_exe().map_err(|e| e.to_string())?, false)?;
        let r = &a.run;
        if a.schema_version != 1
            || !matches!(a.scope.as_str(), "PR174_HOST_EXECUTION_ADMITTED" | "PR174_RUST_CALIBRATION_ADMITTED")
            || a.contract_sha256 != CONTRACT
            || a.nonce != dir.file_name().and_then(|s| s.to_str()).unwrap_or("")
            || r.repository != "anilbeesetti/ktlsp"
            || r.event != "workflow_dispatch"
            || r.attempt != 1
            || r.run_id == 0
            || r.job_id == 0
            || r.suite_id == 0
            || !hex(&r.source_head, 40)
            || !hex(&r.source_tree, 40)
            || [
                &a.root_authorization_sha256,
                &a.calibration_sha256,
                &a.host_binary_sha256,
                &a.adapter_sha256,
                &a.extraction_manifest_sha256,
                &a.workflow_sha256,
            ]
            .iter()
            .any(|s| !digest(s))
            || !a.image_id.strip_prefix("sha256:").is_some_and(digest)
            || !a.image.rsplit_once("@sha256:").is_some_and(|(name, d)| {
                !name.is_empty()
                    && !name.starts_with('-')
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"/._:-".contains(&b))
                    && digest(d)
            })
            || a.daemon_id.is_empty()
            || a.daemon_id.len() > 128
            || a.preparation_deadline_ms <= mono_ms()?
        {
            return Err("protected admission scope/identity/deadline mismatch".into());
        }
        // These are delegated bootstrap inputs, not authentication supplied by a caller.
        for (p, name) in [
            (&a.tools.docker, "docker"),
            (&a.tools.systemctl, "systemctl"),
            (&a.tools.sha256sum, "sha256sum"),
            (&a.tools.gh, "gh"),
            (&a.tools.mount, "mount"),
            (&a.tools.findmnt, "findmnt"),
            (&a.tools.stat, "stat"),
            (&a.tools.curl, "curl"),
        ] {
            protected(p, false)?;
            if p.file_name().is_none_or(|v| v != name) {
                return Err("host tool role mismatch".into());
            }
            if !p
                .as_os_str()
                .as_encoded_bytes()
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(b))
                || fs::metadata(p).map_err(|e| e.to_string())?.mode() & 0o111 == 0
            {
                return Err("host tool path/executable custody".into());
            }
        }
        protected(&dir.join("inputs/tool/ktlsp-oracle-official-pr174"), false)?;
        if fs::metadata(dir.join("inputs/tool/ktlsp-oracle-official-pr174"))
            .map_err(|e| e.to_string())?
            .mode()
            & 0o111
            == 0
        {
            return Err("official adapter is not executable".into());
        }
        let token = dir.join("github.token");
        protected(&token, false)?;
        if fs::metadata(token).map_err(|e| e.to_string())?.mode() & 0o077 != 0 {
            return Err("credential custody".into());
        }
        Ok(a)
    }
    fn hash_file(_e: &mut Evidence, a: &Admission, p: &Path, expected: &str) -> Result<()> {
        if !digest(expected) || sha_path(p, a.preparation_deadline_ms)? != expected {
            return Err("materialized SHA256 mismatch".into());
        }
        Ok(())
    }
    fn gh(
        e: &mut Evidence,
        a: &Admission,
        token: &str,
        path: String,
        raw: bool,
    ) -> Result<Vec<u8>> {
        let mut argv = args(&["api", "--hostname", "github.com"]);
        if raw {
            argv.extend(args(&[
                "--header",
                "Accept: application/vnd.github.raw+json",
            ]));
        }
        argv.push(path);
        let o = e.command(&a.tools.gh, &argv, a.preparation_deadline_ms, Some(token))?;
        if !o.ok() {
            return Err("authenticated original GitHub API unavailable/incomplete".into());
        }
        Ok(o.stdout)
    }
    fn api_identity(run: &Value, job: &Value, commit: &Value, r: &OriginalRun) -> Result<()> {
        if run["id"] != r.run_id
            || run["run_attempt"] != 1
            || run["status"] != "in_progress"
            || !run.get("conclusion").is_some_and(Value::is_null)
            || run["event"] != "workflow_dispatch"
            || run["head_sha"] != r.source_head
            || run["check_suite_id"] != r.suite_id
            || run["repository"]["full_name"] != "anilbeesetti/ktlsp"
            || run["path"] != ".github/workflows/t045-pr174-official-symbols.yml"
            || job["id"] != r.job_id
            || job["status"] != "in_progress"
            || !job.get("conclusion").is_some_and(Value::is_null)
            || job["run_id"] != r.run_id
            || job["head_sha"] != r.source_head
            || commit["sha"] != r.source_head
            || commit["tree"]["sha"] != r.source_tree
            || !job["labels"]
                .as_array()
                .is_some_and(|v| v.contains(&json!("ubuntu-24.04")))
        {
            return Err("original GitHub run/job/commit identity mismatch".into());
        }
        Ok(())
    }
    fn daemon(e: &mut Evidence, tool: &Path, end: u64, expected: &str) -> Result<Value> {
        let b = e.checked(tool, &args(&["info", "--format", "{{json .}}"]), end)?;
        let v: Value = parse(&b)?;
        if v["ID"] != expected
            || v["OSType"] != "linux"
            || v["CgroupVersion"] != "2"
            || v["CgroupDriver"] != "systemd"
        {
            return Err("original daemon/cgroup identity mismatch".into());
        }
        let options = v["SecurityOptions"]
            .as_array()
            .ok_or("daemon security options missing")?;
        if !options.iter().any(|s| {
            s.as_str()
                .is_some_and(|s| s.contains("seccomp") && s.contains("builtin"))
        }) || !options
            .iter()
            .any(|s| s.as_str().is_some_and(|s| s.contains("apparmor")))
        {
            return Err("effective default seccomp/AppArmor unavailable".into());
        }
        Ok(v)
    }
    fn core_policy(e: &mut Evidence) -> Result<()> {
        for name in ["core_pattern", "core_uses_pid", "core_pipe_limit"] {
            let b = read_file(&Path::new("/proc/sys/kernel").join(name), INPUT_BYTES)?;
            e.append(
                &json!({"kind":"host-core-policy","name":name,"rawHex":raw_hex(&b)}),
                false,
            )?;
            if name == "core_pattern" && (b.first() == Some(&b'|') || b.is_empty()) {
                return Err("piped/missing host core policy requires separate admission".into());
            }
        }
        Ok(())
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Materialization {
        entries: Vec<Materialized>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Materialized {
        path: String,
        kind: String,
        bytes: u64,
        sha256: Option<String>,
        target: Option<String>,
    }
    fn materialization(dir: &Path, a: &Admission, e: &mut Evidence) -> Result<()> {
        let mp = dir.join("inputs/extraction-manifest.json");
        protected(&mp, false)?;
        hash_file(e, a, &mp, &a.extraction_manifest_sha256)?;
        // Streamed preparation inventory is bounded separately; it is not persisted host observation evidence.
        let manifest: Materialization = parse(&read_file(&mp, 262_144)?)?;
        if manifest.entries.len() > 1120 {
            return Err("extraction inventory limit".into());
        }
        let base = dir.join("inputs/distribution");
        protected(&base, true)?;
        let mut expected = BTreeSet::new();
        let mut regular = 0;
        let mut links = 0;
        for item in manifest.entries {
            let rel = Path::new(&item.path);
            if rel.is_absolute()
                || rel.components().any(|c| !matches!(c, Component::Normal(_)))
                || !item.path.starts_with(RELEASE)
                || !expected.insert(rel.to_path_buf())
            {
                return Err("extraction path/duplicate".into());
            }
            let p = base.join(rel);
            let m = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
            if m.uid() != 0 || (!m.file_type().is_symlink() && m.mode() & 0o022 != 0) {
                return Err("mutable extraction".into());
            }
            match item.kind.as_str() {
                "file" if m.is_file() && m.len() == item.bytes && item.target.is_none() => {
                    regular += 1;
                    hash_file(
                        e,
                        a,
                        &p,
                        item.sha256.as_deref().ok_or("file digest missing")?,
                    )?;
                }
                "symlink" if m.file_type().is_symlink() => {
                    links += 1;
                    let target = fs::read_link(&p).map_err(|e| e.to_string())?;
                    if target.to_str() != item.target.as_deref()
                        || !fs::canonicalize(&p)
                            .map_err(|e| e.to_string())?
                            .starts_with(&base)
                    {
                        return Err("extraction link escapes/substitution".into());
                    }
                }
                "directory" if m.is_dir() && item.target.is_none() => {}
                _ => return Err("extraction entry type/length".into()),
            }
        }
        if regular != 778 || links != 221 {
            return Err("frozen extraction inventory counts".into());
        }
        let mut actual = BTreeSet::new();
        let mut todo = vec![base.clone()];
        while let Some(p) = todo.pop() {
            if mono_ms()? >= a.preparation_deadline_ms {
                return Err("extraction verification deadline".into());
            }
            for ent in fs::read_dir(p).map_err(|e| e.to_string())? {
                let ent = ent.map_err(|e| e.to_string())?;
                let p = ent.path();
                if actual.len() >= 1120 {
                    return Err("unsealed extraction entry".into());
                }
                actual.insert(
                    p.strip_prefix(&base)
                        .map_err(|_| "extraction path")?
                        .to_path_buf(),
                );
                if ent.file_type().map_err(|e| e.to_string())?.is_dir() {
                    todo.push(p);
                }
            }
        }
        if actual != expected {
            return Err("materialization has missing/unsealed entries".into());
        }
        Ok(())
    }
    fn input_pins(dir: &Path, a: &Admission, e: &mut Evidence) -> Result<()> {
        hash_file(
            e,
            a,
            &std::env::current_exe().map_err(|e| e.to_string())?,
            &a.host_binary_sha256,
        )?;
        let archive = dir.join("inputs/archive.tar.gz");
        protected(&archive, false)?;
        if fs::metadata(&archive).map_err(|e| e.to_string())?.len() != 368_488_700 {
            return Err("archive byte length".into());
        }
        hash_file(e, a, &archive, ARCHIVE)?;
        for (p, d) in [
            ("root-authorization.json", &a.root_authorization_sha256),
            ("calibration.json", &a.calibration_sha256),
            ("workflow.yml", &a.workflow_sha256),
        ] {
            let p = dir.join(p);
            protected(&p, false)?;
            hash_file(e, a, &p, d)?;
        }
        materialization(dir, a, e)?;
        readback_critical(dir, a, e)?;
        Ok(())
    }
    fn preflight(dir: &Path, a: &Admission, e: &mut Evidence) -> Result<String> {
        let boot = String::from_utf8(read_file(
            Path::new("/proc/sys/kernel/random/boot_id"),
            128,
        )?)
        .map_err(|_| "boot ID encoding")?;
        if boot.trim() != a.boot_id {
            return Err("original host boot identity".into());
        }
        let self_cgroup =
            String::from_utf8(read_file(Path::new("/proc/self/cgroup"), INPUT_BYTES)?)
                .map_err(|_| "cgroup encoding")?;
        let cg = self_cgroup
            .trim()
            .strip_prefix("0::")
            .ok_or("unified host cgroup required")?;
        let wanted = format!("/system.slice/{}.service", unit(&a.nonce, "supervisor"));
        let self_stat = read_file(Path::new("/proc/self/stat"), 4096)?;
        if cg != wanted
            || !String::from_utf8_lossy(&read_file(Path::new("/proc/1/comm"), 128)?)
                .starts_with("systemd")
            || parent_pid(&self_stat)? != 1
        {
            return Err("supervisor is not the dedicated outside-step system service".into());
        }
        e.append(&json!({"kind":"outside-step-supervisor","statHex":raw_hex(&self_stat),"cgroup":cg,"pid1CommHex":raw_hex(&read_file(Path::new("/proc/1/comm"),128)?)}),false)?;
        core_policy(e)?;
        attest_mounts(dir, None, e)?;
        input_pins(dir, a, e)?;
        authenticate_original(dir, a, e)?;
        daemon(e, &a.tools.docker, a.preparation_deadline_ms, &a.daemon_id)?;
        let image: Value = parse(&e.checked(
            &a.tools.docker,
            &["image".into(), "inspect".into(), a.image.clone()],
            a.preparation_deadline_ms,
        )?)?;
        let image = image
            .as_array()
            .filter(|v| v.len() == 1)
            .and_then(|v| v.first())
            .ok_or("image inspect count")?;
        if image["Id"] != a.image_id
            || image["Os"] != "linux"
            || image["Architecture"] != "amd64"
            || !empty(&image["Config"]["Volumes"])
            || !image["RepoDigests"]
                .as_array()
                .is_some_and(|v| v.contains(&json!(a.image)))
        {
            return Err("pinned image identity/volumes".into());
        }
        e.append(&json!({"kind":"original-host","bootId":a.boot_id,"supervisorCgroup":cg,"osReleaseHex":raw_hex(&os_release()?),"kernelHex":raw_hex(&read_file(Path::new("/proc/sys/kernel/osrelease"),1024)?),"apiAuthenticated":true,"authority":"reviewed ROOT/bootstrap custody; receiver independently verifies original artifacts"}),false)?;
        measure_child_confinement(dir,a,e)?;
        Ok(cg.into())
    }
    fn empty(v: &Value) -> bool {
        v.is_null()
            || v.as_array().is_some_and(Vec::is_empty)
            || v.as_object().is_some_and(serde_json::Map::is_empty)
    }
    fn mkdir(p: &Path, mode: u32) -> Result<()> {
        fs::create_dir(p).map_err(|e| e.to_string())?;
        fs::set_permissions(p, fs::Permissions::from_mode(mode)).map_err(|e| e.to_string())
    }
    fn mount_state(dir: &Path, a: &Admission, e: &mut Evidence) -> Result<()> {
        let state = dir.join("state");
        mkdir(&state, 0o755)?;
        let target = state.to_str().ok_or("state path encoding")?;
        e.checked(
            &a.tools.mount,
            &args(&[
                "-t",
                "tmpfs",
                "-o",
                "size=536870912,nr_inodes=16384,mode=0755,uid=65532,gid=65532,nosuid,nodev",
                "tmpfs",
                target,
            ]),
            a.preparation_deadline_ms,
        )?;
        e.checked(
            &a.tools.mount,
            &args(&["--make-private", target]),
            a.preparation_deadline_ms,
        )?;
        let mounts: Value = parse(&e.checked(
            &a.tools.findmnt,
            &args(&[
                "--json",
                "--mountpoint",
                target,
                "--output",
                "TARGET,FSTYPE,OPTIONS,PROPAGATION",
            ]),
            a.preparation_deadline_ms,
        )?)?;
        let row = mounts["filesystems"]
            .as_array()
            .filter(|v| v.len() == 1)
            .and_then(|v| v.first())
            .ok_or("bounded mount inspect")?;
        let opts = row["options"].as_str().ok_or("mount options missing")?;
        if row["target"] != target
            || row["fstype"] != "tmpfs"
            || row["propagation"] != "private"
            || !opts.split(',').any(|v| v == "nosuid")
            || !opts.split(',').any(|v| v == "nodev")
        {
            return Err("effective bounded mount mismatch".into());
        }
        let stat = e.checked(
            &a.tools.stat,
            &args(&["--file-system", "--format=%S %b %c %T", target]),
            a.preparation_deadline_ms,
        )?;
        let fields = std::str::from_utf8(&stat)
            .map_err(|_| "stat encoding")?
            .split_whitespace()
            .collect::<Vec<_>>();
        if fields.len() != 4 || fields[3] != "tmpfs" {
            return Err("tmpfs capacity fields".into());
        }
        let block = fields[0].parse::<u64>().map_err(|_| "block size")?;
        let count = fields[1].parse::<u64>().map_err(|_| "block count")?;
        let inodes = fields[2].parse::<u64>().map_err(|_| "inode count")?;
        if block.checked_mul(count) != Some(536_870_912) || inodes != 16_384 {
            return Err("effective tmpfs capacity mismatch".into());
        }
        for p in ["fixture", "distribution", "tool", "preflight"] {
            mkdir(&state.join(p), 0o755)?;
        }
        side_json(e, &dir.join("state-mount.json"), &state_mount_identity(dir)?)?;
        Ok(())
    }

    fn launcher_argv() -> Vec<String> {
        vec![
            format!("{INNER}/distribution/{RELEASE}/bin/intellij-server"),
            "--stdio".into(),
            "--data-sharing=none".into(),
            "--region=apac".into(),
            format!("--system-path={INNER}/capture/system"),
        ]
    }
    fn recorder_argv() -> Vec<String> {
        let mut v = vec![format!("{INNER}/tool/ktlsp-oracle-official-pr174")];
        v.extend(adapter_argv());
        v
    }
    fn actual_argv(raw: &[u8], expected: &[String]) -> bool {
        if raw.last() != Some(&0) {
            return false;
        }
        let parts = raw[..raw.len() - 1].split(|b| *b == 0).collect::<Vec<_>>();
        parts.len() == expected.len() && parts.iter().zip(expected).all(|(a, b)| *a == b.as_bytes())
    }
    fn parent_pid(stat: &[u8]) -> Result<u32> {
        let s = std::str::from_utf8(stat).map_err(|_| "parent stat encoding")?;
        s.rsplit_once(") ")
            .and_then(|(_, tail)| tail.split_whitespace().nth(1))
            .ok_or("PPid missing")?
            .parse::<u32>()
            .map_err(|_| "PPid malformed".into())
    }
    fn exe_stamp(pid: u32) -> Result<(u64, u64)> {
        let m = fs::metadata(Path::new("/proc").join(pid.to_string()).join("exe"))
            .map_err(|e| e.to_string())?;
        Ok((m.dev(), m.ino()))
    }
    fn ancestry(
        stat: &[u8],
        recorder: bool,
        identity: &ProcessIdentity,
        adapter: (u64, u64),
        expected_recorder: Option<(u32, u64)>,
    ) -> Result<Value> {
        let parent = parent_pid(stat)?;
        let path = Path::new("/proc").join(parent.to_string());
        let before = read_file(&path.join("stat"), 4096)?;
        let stamp = process_start(&before)?;
        let exe = exe_stamp(parent)?;
        let init = read_file(
            &Path::new("/proc")
                .join(identity.init_pid.to_string())
                .join("stat"),
            4096,
        )?;
        if process_start(&init)? != (identity.init_pid, identity.init_start) {
            return Err("original init ancestry changed".into());
        }
        let cmdline = read_file(&path.join("cmdline"), INPUT_BYTES)?;
        let parent_proc = fs::metadata(&path).map_err(|e|e.to_string())?;
        if !recorder && (parent_proc.uid()!=0 || parent_proc.gid()!=0) {
            return Err("launcher parent lacks original nondumpable recorder proc custody".into());
        }
        if recorder {
            if stamp != (identity.init_pid, identity.init_start) {
                return Err("recorder is not original init child".into());
            }
        } else if exe != adapter
            || parent_pid(&before)? != identity.init_pid
            || expected_recorder.is_some_and(|p| p != stamp)
            || !actual_argv(&cmdline, &recorder_argv())
        {
            return Err("launcher has wrong original recorder parent/argv".into());
        }
        if process_start(&read_file(&path.join("stat"), 4096)?)? != stamp
            || exe_stamp(parent)? != exe
            || read_file(&path.join("cmdline"), INPUT_BYTES)? != cmdline
        {
            return Err("parent exec/reuse during observation".into());
        }
        Ok(
            json!({"parentHostPid":parent,"parentStarttime":stamp.1,"parentStatHex":raw_hex(&before),"parentCmdlineHex":raw_hex(&cmdline),"parentExeDevice":exe.0,"parentExeInode":exe.1,"initStatHex":raw_hex(&init)}),
        )
    }
    fn adapter_argv() -> Vec<String> {
        args(&[
            "--identity",
            &format!("{INNER}/preflight/identity.json"),
            "--preflight",
            &format!("{INNER}/preflight/host-preflight.json"),
            "--authorization",
            &format!("{INNER}/preflight/launch-authorization.json"),
            "--fixture-directory",
            &format!("{INNER}/fixture"),
        ])
    }
    fn environment() -> Vec<String> {
        let mut v = vec![
            "PATH=/usr/bin:/bin".into(),
            "LANG=C.UTF-8".into(),
            "LC_ALL=C.UTF-8".into(),
        ];
        for (k, p) in [
            ("HOME", "home"),
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_CACHE_HOME", "cache"),
            ("XDG_STATE_HOME", "state"),
            ("TMPDIR", "tmp"),
            ("TMP", "tmp"),
            ("TEMP", "tmp"),
        ] {
            v.push(format!("{k}={INNER}/private/{p}"));
        }
        v
    }
    fn mounts(dir: &Path) -> Vec<(PathBuf, String, bool)> {
        let mut v = vec![(dir.join("state"), INNER.into(), true)];
        for name in ["fixture", "distribution", "tool"] {
            v.push((
                dir.join("inputs").join(name),
                format!("{INNER}/{name}"),
                false,
            ));
        }
        v.push((dir.join("preflight"), format!("{INNER}/preflight"), false));
        v
    }
    fn create_argv(dir: &Path, a: &Admission) -> Result<Vec<String>> {
        let mut v = args(&[
            "create",
            "--platform=linux/amd64",
            "--init",
            "--pid=",
            "--ipc=private",
            "--cgroupns=private",
            "--user=65532:65532",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges=true",
            "--read-only",
            "--network=none",
            "--memory=4294967296",
            "--memory-swap=4294967296",
            "--pids-limit=256",
            "--cpus=2",
            "--restart=no",
            "--pull=never",
            "--no-healthcheck",
            "--log-driver=none",
            "--ulimit=core=0:0",
            "--shm-size=67108864",
        ]);
        for value in environment() {
            v.extend(["--env".into(), value]);
        }
        for (src, dst, rw) in mounts(dir) {
            v.extend([
                "--mount".into(),
                format!(
                    "type=bind,src={},dst={dst},bind-propagation=rprivate{}",
                    src.to_str().ok_or("mount path encoding")?,
                    if rw { "" } else { ",readonly" }
                ),
            ]);
        }
        v.extend([
            "--entrypoint".into(),
            entrypoint(a),
            a.image.clone(),
        ]);
        v.extend(entry_arguments(dir, a)?);
        Ok(v)
    }
    fn one_inspect(b: &[u8]) -> Result<Value> {
        let mut rows: Vec<Value> = parse(b)?;
        if rows.len() != 1 {
            return Err("inspect must contain one complete container".into());
        }
        Ok(rows.remove(0))
    }
    fn inspect(e: &mut Evidence, tool: &Path, cid: &str, end: u64) -> Result<Value> {
        one_inspect(&e.checked(tool, &args(&["container", "inspect", cid]), end)?)
    }
    fn config(v: &Value, dir: &Path, a: &Admission, cid: &str, created: bool) -> Result<()> {
        let h = &v["HostConfig"];
        let c = &v["Config"];
        let s = &v["State"];
        if v["Id"] != cid
            || v["Image"] != a.image_id
            || c["User"] != "65532:65532"
            || v["Path"] != entrypoint(a)
            || v["Args"] != json!(entry_arguments(dir, a)?)
            || c["Entrypoint"] != json!([entrypoint(a)])
            || c["Env"] != json!(environment())
            || !empty(&c["Volumes"])
            || c["Healthcheck"]["Test"] != json!(["NONE"])
            || h["Init"] != true
            || h["Privileged"] != false
            || h["ReadonlyRootfs"] != true
            || h["AutoRemove"] != false
            || h["PublishAllPorts"] != false
            || h["PidMode"] != ""
            || h["IpcMode"] != "private"
            || h["CgroupnsMode"] != "private"
            || h["NetworkMode"] != "none"
            || h["CgroupParent"] != ""
            || h["UTSMode"] != ""
            || h["UsernsMode"] != ""
            || h["Memory"] != 4_294_967_296u64
            || h["MemorySwap"] != 4_294_967_296u64
            || h["PidsLimit"] != 256
            || h["NanoCpus"] != 2_000_000_000u64
            || h["ShmSize"] != 67_108_864
            || h["RestartPolicy"]["Name"] != "no"
            || h["RestartPolicy"]["MaximumRetryCount"] != 0
            || h["LogConfig"]["Type"] != "none"
            || h["CapDrop"] != json!(["ALL"])
            || v["AppArmorProfile"] != "docker-default"
        {
            return Err("created container config/resource/security mismatch".into());
        }
        for key in [
            "CapAdd",
            "GroupAdd",
            "Devices",
            "DeviceRequests",
            "DeviceCgroupRules",
            "PortBindings",
            "VolumesFrom",
            "Links",
            "ExtraHosts",
            "Binds",
        ] {
            if !empty(&h[key]) {
                return Err("extra container authority/mount/network configuration".into());
            }
        }
        if h["SecurityOpt"] != json!(["no-new-privileges=true"])
            && h["SecurityOpt"] != json!(["no-new-privileges"])
        {
            return Err("effective no-new-privileges/default seccomp".into());
        }
        for (key, required) in [
            (
                "MaskedPaths",
                vec!["/proc/kcore", "/proc/keys", "/sys/firmware"],
            ),
            (
                "ReadonlyPaths",
                vec!["/proc/sys", "/proc/sysrq-trigger", "/proc/irq"],
            ),
        ] {
            let paths = h[key]
                .as_array()
                .ok_or("default system path policy missing")?;
            if required.iter().any(|p| !paths.contains(&json!(p))) {
                return Err("effective system path protection".into());
            }
        }
        let limits = h["Ulimits"]
            .as_array()
            .ok_or("container core limits missing")?;
        if limits.len() != 1
            || limits[0]["Name"] != "core"
            || limits[0]["Soft"] != 0
            || limits[0]["Hard"] != 0
        {
            return Err("configured core limits".into());
        }
        let rows = v["Mounts"].as_array().ok_or("mount list missing")?;
        let expected = mounts(dir);
        if rows.len() != expected.len() {
            return Err("extra/missing mount".into());
        }
        for (src, dst, rw) in expected {
            if rows
                .iter()
                .filter(|m| {
                    m["Type"] == "bind"
                        && m["Source"] == src.to_string_lossy().as_ref()
                        && m["Destination"] == dst
                        && m["RW"] == rw
                        && m["Propagation"] == "rprivate"
                })
                .count()
                != 1
            {
                return Err("effective bind mount mismatch".into());
            }
        }
        if created
            && (s["Status"] != "created"
                || s["Running"] != false
                || s["Pid"] != 0
                || s["Restarting"] != false
                || s["Dead"] != false)
        {
            return Err("container not in original created state".into());
        }
        Ok(())
    }
    fn side(e: &mut Evidence, p: &Path, b: &[u8], mode: u32, terminal: bool) -> Result<()> {
        let available = if terminal {
            e.limit.saturating_sub(e.used)
        } else {
            e.available()
        };
        if b.len() > available {
            e.failed = true;
            return Err("aggregate side-receipt cap".into());
        }
        e.used += b.len();
        if let Err(error) = make_file(p, b, mode) {
            e.failed = true;
            return Err(error);
        }
        Ok(())
    }
    fn side_json<T: Serialize>(e: &mut Evidence, p: &Path, v: &T) -> Result<()> {
        side(
            e,
            p,
            &serde_json::to_vec(v).map_err(|e| e.to_string())?,
            0o600,
            false,
        )
    }
    fn seal_handoff(dir: &Path, a: &Admission, cid: &str, e: &mut Evidence) -> Result<()> {
        let original = json!({"repository":a.run.repository,"event":a.run.event,"attempt":a.run.attempt,"runId":a.run.run_id,"jobId":a.run.job_id,"suiteId":a.run.suite_id,"sourceHead":a.run.source_head,"sourceTree":a.run.source_tree,"workflowSha256":a.workflow_sha256,"containerId":cid,"imageSha256":a.image.rsplit_once("@sha256:").ok_or("image digest")?.1,"nonce":a.nonce});
        let identity = json!({"schemaVersion":1,"contractSha256":CONTRACT,"sourceBaseline":BASELINE,"originalRun":original});
        let command = launcher_argv();
        let preflight = json!({"schemaVersion":1,"identity":identity,"pins":{"release":"kotlin-lsp/v263.6379.0","platform":"linux-amd64","archiveSha256":ARCHIVE,"launcherSha256":LAUNCHER,"scriptSha256":"930a87ea1fa9c3f6e6a6b6008d010dd9435082329ea8b897a42084d7369ab008","declarationsSha256":"fa935b144a49838fd9e603bfc95462bcf3701eeb19877501d451ca069ce9e233","errorNodeSha256":"c052b78e7603a88f3531f5b06f3a533f8df5394ce8ce9eeab3eb2b3ce3f4bd2c"},"command":command,"limits":{"phaseMs":60000,"sessionMs":300000,"cleanupMs":5000,"streamBytes":8388608,"stderrBytes":65536,"messages":1024},"outputDirectory":format!("{INNER}/capture")});
        for (name, value) in [
            ("identity.json", identity),
            ("host-preflight.json", preflight),
        ] {
            let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
            if bytes.len() > INPUT_BYTES {
                return Err("strict recorder preflight size".into());
            }
            side(e, &dir.join("preflight").join(name), &bytes, 0o444, false)?;
        }
        let hashed = e.checked(
            &a.tools.sha256sum,
            &[
                "--".into(),
                dir.join("preflight/host-preflight.json")
                    .to_string_lossy()
                    .into_owned(),
            ],
            a.preparation_deadline_ms,
        )?;
        let digest = std::str::from_utf8(&hashed)
            .map_err(|_| "preflight digest encoding")?
            .split_whitespace()
            .next()
            .filter(|s| digest(s))
            .ok_or("preflight digest missing")?;
        let authorization = json!({"schemaVersion":1,"rootAuthorizationSha256":a.root_authorization_sha256,"contractSha256":CONTRACT,"identity":original,"preflightSha256":digest,"bootId":a.boot_id,"daemonId":a.daemon_id,"authority":"protected original-job host START gate; adapter consistency only; receiver API authentication required"});
        side(
            e,
            &dir.join("preflight/launch-authorization.json"),
            &serde_json::to_vec(&authorization).map_err(|e| e.to_string())?,
            0o444,
            false,
        )?;
        fs::set_permissions(dir.join("preflight"), fs::Permissions::from_mode(0o555))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn service_path(nonce: &str, suffix: &str) -> PathBuf {
        Path::new("/run/systemd/system").join(format!("{}.{}", unit(nonce, "cleanup"), suffix))
    }
    fn install_cleanup(dir: &Path, a: &Admission, e: &mut Evidence) -> Result<()> {
        install_cleanup_at(
            dir,
            &a.nonce,
            &service_path(&a.nonce, "service"),
            &a.tools.systemctl,
            a.preparation_deadline_ms,
            e,
            |e, path, bytes| side(e, path, bytes, 0o600, false),
        )
    }
    fn install_cleanup_at(
        dir: &Path,
        nonce: &str,
        path: &Path,
        systemctl: &Path,
        end: u64,
        e: &mut Evidence,
        write: impl FnOnce(&mut Evidence, &Path, &[u8]) -> Result<()>,
    ) -> Result<()> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe = exe.to_str().ok_or("service executable path")?;
        if !exe
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
        {
            return Err("service executable path cannot be represented safely".into());
        }
        let text = format!(
            "[Unit]\nDescription=Bounded PR174 cleanup {}\nStartLimitIntervalSec=600s\nStartLimitBurst=1\n[Service]\nType=oneshot\nRemainAfterExit=yes\nUser=root\nGroup=root\nExecStart={exe} cleanup {}\nKillMode=control-group\nTimeoutStartSec=60s\nTimeoutStopSec=1s\nEnvironment=PATH=/usr/bin:/bin LANG=C.UTF-8 LC_ALL=C.UTF-8\nUnsetEnvironment=RUNNER_TRACKING_ID GH_TOKEN GITHUB_TOKEN JAVA_HOME JDK_HOME LD_LIBRARY_PATH\n",
            nonce,
            dir.display()
        );
        write(e, path, text.as_bytes())?;
        e.checked(systemctl, &args(&["daemon-reload"]), end)?;
        let shown = e.checked(
            systemctl,
            &args(&[
                "show",
                "--property=FragmentPath",
                "--property=User",
                "--property=KillMode",
                "--property=RemainAfterExit",
                "--property=StartLimitBurst",
                "--property=StartLimitIntervalUSec",
                "--property=DropInPaths",
                &format!("{}.service", unit(nonce, "cleanup")),
            ]),
            end,
        )?;
        let p = properties(&shown)?;
        if p.get("User").map(String::as_str) != Some("root")
            || p.get("KillMode").map(String::as_str) != Some("control-group")
            || p.get("RemainAfterExit").map(String::as_str) != Some("yes")
            || p.get("StartLimitBurst").map(String::as_str) != Some("1")
            || p.get("StartLimitIntervalUSec")
                .and_then(|v| systemctl_usec(v).ok())
                != Some(600_000_000)
            || p.get("DropInPaths").map(String::as_str) != Some("")
            || p.get("FragmentPath") != Some(&path.to_string_lossy().into_owned())
        {
            return Err("cleanup service policy not installed as expected".into());
        }
        Ok(())
    }
    fn arm(dir: &Path, a: &Admission, b: &mut Binding, e: &mut Evidence) -> Result<()> {
        let now = mono_ms()?;
        b.armed_ms = Some(now);
        // The original immutable deadline is a separate file, never a rewritten binding.
        side_json(
            e,
            &dir.join("armed.json"),
            &json!({"armedMs":now,"finalDeadlineMs":now.checked_add(600_000).ok_or("lifecycle overflow")?}),
        )?;
        let text = format!(
            "[Unit]\nDescription=PR174 bounded lifecycle {}\n[Timer]\nOnActiveSec=530s\nAccuracySec=1s\nRandomizedDelaySec=0\nUnit={}.service\n",
            a.nonce,
            unit(&a.nonce, "cleanup")
        );
        side(
            e,
            &service_path(&a.nonce, "timer"),
            text.as_bytes(),
            0o600,
            false,
        )?;
        e.checked(
            &a.tools.systemctl,
            &args(&["daemon-reload"]),
            a.preparation_deadline_ms,
        )?;
        e.checked(
            &a.tools.systemctl,
            &args(&["start", &format!("{}.timer", unit(&a.nonce, "cleanup"))]),
            a.preparation_deadline_ms,
        )?;
        let end = now
            .checked_add(540_000)
            .ok_or("timer overflow")?
            .min(a.preparation_deadline_ms);
        let shown = e.checked(
            &a.tools.systemctl,
            &args(&[
                "show",
                "--property=ActiveState",
                "--property=NextElapseUSecMonotonic",
                "--property=Unit",
                &format!("{}.timer", unit(&a.nonce, "cleanup")),
            ]),
            end,
        )?;
        let properties = properties(&shown)?;
        let trigger = properties
            .get("NextElapseUSecMonotonic")
            .ok_or_else(|| "timer trigger missing".to_string())
            .and_then(|s| systemctl_usec(s))?;
        if properties.get("ActiveState").map(String::as_str) != Some("active")
            || properties.get("Unit") != Some(&format!("{}.service", unit(&a.nonce, "cleanup")))
            || !watchdog_within_bound(trigger, now, mono_ms()?)
        {
            return Err("effective watchdog not armed within original bound".into());
        }
        Ok(())
    }
    fn properties(b: &[u8]) -> Result<std::collections::BTreeMap<String, String>> {
        let s = std::str::from_utf8(b).map_err(|_| "property encoding")?;
        let mut m = std::collections::BTreeMap::new();
        for line in s.lines() {
            let (k, v) = line.split_once('=').ok_or("malformed property")?;
            if m.insert(k.into(), v.into()).is_some() {
                return Err("duplicate property".into());
            }
        }
        Ok(m)
    }
    fn watchdog_within_bound(trigger_us: u64, armed_ms: u64, now_ms: u64) -> bool {
        let upper = armed_ms
            .checked_add(540_000)
            .and_then(|v| v.checked_mul(1000));
        // mono_ms truncates; require the trigger beyond the whole current millisecond.
        let current = now_ms.checked_add(1).and_then(|v| v.checked_mul(1000));
        upper
            .zip(current)
            .is_some_and(|(upper, current)| trigger_us > current && trigger_us <= upper)
    }
    fn request_cleanup(dir: &Path, b: &Binding, e: &mut Evidence, end: u64) -> Result<()> {
        if !dir.join("cleanup-requested").exists() {
            let _ = side(
                e,
                &dir.join("cleanup-requested"),
                b"cleanup requested\n",
                0o600,
                false,
            );
        }
        e.checked(
            &b.systemctl,
            &args(&[
                "start",
                "--no-block",
                &format!("{}.service", unit(&b.nonce, "cleanup")),
            ]),
            end,
        )?;
        Ok(())
    }
    fn cleanup_service_identity(b: &Binding) -> String {
        format!("{}:{}:{}\n", b.nonce, b.container_id, b.boot_id)
    }
    fn publish_cleanup_service(dir: &Path, b: &Binding, e: &mut Evidence) -> Result<()> {
        let next = dir.join("cleanup-service-ready.next");
        side(
            e,
            &next,
            cleanup_service_identity(b).as_bytes(),
            0o600,
            false,
        )?;
        publish_complete_no_replace(&next, &dir.join("cleanup-service-ready"))
    }
    fn cancellation_latched(dir: &Path) -> Result<bool> {
        match fs::symlink_metadata(dir.join("cleanup-requested")) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.to_string()),
        }
    }
    fn latch_cleanup(dir: &Path) -> Result<()> {
        if cancellation_latched(dir)? {
            Ok(())
        } else {
            make_file(&dir.join("cleanup-requested"), b"", 0o600)
        }
    }
    fn request_cleanup_external(dir: &Path, b: &Binding) -> Result<bool> {
        request_cleanup_external_with(dir, b, |path| protected(path, false))
    }
    fn request_cleanup_external_with(
        dir: &Path,
        b: &Binding,
        custody: impl FnOnce(&Path) -> Result<()>,
    ) -> Result<bool> {
        let latch = latch_cleanup(dir);
        let mut dispatched = false;
        dispatch_despite_latch_error(latch, || {
            let path = dir.join("cleanup-service-ready");
            match fs::symlink_metadata(&path) {
                Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error.to_string()),
                Ok(_) => {}
            }
            custody(&path)?;
            if read_file(&path, INPUT_BYTES)? != cleanup_service_identity(b).as_bytes() {
                return Err("cleanup service ownership marker mismatch".into());
            }
            let mut c = Command::new(&b.systemctl);
            c.args(args(&[
                "start",
                "--no-block",
                &format!("{}.service", unit(&b.nonce, "cleanup")),
            ]))
            .env_clear()
            .env("PATH", "/usr/bin:/bin");
            if !bounded_command(c, 0, deadline(lifecycle_end(b, mono_ms()?)?)?)?.ok() {
                return Err("cleanup request incomplete; closure UNKNOWN".into());
            }
            dispatched = true;
            Ok(())
        })?;
        Ok(dispatched)
    }
    fn process_start(b: &[u8]) -> Result<(u32, u64)> {
        let s = std::str::from_utf8(b).map_err(|_| "proc stat encoding")?;
        let (head, tail) = s.rsplit_once(") ").ok_or("proc stat comm delimiter")?;
        let pid = head
            .split_once(" (")
            .ok_or("proc stat PID")?
            .0
            .parse::<u32>()
            .map_err(|_| "proc PID")?;
        let fields: Vec<_> = tail.split_whitespace().collect();
        let start = fields
            .get(19)
            .ok_or("proc starttime missing")?
            .parse::<u64>()
            .map_err(|_| "proc starttime")?;
        Ok((pid, start))
    }
    fn original_init(pid: u32) -> Result<ProcessIdentity> {
        if pid <= 1 {
            return Err("invalid original init host PID".into());
        }
        let path = Path::new("/proc").join(pid.to_string());
        let b = read_file(&path.join("stat"), 4096)?;
        let (id, start) = process_start(&b)?;
        if id != pid {
            return Err("init PID substitution".into());
        }
        let ns = fs::read_link(path.join("ns/pid"))
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned();
        if fs::read_link("/proc/self/ns/pid")
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            == ns
        {
            return Err("host PID namespace shared".into());
        }
        let raw = read_file(&path.join("cgroup"), INPUT_BYTES)?;
        let cg = std::str::from_utf8(&raw)
            .map_err(|_| "cgroup encoding")?
            .trim()
            .strip_prefix("0::")
            .ok_or("original init cgroup")?;
        if !safe_absolute(Path::new(cg))
            || cg == "/"
            || process_start(&read_file(&path.join("stat"), 4096)?)? != (pid, start)
        {
            return Err("init identity/cgroup raced".into());
        }
        let object = fs::metadata(cgroup_path(cg)?).map_err(|e| e.to_string())?;
        Ok(ProcessIdentity {
            cgroup_device: object.dev(),
            cgroup_inode: object.ino(),
            init_pid: pid,
            init_start: start,
            pid_namespace: ns,
            cgroup: cg.into(),
        })
    }
    fn cgroup_path(relative: &str) -> Result<PathBuf> {
        if !safe_absolute(Path::new(relative)) || relative == "/" {
            return Err("unsafe original cgroup path".into());
        }
        Ok(Path::new("/sys/fs/cgroup").join(relative.trim_start_matches('/')))
    }

    struct Membership {
        pids: BTreeSet<u32>,
        empty: bool,
        raw: Vec<Value>,
    }
    fn queue_directory(seen: &mut usize, queued: usize, now: u64, end: u64) -> Result<()> {
        if now >= end || *seen + queued >= 256 {
            return Err("cgroup directory/deadline limit".into());
        }
        *seen += 1;
        Ok(())
    }
    fn members(relative: &str, end: u64, cap: usize) -> Result<Membership> {
        let mut todo = vec![cgroup_path(relative)?];
        let mut pids = BTreeSet::new();
        let mut threads = BTreeSet::new();
        let mut count = 0;
        let mut populated = false;
        let mut raw = Vec::new();
        let mut bytes = 0usize;
        while let Some(p) = todo.pop() {
            count += 1;
            if count > 256 || mono_ms()? >= end {
                return Err("cgroup inventory/deadline limit".into());
            }
            let object = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
            if !object.is_dir() {
                return Err("cgroup object type".into());
            }
            let events = read_file(
                &p.join("cgroup.events"),
                INPUT_BYTES.min(cap.saturating_sub(bytes)),
            )?;
            bytes += events.len();
            let procs = read_file(
                &p.join("cgroup.procs"),
                INPUT_BYTES.min(cap.saturating_sub(bytes)),
            )?;
            bytes += procs.len();
            let tasks = read_file(
                &p.join("cgroup.threads"),
                INPUT_BYTES.min(cap.saturating_sub(bytes)),
            )?;
            bytes += tasks.len();
            let text = std::str::from_utf8(&events).map_err(|_| "cgroup events encoding")?;
            if !text
                .lines()
                .any(|v| v == "populated 0" || v == "populated 1")
            {
                return Err("populated state missing".into());
            }
            populated |= text.lines().any(|v| v == "populated 1");
            for (b, set) in [(&procs, &mut pids), (&tasks, &mut threads)] {
                for id in std::str::from_utf8(b)
                    .map_err(|_| "cgroup members encoding")?
                    .split_whitespace()
                {
                    let pid = id.parse::<u32>().map_err(|_| "invalid cgroup PID")?;
                    if pid == 0 || (!set.contains(&pid) && set.len() >= 256) {
                        return Err("cgroup PID inventory limit".into());
                    }
                    set.insert(pid);
                }
            }
            raw.push(json!({"path":p,"device":object.dev(),"inode":object.ino(),"eventsHex":raw_hex(&events),"procsHex":raw_hex(&procs),"threadsHex":raw_hex(&tasks)}));
            for ent in fs::read_dir(&p).map_err(|e| e.to_string())? {
                if mono_ms()? >= end {
                    return Err("cgroup enumeration deadline".into());
                }
                let ent = ent.map_err(|e| e.to_string())?;
                let t = ent.file_type().map_err(|e| e.to_string())?;
                if t.is_symlink() {
                    return Err("cgroup symlink substitution".into());
                }
                if t.is_dir() {
                    let mut seen = count;
                    queue_directory(&mut seen, todo.len(), mono_ms()?, end)?;
                    bytes = bytes
                        .checked_add(ent.path().as_os_str().as_encoded_bytes().len())
                        .ok_or("cgroup byte overflow")?;
                    if bytes > cap {
                        return Err("cgroup inventory byte limit".into());
                    }
                    todo.push(ent.path());
                }
            }
            let after = fs::metadata(&p).map_err(|e| e.to_string())?;
            if (after.dev(), after.ino()) != (object.dev(), object.ino()) {
                return Err("cgroup object changed during scan".into());
            }
        }
        Ok(Membership {
            pids,
            empty: !populated && threads.is_empty(),
            raw,
        })
    }
    fn object_matches(actual: (u64, u64), expected: (u64, u64)) -> bool {
        actual == expected
    }
    fn cgroup_closed(
        relative: &str,
        device: u64,
        inode: u64,
        end: u64,
        e: &mut Evidence,
        stage: &str,
    ) -> Result<bool> {
        if mono_ms()? >= end {
            return Err("closure observation deadline".into());
        }
        let p = cgroup_path(relative)?;
        match fs::symlink_metadata(&p) {
            Err(error) if error.kind() == ErrorKind::NotFound => {
                e.append(&json!({"kind":"original-cgroup-closure","stage":stage,"path":relative,"originalDevice":device,"originalInode":inode,"observedMs":mono_ms()?,"attributableAbsent":true}),false)?;
                Ok(true)
            }
            Err(error) => Err(error.to_string()),
            Ok(object) => {
                if !object.is_dir()
                    || !object_matches((object.dev(), object.ino()), (device, inode))
                {
                    return Err("original cgroup recreated/substituted".into());
                }
                let observed = members(relative, end, e.available() / 4)?;
                let after = fs::metadata(&p).map_err(|e| e.to_string())?;
                if !object_matches((after.dev(), after.ino()), (device, inode)) {
                    return Err("original cgroup changed during closure".into());
                }
                let empty = observed.pids.is_empty() && observed.empty;
                e.append(&json!({"kind":"original-cgroup-closure","stage":stage,"path":relative,"originalDevice":device,"originalInode":inode,"observedMs":mono_ms()?,"attributableAbsent":false,"rows":observed.raw,"empty":empty}),false)?;
                Ok(empty)
            }
        }
    }
    fn core_zero(status: &str, limits: &str) -> bool {
        limits.lines().any(|l| {
            l.split_whitespace().collect::<Vec<_>>()
                == ["Max", "core", "file", "size", "0", "0", "bytes"]
        }) && status
            .lines()
            .any(|l| l.split_whitespace().collect::<Vec<_>>() == ["NoNewPrivs:", "1"])
            && status.lines().any(|l| {
                l.split_whitespace().collect::<Vec<_>>() == ["CapEff:", "0000000000000000"]
            })
            && status
                .lines()
                .any(|l| l.split_whitespace().collect::<Vec<_>>() == ["Seccomp:", "2"])
            && status.lines().any(|l| {
                l.split_whitespace().collect::<Vec<_>>()
                    == ["Uid:", "65532", "65532", "65532", "65532"]
            })
            && status.lines().any(|l| {
                l.split_whitespace().collect::<Vec<_>>()
                    == ["Gid:", "65532", "65532", "65532", "65532"]
            })
    }
    fn mapped_jvm(maps: &str, dev: u64, ino: u64) -> Result<bool> {
        let wanted = format!("{INNER}/distribution/{RELEASE}/jbr/lib/server/libjvm.so");
        let mut found = false;
        let major = ((dev >> 8) & 0xfff) | ((dev >> 32) & 0xfffff000);
        let minor = (dev & 0xff) | ((dev >> 12) & 0xffffff00);
        for line in maps.lines() {
            let f: Vec<_> = line.split_whitespace().collect();
            if f.iter().any(|s| s.contains("libjvm.so")) {
                if f.len() != 6 || f[5] != wanted || f[4].parse::<u64>().ok() != Some(ino) {
                    return Err("deleted/substituted JVM mapping".into());
                }
                let (a, b) = f[3].split_once(':').ok_or("mapped device missing")?;
                if u64::from_str_radix(a, 16).ok() != Some(major)
                    || u64::from_str_radix(b, 16).ok() != Some(minor)
                {
                    return Err("mapped JVM device mismatch".into());
                }
                found = true;
            }
        }
        Ok(found)
    }
    #[derive(Default)]
    struct Attestation {
        recorder: Option<(u32, u64)>,
        launcher: Option<(u32, u64)>,
        jvm: bool,
    }

    fn attest(
        dir: &Path,
        identity: &ProcessIdentity,
        end: u64,
        e: &mut Evidence,
        seen: &mut Attestation,
    ) -> Result<()> {
        let pids = members(&identity.cgroup, end, e.available() / 4)?.pids;
        let adapter = fs::metadata(dir.join("inputs/tool/ktlsp-oracle-official-pr174"))
            .map_err(|e| e.to_string())?;
        let launcher = fs::metadata(
            dir.join("inputs/distribution")
                .join(RELEASE)
                .join("bin/intellij-server"),
        )
        .map_err(|e| e.to_string())?;
        let jvm = fs::metadata(
            dir.join("inputs/distribution")
                .join(RELEASE)
                .join("jbr/lib/server/libjvm.so"),
        )
        .map_err(|e| e.to_string())?;
        for pid in pids {
            if mono_ms()? >= end {
                return Err("process observation deadline".into());
            }
            let p = Path::new("/proc").join(pid.to_string());
            let exe = match exe_stamp(pid) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let recorder = exe == (adapter.dev(), adapter.ino());
            let official = exe == (launcher.dev(), launcher.ino());
            if !recorder && !official {
                continue;
            }
            let proc_custody = fs::metadata(&p).map_err(|e|e.to_string())?;
            // Setup can precede the production hook. Only policy-ready recorder observations qualify;
            // missing such an original observation at normal exit still fails runtime attestation.
            if recorder && (proc_custody.uid()!=0 || proc_custody.gid()!=0) { continue; }
            let stat = read_file(&p.join("stat"), 4096)?;
            let stamp = process_start(&stat)?;
            if stamp.0 != pid {
                return Err("process PID changed".into());
            }
            let previous = if recorder {
                seen.recorder
            } else {
                seen.launcher
            };
            if previous.is_some_and(|v| v != stamp) {
                return Err("original recorder/launcher identity substituted".into());
            }
            let cmdline = read_file(&p.join("cmdline"), INPUT_BYTES)?;
            let expected = if recorder {
                recorder_argv()
            } else {
                launcher_argv()
            };
            if !actual_argv(&cmdline, &expected) {
                return Err("actual original process argv mismatch".into());
            }
            let parents = ancestry(
                &stat,
                recorder,
                identity,
                (adapter.dev(), adapter.ino()),
                seen.recorder,
            )?;
            let status = read_file(&p.join("status"), INPUT_BYTES)?;
            let limits = read_file(&p.join("limits"), INPUT_BYTES)?;
            let ns = fs::read_link(p.join("ns/pid"))
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .into_owned();
            let cg = read_file(&p.join("cgroup"), INPUT_BYTES)?;
            let nspid = std::str::from_utf8(&status)
                .map_err(|_| "status encoding")?
                .lines()
                .find(|l| l.starts_with("NSpid:"))
                .ok_or("NSpid missing")?
                .split_whitespace()
                .skip(1)
                .map(str::parse::<u32>)
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|_| "NSpid malformed")?;
            if ns != identity.pid_namespace
                || nspid.len() < 2
                || nspid.first() != Some(&pid)
                || nspid.contains(&0)
                || !core_zero(
                    std::str::from_utf8(&status).map_err(|_| "status encoding")?,
                    std::str::from_utf8(&limits).map_err(|_| "limits encoding")?,
                )
                || std::str::from_utf8(&cg)
                    .map_err(|_| "cgroup encoding")?
                    .trim()
                    != format!("0::{}", identity.cgroup)
            {
                return Err("original process security/core/namespace mismatch".into());
            }
            let maps = if official && !seen.jvm {
                read_file(&p.join("maps"), e.available() / 8)?
            } else {
                Vec::new()
            };
            let mapped = official
                && !seen.jvm
                && mapped_jvm(
                    std::str::from_utf8(&maps).map_err(|_| "maps encoding")?,
                    jvm.dev(),
                    jvm.ino(),
                )?;
            if mapped {
                let actual = fs::metadata(
                    p.join("root")
                        .join(INNER.trim_start_matches('/'))
                        .join("distribution")
                        .join(RELEASE)
                        .join("jbr/lib/server/libjvm.so"),
                )
                .map_err(|e| e.to_string())?;
                if (actual.dev(), actual.ino()) != (jvm.dev(), jvm.ino()) {
                    return Err("namespace-root JVM file differs".into());
                }
            }
            let after = read_file(&p.join("stat"), 4096)?;
            if process_start(&after)? != stamp
                || parent_pid(&after)? != parent_pid(&stat)?
                || exe_stamp(pid)? != exe
                || read_file(&p.join("cmdline"), INPUT_BYTES)? != cmdline
            {
                return Err("process exec/ancestry changed during observation".into());
            }
            if previous.is_none() || mapped {
                e.append(&json!({"kind":"original-process","role":if recorder {"recorder"}else{"launcher"},"hostPid":pid,"starttime":stamp.1,"namespace":ns,"statHex":raw_hex(&stat),"cmdlineHex":raw_hex(&cmdline),"ancestry":parents,"statusHex":raw_hex(&status),"limitsHex":raw_hex(&limits),"cgroupHex":raw_hex(&cg),"mapsHex":raw_hex(&maps),"pinnedLibjvmMapped":mapped,"exeDevice":exe.0,"exeInode":exe.1,"procDirectoryUid":proc_custody.uid(),"procDirectoryGid":proc_custody.gid(),"sealedExecutableDevice":if recorder {adapter.dev()}else{launcher.dev()},"sealedExecutableInode":if recorder {adapter.ino()}else{launcher.ino()},"sealedJvmDevice":jvm.dev(),"sealedJvmInode":jvm.ino(),"namespaceJvmDevice":if mapped {Some(jvm.dev())}else{None},"namespaceJvmInode":if mapped {Some(jvm.ino())}else{None}}),false)?;
            }
            if recorder {
                seen.recorder = Some(stamp);
            } else {
                seen.launcher = Some(stamp);
            }
            seen.jvm |= mapped;
        }
        Ok(())
    }
    fn actual_resources(identity: &ProcessIdentity, e: &mut Evidence) -> Result<()> {
        let root = cgroup_path(&identity.cgroup)?;
        for (name, expected) in [
            ("memory.max", "4294967296"),
            ("memory.swap.max", "0"),
            ("pids.max", "256"),
            ("cpu.max", "200000 100000"),
        ] {
            let b = read_file(&root.join(name), 1024)?;
            if std::str::from_utf8(&b)
                .map_err(|_| "resource encoding")?
                .trim()
                != expected
            {
                return Err("effective cgroup resource bound mismatch".into());
            }
            e.append(
                &json!({"kind":"effective-cgroup-resource","name":name,"rawHex":raw_hex(&b)}),
                false,
            )?;
        }
        for name in ["pid", "ipc", "net", "cgroup"] {
            let actual = fs::read_link(
                Path::new("/proc")
                    .join(identity.init_pid.to_string())
                    .join("ns")
                    .join(name),
            )
            .map_err(|e| e.to_string())?;
            let host =
                fs::read_link(Path::new("/proc/self/ns").join(name)).map_err(|e| e.to_string())?;
            if actual == host {
                return Err("host namespace shared".into());
            }
            e.append(
                &json!({"kind":"original-private-namespace","name":name,"namespace":actual}),
                false,
            )?;
        }
        Ok(())
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum CaptureStep {
        Preflight,
        Prepare,
        Create,
        InstallCleanup,
        Inspect,
        Seal,
        Arm,
        Readback,
        Start,
        Observe,
    }
    fn capture_steps(mut apply: impl FnMut(CaptureStep) -> Result<()>) -> Result<()> {
        for step in [
            CaptureStep::Preflight,
            CaptureStep::Prepare,
            CaptureStep::Create,
            CaptureStep::InstallCleanup,
            CaptureStep::Inspect,
            CaptureStep::Seal,
            CaptureStep::Arm,
            CaptureStep::Readback,
            CaptureStep::Start,
            CaptureStep::Observe,
        ] {
            apply(step)?;
        }
        Ok(())
    }
    #[derive(Deserialize, Serialize, Clone)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct CleanupClock {
        nonce: String,
        container_id: String,
        boot_id: String,
        first_ms: u64,
        end_ms: u64,
    }
    fn validate_cleanup_clock(clock: CleanupClock, b: &Binding) -> Result<u64> {
        if clock.nonce != b.nonce
            || clock.container_id != b.container_id
            || clock.boot_id != b.boot_id
            || clock.end_ms != lifecycle_end(b, clock.first_ms)?
        {
            return Err("cleanup deadline does not match original start".into());
        }
        Ok(clock.end_ms)
    }
    fn cleanup_clock(
        dir: &Path,
        b: &Binding,
        e: &mut Evidence,
        now: u64,
        resuming: bool,
    ) -> (u64, bool) {
        let path = dir.join("cleanup-begun.json");
        match fs::symlink_metadata(&path) {
            Ok(_) => match load::<CleanupClock>(&path).and_then(|c| validate_cleanup_clock(c, b)) {
                Ok(end) => (end, true),
                Err(_) => (now, false),
            },
            Err(error) if error.kind() == ErrorKind::NotFound && !resuming => {
                let end = lifecycle_end(b, now).unwrap_or(now);
                let saved = side_json(
                    e,
                    &path,
                    &CleanupClock {
                        nonce: b.nonce.clone(),
                        container_id: b.container_id.clone(),
                        boot_id: b.boot_id.clone(),
                        first_ms: now,
                        end_ms: end,
                    },
                )
                .is_ok();
                // The one-shot unit rate limit prohibits another invocation if this write fails.
                (end, saved)
            }
            _ => (now, false),
        }
    }
    #[derive(Clone, Copy)]
    struct CleanupBudget {
        end: u64,
    }
    impl CleanupBudget {
        fn diagnostics(self) -> u64 {
            self.end.saturating_sub(30_000)
        }
        fn kill(self) -> u64 {
            self.end.saturating_sub(20_000)
        }
        fn before_remove(self) -> u64 {
            self.end.saturating_sub(20_000)
        }
        fn remove(self) -> u64 {
            self.end.saturating_sub(10_000)
        }
    }
    #[derive(Default)]
    struct ClosureFacts {
        proofs: bool,
        start_settled: bool,
        observer_stopped: bool,
        deadline_valid: bool,
        evidence_complete: bool,
    }
    impl ClosureFacts {
        fn confirmed(&self) -> bool {
            self.proofs
                && self.start_settled
                && self.observer_stopped
                && self.deadline_valid
                && self.evidence_complete
        }
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct StartIntent {
        container_id: String,
        nonce: String,
        boot_id: String,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct StartResult {
        container_id: String,
        complete: bool,
    }
    fn settled_start(intent: &[u8], result: &[u8], b: &Binding) -> bool {
        let Ok(i) = parse::<StartIntent>(intent) else {
            return false;
        };
        let Ok(r) = parse::<StartResult>(result) else {
            return false;
        };
        i.container_id == b.container_id
            && i.nonce == b.nonce
            && i.boot_id == b.boot_id
            && r.container_id == b.container_id
            && r.complete
    }
    fn dispatch_despite_latch_error(
        latch: Result<()>,
        dispatch: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        let sent = dispatch();
        sent?;
        latch
    }
    fn unescape_mount(s: &str) -> Result<String> {
        let mut out = Vec::new();
        let mut b = s.as_bytes();
        while !b.is_empty() {
            if b[0] == b'\\' {
                if b.len() < 4 {
                    return Err("partial mount escape".into());
                }
                let c = match &b[1..4] {
                    b"040" => b' ',
                    b"011" => b'\t',
                    b"012" => b'\n',
                    b"134" => b'\\',
                    _ => return Err("unknown mount escape".into()),
                };
                out.push(c);
                b = &b[4..];
            } else {
                out.push(b[0]);
                b = &b[1..];
            }
        }
        String::from_utf8(out).map_err(|_| "mount path encoding".into())
    }
    fn mount_topology(raw: &[u8], roots: &[String], inside: bool) -> Result<()> {
        if !raw.ends_with(b"\n") {
            return Err("partial mount table".into());
        }
        let text = std::str::from_utf8(raw).map_err(|_| "mount table encoding")?;
        let mut found = BTreeSet::new();
        let mut ids = BTreeSet::new();
        let mut readonly_root = false;
        for line in text.lines() {
            let parts: Vec<_> = line.split_whitespace().collect();
            let split = parts
                .iter()
                .position(|p| *p == "-")
                .ok_or("mount separator")?;
            if split < 6 || parts.len() < split + 4 || !ids.insert(parts[0]) {
                return Err("malformed/duplicate mount record".into());
            }
            let target = unescape_mount(parts[4])?;
            let ro = parts[5].split(',').any(|v| v == "ro");
            if target == "/" {
                readonly_root = ro;
            }
            for root in roots {
                if target == *root {
                    if !ro
                        || parts[6..split]
                            .iter()
                            .any(|p| p.starts_with("shared:") || p.starts_with("master:"))
                        || !found.insert(root.clone())
                    {
                        return Err("input mount is writable/shared/duplicated".into());
                    }
                } else if target
                    .strip_prefix(root)
                    .is_some_and(|p| p.starts_with('/'))
                {
                    return Err("hidden nested input mount".into());
                }
            }
        }
        if inside && (!readonly_root || found.len() != roots.len()) {
            return Err("effective readonly input/root mounts missing".into());
        }
        Ok(())
    }
    fn attest_mounts(dir: &Path, pid: Option<u32>, e: &mut Evidence) -> Result<()> {
        let roots = if pid.is_some() {
            ["fixture", "distribution", "tool", "preflight"]
                .map(|p| format!("{INNER}/{p}"))
                .to_vec()
        } else {
            [
                dir.join("inputs/fixture"),
                dir.join("inputs/distribution"),
                dir.join("inputs/tool"),
                dir.join("preflight"),
            ]
            .map(|p| p.to_string_lossy().into_owned())
            .to_vec()
        };
        let p = pid.map_or_else(
            || PathBuf::from("/proc/self/mountinfo"),
            |p| Path::new("/proc").join(p.to_string()).join("mountinfo"),
        );
        let raw = read_file(&p, e.available() / 4)?;
        mount_topology(&raw, &roots, pid.is_some())?;
        e.append(&json!({"kind":"effective-input-mount-table","hostPid":pid,"roots":roots,"rawHex":raw_hex(&raw)}),false)
    }
    fn lifecycle_end(b: &Binding, now: u64) -> Result<u64> {
        let outer = match b.armed_ms {
            Some(t) => t.checked_add(600_000).ok_or("outer deadline overflow")?,
            None => b.preparation_deadline_ms,
        };
        Ok(outer.min(now.checked_add(60_000).ok_or("cleanup deadline overflow")?))
    }
    fn stopped(v: &Value, cid: &str) -> bool {
        v["Id"] == cid
            && v["State"]["Running"] == false
            && v["State"]["Restarting"] == false
            && v["State"]["Dead"] == false
            && v["State"]["Pid"] == 0
            && v["State"]["ExitCode"].is_i64()
            && v["State"]["StartedAt"].is_string()
            && v["State"]["FinishedAt"].is_string()
    }

    fn gone(identity: &ProcessIdentity, end: u64, e: &mut Evidence, stage: &str) -> Result<bool> {
        let stat = Path::new("/proc")
            .join(identity.init_pid.to_string())
            .join("stat");
        let (original_gone, raw) = match fs::symlink_metadata(&stat) {
            Err(error) if error.kind() == ErrorKind::NotFound => (true, Vec::new()),
            Err(error) => return Err(error.to_string()),
            Ok(_) => {
                let raw = read_file(&stat, 4096)?;
                (
                    process_start(&raw)? != (identity.init_pid, identity.init_start),
                    raw,
                )
            }
        };
        e.append(&json!({"kind":"original-init-closure","stage":stage,"hostPid":identity.init_pid,"originalStarttime":identity.init_start,"statHex":raw_hex(&raw),"observedMs":mono_ms()?,"originalGone":original_gone}),false)?;
        let empty = cgroup_closed(
            &identity.cgroup,
            identity.cgroup_device,
            identity.cgroup_inode,
            end,
            e,
            stage,
        )?;
        Ok(original_gone && empty)
    }
    fn binding(dir: &Path) -> Result<Binding> {
        let mut b: Binding = load(&dir.join("binding.json"))?;
        if b.schema_version != 1
            || !hex(&b.nonce, 64)
            || dir.file_name().and_then(|v| v.to_str()) != Some(&b.nonce)
            || !hex(&b.container_id, 64)
            || b.daemon_id.is_empty()
            || b.supervisor_cgroup
                != format!("/system.slice/{}.service", unit(&b.nonce, "supervisor"))
            || b.armed_ms.is_some()
        {
            return Err("protected cleanup binding mismatch".into());
        }
        if String::from_utf8_lossy(&read_file(
            Path::new("/proc/sys/kernel/random/boot_id"),
            128,
        )?)
        .trim()
            != b.boot_id
        {
            return Err("cleanup original boot differs".into());
        }
        for tool in [&b.docker, &b.systemctl, &b.curl] {
            protected(tool, false)?;
        }
        if dir
            .join("armed.json")
            .try_exists()
            .map_err(|e| e.to_string())?
        {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields, rename_all = "camelCase")]
            struct Armed {
                armed_ms: u64,
                final_deadline_ms: u64,
            }
            match load::<Armed>(&dir.join("armed.json")) {
                Ok(a) if a.armed_ms.checked_add(600_000) == Some(a.final_deadline_ms) => {
                    b.armed_ms = Some(a.armed_ms)
                }
                _ => b.uncertain = true,
            }
        }
        if dir.join("start-intent.json").exists() && b.armed_ms.is_none() {
            b.uncertain = true;
        }
        Ok(b)
    }

    fn cleanup_bound(dir: &Path, b: &Binding, direct_prearm: bool) -> Result<Value> {
        let resuming = dir
            .join("receipts/cleanup.evidence")
            .try_exists()
            .unwrap_or(true);
        let mut e = Evidence::open(&dir.join("receipts"), true).unwrap_or_else(|_| {
            let mut e = Evidence::preflight();
            e.limit = CLEANUP_BYTES;
            e.failed = true;
            e
        });
        let (mut service_end, mut clock_valid) = cleanup_clock(dir, b, &mut e, mono_ms()?, resuming);
        if !direct_prearm {
            let measured = e.checked(&b.systemctl,&args(&["show","--property=ExecMainStartTimestampMonotonic",&format!("{}.service",unit(&b.nonce,"cleanup"))]),service_end)
                .and_then(|raw|properties(&raw))
                .and_then(|values|values.get("ExecMainStartTimestampMonotonic").ok_or_else(||"cleanup systemd start timestamp unavailable".into()).and_then(|value|value.parse::<u64>().map_err(|_|"numeric timestamp".to_string()).or_else(|_|systemctl_usec(value))))
                .and_then(|start|{let start=start/1000;if start==0||start>mono_ms()?{return Err("actual cleanup systemd start timestamp invalid".into());}start.checked_add(60000).ok_or("cleanup service deadline overflow".into())});
            match measured {Ok(actual_end)=>service_end=service_end.min(actual_end),Err(_)=>clock_valid=false}
        }
        // Preserve 10s for immutable export and 2s for its final outcome inside the SAME 60s unit.
        let end = service_end.saturating_sub(12_000);
        let budget = CleanupBudget { end };
        let _ = e.append(&json!({"kind":"cleanup-export-budget","teardownEndMs":end,"exportEndMs":service_end.saturating_sub(2_000),"serviceEndMs":service_end}),false);
        let mut facts = ClosureFacts {
            proofs: !b.uncertain,
            deadline_valid: clock_valid,
            ..ClosureFacts::default()
        };
        if !dir.join("cleanup-requested").exists()
            && side(&mut e, &dir.join("cleanup-requested"), b"", 0o600, false).is_err()
        {
            facts.proofs = false;
        }
        if !direct_prearm {
            let _ = e.command(
                &b.systemctl,
                &args(&[
                    "kill",
                    "--kill-whom=all",
                    "--signal=KILL",
                    &format!("{}.service", unit(&b.nonce, "supervisor")),
                ]),
                budget.diagnostics(),
                None,
            );
            let shown = e.checked(
                &b.systemctl,
                &args(&[
                    "show",
                    "--property=ActiveState",
                    &format!("{}.service", unit(&b.nonce, "supervisor")),
                ]),
                budget.diagnostics(),
            );
            let inactive = shown.and_then(|v| properties(&v)).is_ok_and(|p| {
                p.get("ActiveState")
                    .is_some_and(|v| v == "inactive" || v == "failed")
            });
            let empty = cgroup_closed(
                &b.supervisor_cgroup,
                b.supervisor_cgroup_device,
                b.supervisor_cgroup_inode,
                budget.diagnostics(),
                &mut e,
                "supervisor-stopped",
            )
            .unwrap_or(false);
            facts.observer_stopped = inactive && empty;
        } else {
            if b.armed_ms.is_some()
                || dir.join("armed.json").try_exists().unwrap_or(true)
                || dir.join("start-intent.json").try_exists().unwrap_or(true)
            {
                return Err("direct cleanup after watchdog ownership/START forbidden".into());
            }
            facts.observer_stopped = true;
        }
        let start_intent = dir.join("start-intent.json").try_exists().unwrap_or(true);
        facts.start_settled = !start_intent
            || (read_file(&dir.join("start-intent.json"), INPUT_BYTES)
                .and_then(|intent| {
                    protected(&dir.join("start-intent.json"), false)?;
                    protected(&dir.join("start-result.json"), false)?;
                    let result = read_file(&dir.join("start-result.json"), INPUT_BYTES)?;
                    Ok(settled_start(&intent, &result, b))
                })
                .unwrap_or(false));
        if daemon_stamp().ok() != Some((b.daemon_pid, b.daemon_start))
            || daemon(&mut e, &b.docker, budget.diagnostics(), &b.daemon_id).is_err()
        {
            facts.proofs = false;
        }
        let identity = load::<ProcessIdentity>(&dir.join("process-identity.json"));
        facts.proofs &= contain_container(
            &mut e,
            b,
            budget,
            |e, program, argv, end| e.command(program, argv, end, None),
            |e, end| {
                !start_intent
                    || identity
                        .as_ref()
                        .is_ok_and(|i| gone(i, end, e, "pre-removal").unwrap_or(false))
            },
        );
        if daemon_stamp().ok() != Some((b.daemon_pid, b.daemon_start))
            || daemon(&mut e, &b.docker, end, &b.daemon_id).is_err()
        {
            facts.proofs = false;
        }
        let listed = e.checked(
            &b.docker,
            &args(&[
                "container",
                "ls",
                "--all",
                "--no-trunc",
                "--format",
                "{{json .ID}}",
            ]),
            end,
        );
        if !listed
            .as_ref()
            .is_ok_and(|raw| complete_absence(raw, &b.container_id))
        {
            facts.proofs = false;
        }
        let url = format!("http://localhost/v1.47/containers/{}/json", b.container_id);
        let absent = e.checked(
            &b.curl,
            &args(&[
                "--silent",
                "--show-error",
                "--max-time",
                "10",
                "--include",
                "--unix-socket",
                "/run/docker.sock",
                "--noproxy",
                "*",
                &url,
            ]),
            end,
        );
        if !absent
            .as_ref()
            .is_ok_and(|raw| typed_not_found(raw, &b.container_id))
            || daemon_stamp().ok() != Some((b.daemon_pid, b.daemon_start))
        {
            facts.proofs = false;
        }
        if start_intent
            && !identity
                .as_ref()
                .is_ok_and(|i| gone(i, end, &mut e, "post-removal").unwrap_or(false))
        {
            facts.proofs = false;
        }
        facts.deadline_valid &= mono_ms()? <= end;
        facts.evidence_complete = !e.failed;
        let confirmed = facts.confirmed();
        let result = json!({"schemaVersion":1,"containerId":b.container_id,"nonce":b.nonce,"bootId":b.boot_id,"teardown":if confirmed{"CONTAINER_TEARDOWN_CONFIRMED"}else{"UNKNOWN"},"officialCaptureSuccess":false,"reason":"receiver review required; host/daemon/partial closure is never clean","directPrearm":direct_prearm,"endMs":service_end});
        side(
            &mut e,
            &dir.join("teardown-receipt.json"),
            &serde_json::to_vec(&result).map_err(|e| e.to_string())?,
            0o600,
            true,
        )
        .map_err(|_| "cleanup attempted; final receipt unavailable; teardown UNKNOWN")?;
        if confirmed {
            let _ = e.command(
                &b.systemctl,
                &args(&["stop", &format!("{}.timer", unit(&b.nonce, "cleanup"))]),
                end,
                None,
            );
        }
        drop(e);
        // The unique outside-step cleanup owner also preserves the protected export after runner-step loss.
        // Export failure never rewrites the original teardown result.
        let export = protected_export_until(dir, service_end.saturating_sub(2_000));
        match export {
            Err(error) => {
                let bytes = serde_json::to_vec(&json!({"scope":"PR174_EXPORT_FAILURE","error":error,"officialCaptureSuccess":false})).map_err(|e|e.to_string())?;
                if bytes.len() <= 2048 { let _ = make_file(&dir.join("export-failure.json"), &bytes, 0o400); }
            },
            Ok(_)=>{},
        }
        Ok(result)
    }
    fn contain_container<C>(
        context: &mut C,
        b: &Binding,
        budget: CleanupBudget,
        mut command: impl FnMut(&mut C, &Path, &[String], u64) -> Result<Output>,
        mut proof: impl FnMut(&mut C, u64) -> bool,
    ) -> bool {
        let checked = |out: Result<Output>| -> Result<Vec<u8>> {
            let out = out?;
            if !out.ok() {
                return Err("incomplete cleanup command".into());
            }
            Ok(out.stdout)
        };
        let mut complete = true;
        let state = checked(command(
            context,
            &b.docker,
            &args(&["container", "inspect", &b.container_id]),
            budget.diagnostics(),
        ))
        .and_then(|raw| one_inspect(&raw));
        if !state
            .as_ref()
            .is_ok_and(|v| v["Id"] == b.container_id && v["State"]["Running"] == false)
            && !command(
                context,
                &b.docker,
                &args(&["kill", "--signal=KILL", &b.container_id]),
                budget.kill(),
            )
            .as_ref()
            .is_ok_and(Output::ok)
        {
            complete = false;
        }
        let waited = checked(command(
            context,
            &b.docker,
            &args(&["wait", &b.container_id]),
            budget.before_remove(),
        ));
        let exit = waited
            .as_ref()
            .ok()
            .and_then(|v| std::str::from_utf8(v).ok())
            .and_then(|v| v.trim().parse::<i32>().ok());
        let before = checked(command(
            context,
            &b.docker,
            &args(&["container", "inspect", &b.container_id]),
            budget.before_remove(),
        ))
        .and_then(|raw| one_inspect(&raw));
        let original_gone = proof(context, budget.before_remove());
        if exit.is_none()
            || !before.as_ref().is_ok_and(|v| {
                stopped(v, &b.container_id)
                    && v["State"]["ExitCode"].as_i64() == exit.map(i64::from)
            })
            || !original_gone
        {
            complete = false;
        }
        let removed = command(
            context,
            &b.docker,
            &args(&["rm", &b.container_id]),
            budget.remove(),
        );
        if !removed.as_ref().is_ok_and(|o| {
            o.ok() && std::str::from_utf8(&o.stdout).is_ok_and(|v| v.trim() == b.container_id)
        }) {
            complete = false;
            let _ = command(
                context,
                &b.docker,
                &args(&["rm", "--force", &b.container_id]),
                budget.end,
            );
        }
        complete
    }
    fn complete_absence(b: &[u8], cid: &str) -> bool {
        let Ok(s) = std::str::from_utf8(b) else {
            return false;
        };
        if !s.is_empty() && !s.ends_with('\n') {
            return false;
        }
        let mut seen = BTreeSet::new();
        for line in s.lines() {
            let Ok(id) = parse::<String>(line.as_bytes()) else {
                return false;
            };
            if !hex(&id, 64) || id == cid || !seen.insert(id) {
                return false;
            }
        }
        true
    }
    fn daemon_stamp() -> Result<(u32, u64)> {
        let p = Path::new("/run/docker.pid");
        protected(p, false)?;
        let raw = read_file(p, 128)?;
        let pid = std::str::from_utf8(&raw)
            .map_err(|_| "daemon PID encoding")?
            .trim()
            .parse::<u32>()
            .map_err(|_| "daemon PID")?;
        if pid <= 1 {
            return Err("daemon PID identity".into());
        }
        let stamp = process_start(&read_file(
            &Path::new("/proc").join(pid.to_string()).join("stat"),
            4096,
        )?)?;
        if stamp.0 != pid {
            return Err("daemon PID substituted".into());
        }
        Ok(stamp)
    }
    fn typed_not_found(raw: &[u8], cid: &str) -> bool {
        let Some(split) = raw.windows(4).position(|v| v == b"\r\n\r\n") else {
            return false;
        };
        if split + 4 > 8192 {
            return false;
        }
        let Ok(h) = std::str::from_utf8(&raw[..split]) else {
            return false;
        };
        let mut lines = h.split("\r\n");
        if !lines.next().is_some_and(|s| s == "HTTP/1.1 404 Not Found") {
            return false;
        }
        let mut headers = std::collections::BTreeMap::new();
        for line in lines {
            let Some((key, value)) = line.split_once(':') else {
                return false;
            };
            let key = key.to_ascii_lowercase();
            if headers.insert(key, value.trim()).is_some() {
                return false;
            }
        }
        let body = &raw[split + 4..];
        if headers.contains_key("transfer-encoding")
            || headers
                .get("content-length")
                .and_then(|v| v.parse::<usize>().ok())
                != Some(body.len())
            || !headers
                .get("content-type")
                .is_some_and(|v| v.starts_with("application/json"))
        {
            return false;
        }
        let Ok(v) = parse::<Value>(body) else {
            return false;
        };
        v == json!({"message":format!("No such container: {cid}")})
    }
    fn start_allowed(dir: &Path, b: &Binding, now: u64) -> Result<()> {
        if b.armed_ms.is_none_or(|t| now >= t.saturating_add(540_000))
            || cancellation_latched(dir)?
            || dir
                .join("teardown-receipt.json")
                .try_exists()
                .map_err(|e| e.to_string())?
        {
            return Err("cleanup/deadline permanently excludes START".into());
        }
        Ok(())
    }
    fn readback_critical(dir: &Path, a: &Admission, e: &mut Evidence) -> Result<()> {
        for (p, d) in MEMBERS {
            hash_file(
                e,
                a,
                &dir.join("inputs/distribution").join(RELEASE).join(p),
                d,
            )?;
        }
        for (name, bytes, d) in FIXTURES {
            let file = dir.join("inputs/fixture").join(name);
            protected(&file, false)?;
            if fs::metadata(&file).map_err(|e| e.to_string())?.len() != bytes {
                return Err("fixture byte length".into());
            }
            hash_file(e, a, &dir.join("inputs/fixture").join(name), d)?;
        }
        hash_file(
            e,
            a,
            &dir.join("inputs/tool/ktlsp-oracle-official-pr174"),
            &a.adapter_sha256,
        )?;
        if dir
            .join("state/capture")
            .try_exists()
            .map_err(|e| e.to_string())?
        {
            return Err("output is not fresh".into());
        }
        Ok(())
    }
    fn observe(dir: &Path, a: &Admission, b: &Binding, e: &mut Evidence) -> Result<()> {
        let end = b
            .armed_ms
            .ok_or("not armed")?
            .checked_add(540_000)
            .ok_or("deadline overflow")?
            .min(mono_ms()?.checked_add(300_000).ok_or("session overflow")?);
        let v = inspect(e, &a.tools.docker, &b.container_id, end)?;
        config(&v, dir, a, &b.container_id, false)?;
        let pid = v["State"]["Pid"]
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| *v > 1)
            .ok_or("original init observation missed")?;
        let identity = original_init(pid)?;
        if identity.cgroup != format!("/system.slice/docker-{}.scope", b.container_id) {
            return Err("init cgroup not attributable to original full CID".into());
        }
        side_json(e, &dir.join("process-identity.json"), &identity)?;
        attest_mounts(dir, Some(pid), e)?;
        actual_resources(&identity, e)?;
        let mut seen = Attestation::default();
        loop {
            if dir
                .join("cleanup-requested")
                .try_exists()
                .map_err(|e| e.to_string())?
                || mono_ms()? >= end
            {
                return Err("capture cancelled/deadline".into());
            }
            let stat_path = Path::new("/proc").join(pid.to_string()).join("stat");
            if !stat_path.try_exists().map_err(|e| e.to_string())? {
                break;
            }
            if process_start(&read_file(&stat_path, 4096)?)? != (pid, identity.init_start) {
                return Err("original init PID reused before exit receipt".into());
            }
            attest(dir, &identity, end, e, &mut seen)?;
            std::thread::yield_now();
        }
        let v = inspect(e, &a.tools.docker, &b.container_id, end)?;
        if !stopped(&v, &b.container_id)
            || v["State"]["ExitCode"] != 0
            || v["State"]["OOMKilled"] != false
            || v["RestartCount"] != 0
            || seen.recorder.is_none()
            || seen.launcher.is_none()
            || !seen.jvm
            || e.failed
        {
            return Err("normal exit or original runtime/security attestation incomplete".into());
        }
        e.append(&json!({"kind":"capture-host-observation","containerId":b.container_id,"originalNormalExitZero":true,"pinnedLibjvmObserved":true,"transportAndSymbols":"independent receiver review required; host does not read child-writable paths"}),false)?;
        Ok(())
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum CleanupOwner {
        None,
        DirectPrearm,
        Service,
    }
    fn cleanup_owner(known: bool, installed: bool) -> CleanupOwner {
        if !known {
            CleanupOwner::None
        } else if installed {
            CleanupOwner::Service
        } else {
            CleanupOwner::DirectPrearm
        }
    }
    fn finish_supervision(
        run: Result<()>,
        dir: &Path,
        known: Option<&Binding>,
        cleanup_ready: bool,
        e: &mut Evidence,
    ) -> Result<()> {
        let cleanup = match cleanup_owner(known.is_some(), cleanup_ready) {
            CleanupOwner::None => Ok(()),
            CleanupOwner::Service => {
                let b = known.ok_or("cleanup CID missing")?;
                request_cleanup(dir, b, e, lifecycle_end(b, mono_ms()?)?)
            }
            CleanupOwner::DirectPrearm => {
                cleanup_bound(dir, known.ok_or("cleanup CID missing")?, true).map(|_| ())
            }
        };
        run?;
        cleanup
    }
    fn supervise(dir: &Path) -> Result<Value> {
        let a = admission(dir)?;
        root_authority(dir, &a)?;
        bootstrap_receipt_gate(dir, &a)?;
        let mut e = Evidence::preflight();
        e.limit = OBSERVER_BYTES - 131_072 - 16_384 - 16_384 - 8_192
            - if calibration_case(dir, &a)? == Some(CalibrationCase::WorkflowStepCancel) {131_072}else{0};
        let mut cg = String::new();
        let mut stamp = None;
        let mut known: Option<Binding> = None;
        let mut cleanup_ready = false;
        let run = capture_steps(|step| {
            match step {
                CaptureStep::Preflight => {
                    for name in [
                        "state",
                        "preflight",
                        "receipts",
                        "binding.json",
                        "cleanup-requested",
                        "cleanup-service-ready",
                        "start-intent.json",
                        "teardown-receipt.json",
                    ] {
                        match fs::symlink_metadata(dir.join(name)) {
                            Err(error) if error.kind() == ErrorKind::NotFound => {}
                            _ => return Err("original output/admission nonce already used".into()),
                        }
                    }
                    for kind in ["service", "timer"] {
                        if service_path(&a.nonce, kind)
                            .try_exists()
                            .map_err(|e| e.to_string())?
                        {
                            return Err("original nonce has an existing lifecycle unit".into());
                        }
                    }
                    cg = preflight(dir, &a, &mut e)?;
                    stamp = Some(daemon_stamp()?);
                    if e.failed {
                        return Err("preflight evidence incomplete".into());
                    }
                    return Ok(());
                }
                CaptureStep::Prepare => {
                    mkdir(&dir.join("receipts"), 0o700)?;
                    e.commit(&dir.join("receipts"))?;
                    mkdir(&dir.join("preflight"), 0o755)?;
                    mount_state(dir, &a, &mut e)?;
                    return Ok(());
                }
                CaptureStep::Create => {
                    let out = e.checked(
                        &a.tools.docker,
                        &create_argv(dir, &a)?,
                        a.preparation_deadline_ms,
                    )?;
                    let cid = std::str::from_utf8(&out)
                        .map_err(|_| "CREATE response encoding")?
                        .trim();
                    if !hex(cid, 64) {
                        return Err("CREATE full CID unavailable; no guessed cleanup".into());
                    }
                    let (daemon_pid, daemon_start) = stamp.ok_or("daemon stamp missing")?;
                    let object = fs::metadata(cgroup_path(&cg)?).map_err(|e| e.to_string())?;
                    known = Some(Binding {
                        schema_version: 1,
                        nonce: a.nonce.clone(),
                        container_id: cid.into(),
                        daemon_id: a.daemon_id.clone(),
                        boot_id: a.boot_id.clone(),
                        preparation_deadline_ms: a.preparation_deadline_ms,
                        armed_ms: None,
                        uncertain: false,
                        supervisor_cgroup: cg.clone(),
                        supervisor_cgroup_device: object.dev(),
                        supervisor_cgroup_inode: object.ino(),
                        daemon_pid,
                        daemon_start,
                        curl: a.tools.curl.clone(),
                        docker: a.tools.docker.clone(),
                        systemctl: a.tools.systemctl.clone(),
                    });
                    side_json(
                        &mut e,
                        &dir.join("binding.json"),
                        known.as_ref().ok_or("CREATE binding missing")?,
                    )?;
                    return Ok(());
                }
                _ => {}
            }
            let b = known
                .as_mut()
                .ok_or("original CID missing before lifecycle action")?;
            let cid = b.container_id.clone();
            match step {
                CaptureStep::InstallCleanup => {
                    install_cleanup(dir, &a, &mut e)?;
                    cleanup_ready = true;
                    publish_cleanup_service(dir, b, &mut e)
                }
                CaptureStep::Inspect => config(
                    &inspect(&mut e, &a.tools.docker, &cid, a.preparation_deadline_ms)?,
                    dir,
                    &a,
                    &cid,
                    true,
                ),
                CaptureStep::Seal => {
                    if calibration_case(dir, &a)?.is_some() { seal_calibration(dir, &a, &cid, &mut e) }
                    else { seal_handoff(dir, &a, &cid, &mut e) }
                },
                CaptureStep::Arm => {
                    if a.preparation_deadline_ms
                        < mono_ms()?
                            .checked_add(600_000)
                            .ok_or("job deadline overflow")?
                    {
                        return Err("job deadline lacks complete lifecycle reserve".into());
                    }
                    arm(dir, &a, b, &mut e)
                }
                CaptureStep::Readback => {
                    readback_critical(dir, &a, &mut e)?;
                    attest_mounts(dir, None, &mut e)?;
                    config(
                        &inspect(&mut e, &a.tools.docker, &cid, a.preparation_deadline_ms)?,
                        dir,
                        &a,
                        &cid,
                        true,
                    )?;
                    daemon(
                        &mut e,
                        &a.tools.docker,
                        a.preparation_deadline_ms,
                        &a.daemon_id,
                    )?;
                    if daemon_stamp()? != (b.daemon_pid, b.daemon_start) {
                        return Err("daemon changed before START".into());
                    }
                    start_allowed(dir, b, mono_ms()?)
                }
                CaptureStep::Start => {
                    start_allowed(dir, b, mono_ms()?)?;
                    side_json(
                        &mut e,
                        &dir.join("start-intent.json"),
                        &json!({"containerId":cid,"nonce":b.nonce,"bootId":b.boot_id}),
                    )?;
                    start_allowed(dir, b, mono_ms()?)?;
                    let out = e.command(
                        &a.tools.docker,
                        &args(&["start", &cid]),
                        b.armed_ms.ok_or("arming missing")? + 540_000,
                        None,
                    )?;
                    let complete =
                        out.ok() && std::str::from_utf8(&out.stdout).is_ok_and(|v| v.trim() == cid);
                    side_json(
                        &mut e,
                        &dir.join("start-result.json"),
                        &json!({"containerId":cid,"complete":complete}),
                    )?;
                    if !complete {
                        return Err("START incomplete/ambiguous; permanent UNKNOWN".into());
                    }
                    Ok(())
                }
                CaptureStep::Observe => {
                    if let Some(case) = calibration_case(dir, &a)? { observe_calibration(dir, &a, b, &mut e, case) }
                    else { observe(dir, &a, b, &mut e) }
                },
                _ => Err("invalid lifecycle phase".into()),
            }
        });
        let _=e.append(&json!({"kind":"supervisor-final","captureError":run.as_ref().err(),"containerId":known.as_ref().map(|b|&b.container_id),"officialCaptureSuccess":false,"closure":"UNKNOWN until independent cleanup receipt"}),true);
        if e.file.is_none() && !e.pending.is_empty()
            && fs::symlink_metadata(dir.join("receipts/observer.evidence")).is_err_and(|error|error.kind()==ErrorKind::NotFound) {
            // Preserve pre-CREATE raw failure prefixes without duplicating an already charged partial observer file.
            let _ = make_file(&dir.join("preflight-failure.evidence"), &e.pending, 0o600);
        }
        finish_supervision(run, dir, known.as_ref(), cleanup_ready, &mut e)?;
        Ok(
            json!({"status":"CLEANUP_REQUESTED","officialCaptureSuccess":false,"receiverReviewRequired":true}),
        )
    }
    pub fn entry() -> Result<Value> {
        let argv: Vec<_> = std::env::args().skip(1).collect();
        if argv.as_slice()==["describe-template"] {
            return Ok(json!({"schemaVersion":1,"scope":"PR174_CONTAINMENT_TEMPLATE_SOURCE","containmentTemplateSha256":containment_template_sha256()?,"profile":child_confinement::profile(),"runtimeAuthority":"NONE; exact source policy only, no kernel/launch/calibration fact"}));
        }
        if !cfg!(target_os = "linux") {
            return Err(
                "host execution is Linux-only; optional macOS compilation remains supported".into(),
            );
        }
        if argv.first().is_some_and(|mode| mode == "workflow-source") {
            return workflow_source(&argv);
        }
        unsafe extern "C" {
            fn geteuid() -> u32;
        }
        if unsafe { geteuid() } != 0 {
            return Err("protected host execution requires OS root".into());
        }
        if argv.first().is_some_and(|mode| mode.starts_with("workflow-")) {
            return workflow_entry(&argv);
        }
        if argv.as_slice() == ["bootstrap", STAGING] {
            return bootstrap();
        }
        if argv.as_slice() == ["collect-calibration", STAGING] {
            return collect_calibration();
        }
        if argv.len() != 2 {
            return Err("usage: ktlsp-oracle-pr174-host bootstrap /etc/ktlsp-pr174/staging | supervise|calibrate|export|cleanup|request-cleanup /run/ktlsp-pr174/NONCE".into());
        }
        let dir = directory(&argv[1])?;
        entry_mode(&argv[0], &dir)
    }
    // The caller has already validated OS root and the protected nonce directory.
    fn entry_mode(mode: &str, dir: &Path) -> Result<Value> {
        match mode {
            "supervise" | "calibrate" => {
                let a = admission(dir)?;
                if (mode == "calibrate") != (a.scope == "PR174_RUST_CALIBRATION_ADMITTED") {
                    return Err("official and Rust-only calibration entry scopes differ".into());
                }
                supervise(dir)
            },
            "export" => protected_export(dir),
            "await-calibration" => await_calibration(dir),
            "release-calibration" => release_calibration(dir),
            "cleanup" => {
                let b = binding(dir)?;
                let expected = format!("0::/system.slice/{}.service", unit(&b.nonce, "cleanup"));
                if String::from_utf8_lossy(&read_file(Path::new("/proc/self/cgroup"), INPUT_BYTES)?)
                    .trim()
                    != expected
                {
                    return Err("cleanup must be the unique outside-step service".into());
                }
                if dir
                    .join("teardown-receipt.json")
                    .try_exists()
                    .map_err(|e| e.to_string())?
                {
                    return load(&dir.join("teardown-receipt.json"));
                }
                cleanup_bound(dir, &b, false)
            }

            "request-cleanup" => {
                let latch = latch_cleanup(dir);
                let mut dispatched = false;
                dispatch_despite_latch_error(latch, || {
                    match fs::symlink_metadata(dir.join("binding.json")) {
                        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
                        Err(error) => return Err(error.to_string()),
                        Ok(_) => {}
                    }
                    // Existing-but-invalid custody/data is an error, not an unknown binding.
                    let b = binding(dir)?;
                    dispatched = request_cleanup_external(dir, &b)?;
                    Ok(())
                })?;
                Ok(json!({"status":if dispatched {"CLEANUP_REQUESTED"} else {"CLEANUP_LATCHED"}}))
            }
            _ => Err("unknown fixed host mode".into()),
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        struct Temp(PathBuf);
        impl Temp {
            fn new() -> Self {
                let p = std::env::temp_dir().join(format!(
                    "ktlsp-host-peer-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                fs::create_dir(&p).unwrap();
                Self(p)
            }
        }
        impl Drop for Temp {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        fn binding_fixture() -> Binding {
            Binding {
                schema_version: 1,
                nonce: "a".repeat(64),
                container_id: "b".repeat(64),
                daemon_id: "synthetic-daemon".into(),
                boot_id: "synthetic-boot".into(),
                preparation_deadline_ms: 900_000,
                armed_ms: Some(1000),
                uncertain: false,
                supervisor_cgroup_device: 1,
                supervisor_cgroup_inode: 2,
                supervisor_cgroup: format!(
                    "/system.slice/{}.service",
                    unit(&"a".repeat(64), "supervisor")
                ),
                daemon_pid: 2,
                daemon_start: 3,
                curl: "/unused/curl".into(),
                docker: "/unused/docker".into(),
                systemctl: "/unused/systemctl".into(),
            }
        }
        fn peer(mode: &str) -> Command {
            let mut c = Command::new(std::env::current_exe().unwrap());
            c.args([
                "--exact",
                "host::tests::command_peer",
                "--ignored",
                "--nocapture",
            ])
            .env_clear()
            .env("KTLSP_HOST_TEST_PEER", mode);
            c
        }
        #[test]
        #[ignore = "Rust executable peer; invoked by command-reader and early-cancellation regressions"]
        fn command_peer() {
            match std::env::var("KTLSP_HOST_TEST_PEER").unwrap().as_str() {
                "empty" => {}
                "partial" => {
                    std::io::stdout().write_all(b"host-peer-").unwrap();
                    std::thread::yield_now();
                    std::io::stdout().write_all(b"fragment\n").unwrap();
                    std::io::stderr().write_all(b"peer-stderr\n").unwrap();
                }
                "overflow" => {
                    for _ in 0..100 {
                        std::io::stdout().write_all(&[b'x'; 4096]).unwrap();
                    }
                }
                "early-cancel" => {
                    for state in [
                        "preflight",
                        "prepare",
                        "create",
                        "invalid-binding",
                        "preexisting-latch",
                        "symlink-latch",
                    ] {
                        let temp = Temp::new();
                        if state == "prepare" {
                            fs::create_dir(temp.0.join("receipts")).unwrap();
                        }
                        if state == "create" {
                            fs::write(temp.0.join("container"), b"synthetic created").unwrap();
                        }
                        if state == "invalid-binding" {
                            fs::write(temp.0.join("binding.json"), b"{").unwrap();
                        }
                        if state == "preexisting-latch" {
                            fs::write(temp.0.join("cleanup-requested"), b"preserve cancellation")
                                .unwrap();
                        }
                        if state == "symlink-latch" {
                            std::os::unix::fs::symlink("absent", temp.0.join("cleanup-requested"))
                                .unwrap();
                        }
                        // Only protected-directory admission is bypassed; use the real CLI mode.
                        let result = entry_mode("request-cleanup", &temp.0);
                        if state == "invalid-binding" {
                            assert!(result.is_err());
                        } else {
                            assert_eq!(result.as_ref().unwrap()["status"], "CLEANUP_LATCHED");
                        }
                        assert!(cancellation_latched(&temp.0).unwrap());
                        let b = binding_fixture();
                        if state == "create" {
                            // Publication after cancellation cannot make START legal again.
                            fs::write(temp.0.join("binding.json"), serde_json::to_vec(&b).unwrap())
                                .unwrap();
                        }
                        assert!(start_allowed(&temp.0, &b, 2000).is_err());
                        assert!(!temp.0.join("commands").exists());
                        if state == "preexisting-latch" {
                            assert_eq!(
                                fs::read(temp.0.join("cleanup-requested")).unwrap(),
                                b"preserve cancellation"
                            );
                        }
                        if state == "symlink-latch" {
                            assert_eq!(
                                fs::read_link(temp.0.join("cleanup-requested")).unwrap(),
                                Path::new("absent")
                            );
                        }
                        println!(
                            "EARLY_CANCEL_SMOKE {}",
                            json!({"state":state,"status":if result.is_ok(){"CLEANUP_LATCHED"}else{"ERROR_LATCHED"},"startAllowed":false,"serviceDispatched":false,"officialCaptureSuccess":false})
                        );
                    }
                }
                "blocked" => loop {
                    std::thread::yield_now();
                },
                _ => panic!("unknown synthetic peer mode"),
            }
        }
        #[test]
        fn executable_early_cancellation_precedes_binding_and_excludes_start() {
            let out = bounded_command(
                peer("early-cancel"),
                16_384,
                Instant::now() + Duration::from_secs(5),
            )
            .unwrap();
            assert!(out.ok(), "{}", String::from_utf8_lossy(&out.stderr));
            let text = String::from_utf8(out.stdout).unwrap();
            print!("{text}");
        }
        #[test]
        fn bounded_command_reads_fragmented_caps_and_timeout() {
            let out = bounded_command(
                peer("partial"),
                4096,
                Instant::now() + Duration::from_secs(2),
            )
            .unwrap();
            assert!(out.ok());
            assert!(
                out.stdout
                    .windows(b"host-peer-fragment".len())
                    .any(|b| b == b"host-peer-fragment")
            );
            assert!(out.stderr.ends_with(b"peer-stderr\n"));
            let out = bounded_command(
                peer("overflow"),
                33,
                Instant::now() + Duration::from_secs(2),
            )
            .unwrap();
            assert!(out.truncated);
            assert!(out.stdout.len() + out.stderr.len() <= 33);
            assert!(!out.ok());
            let start = Instant::now();
            let out =
                bounded_command(peer("blocked"), 4096, start + Duration::from_millis(50)).unwrap();
            assert!(out.timeout);
            assert!(!out.ok());
            assert!(start.elapsed() < Duration::from_secs(2));
            assert!(
                bounded_command(
                    peer("partial"),
                    1,
                    Instant::now() - Duration::from_millis(1)
                )
                .is_err()
            );
        }
        #[test]
        fn duplicate_partial_extended_and_oversized_receipts_are_not_facts() {
            for raw in [
                br#"{"x":1,"x":2}"#.as_slice(),
                br#"{"x":{"a":1,"a":1}}"#,
                br#"{"x":1} {}"#,
                b"{",
                b"",
            ] {
                assert!(parse::<Value>(raw).is_err());
            }
            let temp = Temp::new();
            let p = temp.0.join("large");
            fs::write(&p, [0u8; 17]).unwrap();
            assert!(read_file(&p, 16).is_err());
            std::os::unix::fs::symlink(&p, temp.0.join("link")).unwrap();
            assert!(read_file(&temp.0.join("link"), 32).is_err());
            assert!(!digest(&"0".repeat(64)));
            assert!(!hex(&"G".repeat(64), 64));
            assert!(!safe_absolute(Path::new("/run/../escape")));
        }
        #[test]
        fn failed_file_writes_preserve_cancellation_and_unrelated_paths() {
            let temp = Temp::new();
            let path = temp.0.join("cleanup-requested");
            for partial in [false, true] {
                assert!(
                    make_file_with(&path, b"cancel", 0o600, |f, _| {
                        if partial {
                            f.write_all(b"part")?;
                        }
                        Err(std::io::Error::other("injected write/sync failure"))
                    })
                    .is_err()
                );
                assert!(path.exists());
                let mut dispatched = false;
                assert!(
                    dispatch_despite_latch_error(Err("failed persistence".into()), || {
                        dispatched = true;
                        Err("failed dispatch".into())
                    })
                    .is_err()
                );
                assert!(dispatched);
                assert!(start_allowed(&temp.0, &binding_fixture(), 2000).is_err());
                fs::remove_file(&path).unwrap();
            }
            make_file(&path, b"preexisting", 0o600).unwrap();
            assert!(make_file(&path, b"replacement", 0o600).is_err());
            assert_eq!(fs::read(&path).unwrap(), b"preexisting");
            fs::remove_file(&path).unwrap();
            fs::write(temp.0.join("unrelated"), b"unrelated").unwrap();
            std::os::unix::fs::symlink(temp.0.join("unrelated"), &path).unwrap();
            assert!(make_file(&path, b"replacement", 0o600).is_err());
            assert_eq!(fs::read(&path).unwrap(), b"unrelated");
        }
        #[test]
        fn actual_direct_cleanup_rejects_watchdog_or_start_ownership() {
            for marker in ["binding-armed", "armed.json", "start-intent.json"] {
                let temp = Temp::new();
                fs::create_dir(temp.0.join("receipts")).unwrap();
                let mut b = binding_fixture();
                b.preparation_deadline_ms = mono_ms().unwrap() + 120_000;
                if marker != "binding-armed" {
                    b.armed_ms = None;
                    fs::write(temp.0.join(marker), b"partial").unwrap();
                }
                assert!(
                    cleanup_bound(&temp.0, &b, true)
                        .unwrap_err()
                        .contains("forbidden")
                );
                assert!(!temp.0.join("teardown-receipt.json").exists());
            }
        }
        #[test]
        fn executable_install_faults_route_actual_direct_cleanup() {
            // Full optional-workspace cargo test builds its integration-test binary peers.
            let peer = std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("host-cleanup-test-peer");
            assert!(
                peer.is_file(),
                "build the optional workspace binaries before this filtered test"
            );
            for fault in [
                "write",
                "sync",
                "reload",
                "readback-command",
                "readback-policy",
                "preexisting",
                "symlink",
                "marker-write",
                "ready",
            ] {
                let temp = Temp::new();
                for program in ["systemctl", "docker", "curl"] {
                    fs::copy(&peer, temp.0.join(program)).unwrap();
                }
                fs::create_dir(temp.0.join("receipts")).unwrap();
                fs::write(temp.0.join("fault"), fault).unwrap();
                let mut b = binding_fixture();
                b.armed_ms = None;
                b.preparation_deadline_ms = mono_ms().unwrap() + 120_000;
                b.docker = temp.0.join("docker");
                b.systemctl = temp.0.join("systemctl");
                b.curl = temp.0.join("curl");
                fs::write(temp.0.join("cid"), &b.container_id).unwrap();
                fs::write(temp.0.join("container"), b"created").unwrap();
                let path = temp.0.join("cleanup.service");
                if fault == "preexisting" {
                    fs::write(&path, b"unrelated unit").unwrap();
                }
                if fault == "symlink" {
                    fs::write(temp.0.join("unrelated.service"), b"unrelated unit").unwrap();
                    std::os::unix::fs::symlink(temp.0.join("unrelated.service"), &path).unwrap();
                }
                let mut e = Evidence::preflight();
                let installed = install_cleanup_at(
                    &temp.0,
                    &b.nonce,
                    &path,
                    &b.systemctl,
                    b.preparation_deadline_ms,
                    &mut e,
                    |e, path, bytes| {
                        if matches!(fault, "write" | "sync") {
                            make_file_with(path, bytes, 0o600, |f, bytes| {
                                f.write_all(if fault == "write" {
                                    &bytes[..16]
                                } else {
                                    bytes
                                })?;
                                Err(std::io::Error::other("injected write/sync failure"))
                            })
                        } else {
                            side(e, path, bytes, 0o600, false)
                        }
                    },
                );
                let ready = installed.is_ok();
                assert_eq!(
                    ready,
                    matches!(fault, "ready" | "marker-write"),
                    "{fault}: {installed:?}"
                );
                assert!(!request_cleanup_external(&temp.0, &b).unwrap());
                assert!(!temp.0.join("service-requested").exists());
                let run = if ready {
                    if fault == "marker-write" {
                        fs::write(temp.0.join("cleanup-service-ready"), b"partial").unwrap();
                    }
                    let published = publish_cleanup_service(&temp.0, &b, &mut e);
                    if fault == "ready" {
                        assert!(request_cleanup_external_with(&temp.0, &b, |_| Ok(())).unwrap());
                    } else {
                        assert!(published.is_err());
                        assert!(
                            request_cleanup_external_with(&temp.0, &b, |_| Ok(()))
                                .unwrap_err()
                                .contains("marker mismatch")
                        );
                        assert!(!temp.0.join("service-requested").exists());
                    }
                    published
                } else {
                    installed
                };
                let finished = finish_supervision(run, &temp.0, Some(&b), ready, &mut e);
                assert_eq!(finished.is_ok(), fault == "ready", "{fault}: {finished:?}");
                let commands = fs::read_to_string(temp.0.join("commands")).unwrap();
                let calls: Vec<Value> = commands
                    .lines()
                    .map(|s| serde_json::from_str(s).unwrap())
                    .collect();
                if ready {
                    assert!(temp.0.join("service-requested").is_file());
                    assert!(temp.0.join("container").is_file());
                    assert!(!calls.iter().any(|c| c["program"] == "docker"));
                } else {
                    assert!(!temp.0.join("container").exists(), "{fault}: {commands}");
                    assert!(!temp.0.join("service-requested").exists());
                    assert!(
                        calls.iter().any(|c| c["program"] == "docker"
                            && c["args"] == json!(["rm", b.container_id]))
                    );
                    let receipt: Value = serde_json::from_slice(
                        &fs::read(temp.0.join("teardown-receipt.json")).unwrap(),
                    )
                    .unwrap();
                    assert_eq!(receipt["directPrearm"], true);
                    assert_eq!(receipt["teardown"], "UNKNOWN");
                    assert_eq!(receipt["officialCaptureSuccess"], false);
                }
                if matches!(fault, "preexisting" | "symlink") {
                    assert_eq!(fs::read(&path).unwrap(), b"unrelated unit");
                }
                assert!(path.is_file());
                println!(
                    "synthetic install fault={fault} owner={:?} containerPresent={} officialCaptureSuccess=false",
                    cleanup_owner(true, ready),
                    temp.0.join("container").exists()
                );
            }
        }
        #[test]
        fn external_cleanup_rejects_partial_or_unrelated_ownership_markers() {
            for bytes in [b"".as_slice(), b"partial", b"unrelated:container:boot\n"] {
                let temp = Temp::new();
                let b = binding_fixture();
                fs::write(temp.0.join("cleanup-service-ready"), bytes).unwrap();
                assert!(
                    request_cleanup_external_with(&temp.0, &b, |_| Ok(()))
                        .unwrap_err()
                        .contains("marker mismatch")
                );
                assert!(temp.0.join("cleanup-requested").exists());
                assert!(start_allowed(&temp.0, &b, 2000).is_err());
            }
        }
        #[test]
        fn aggregate_evidence_and_side_writes_do_not_reset_or_borrow() {
            assert_eq!(OBSERVER_BYTES + CLEANUP_BYTES, 1_048_576);
            let mut e = Evidence::preflight();
            e.limit = 128 + TERMINAL_BYTES;
            e.append(&json!({"first":"prefix"}), false).unwrap();
            let used = e.used;
            assert!(
                e.append(&json!({"oversize":"x".repeat(200)}), false)
                    .is_err()
            );
            assert_eq!(e.used, used);
            assert!(e.failed);
            let temp = Temp::new();
            let bytes = [b'x'; 129];
            assert!(side(&mut e, &temp.0.join("not-created"), &bytes, 0o600, false).is_err());
            assert!(!temp.0.join("not-created").exists());
            assert!(e.append(&json!({"teardown":"UNKNOWN"}), true).is_ok());
            assert!(e.used <= e.limit);
        }
        #[test]
        fn cleanup_latch_and_original_deadline_permanently_exclude_start() {
            let temp = Temp::new();
            let mut b = binding_fixture();
            assert!(start_allowed(&temp.0, &b, 2000).is_ok());
            fs::write(temp.0.join("cleanup-requested"), b"cancel after last check").unwrap();
            assert!(start_allowed(&temp.0, &b, 2000).is_err());
            fs::remove_file(temp.0.join("cleanup-requested")).unwrap();
            assert!(start_allowed(&temp.0, &b, 541_000).is_err());
            assert_eq!(lifecycle_end(&b, 590_000).unwrap(), 601_000);
            b.armed_ms = None;
            assert_eq!(lifecycle_end(&b, 890_000).unwrap(), 900_000);
            // An intent without a complete result cannot become positive closure data.
            assert!(parse::<OriginalRun>(br#"{"runId":1}"#).is_err());
        }
        #[test]
        fn exact_typed_absence_requires_complete_same_identity_data() {
            let cid = "b".repeat(64);
            assert!(complete_absence(b"", &cid));
            assert!(complete_absence(
                format!("{}\n", json!("a".repeat(64))).as_bytes(),
                &cid
            ));
            for b in [
                json!(cid).to_string(),
                format!("{}\n", json!(cid)),
                "garbage\n".into(),
                "null\n".into(),
            ] {
                assert!(!complete_absence(b.as_bytes(), &cid));
            }
            let body = json!({"message":format!("No such container: {cid}")}).to_string();
            let good = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{body}",
                body.len()
            );
            assert!(typed_not_found(good.as_bytes(), &cid));
            for bad in [
                good.replace("404", "500"),
                good.replace("Content-Length:", "Transfer-Encoding:"),
                good.replace(&cid, &"a".repeat(64)),
                format!("{good}x"),
                good.replace("Content-Type:", "Content-Length:"),
            ] {
                assert!(!typed_not_found(bad.as_bytes(), &cid));
            }
        }
        #[test]
        fn original_stat_core_namespace_and_jvm_identity_are_not_pid_claims() {
            let mut fields = vec!["S".to_string(); 20];
            fields[19] = "1234".into();
            let stat = format!("42 (name with ) parentheses) {}", fields.join(" "));
            assert_eq!(process_start(stat.as_bytes()).unwrap(), (42, 1234));
            assert!(process_start(b"42 (partial").is_err());
            let status = "NoNewPrivs:\t1\nCapEff:\t0000000000000000\nSeccomp:\t2\nUid:\t65532\t65532\t65532\t65532\nGid:\t65532\t65532\t65532\t65532\n";
            assert!(core_zero(status, "Max core file size 0 0 bytes\n"));
            assert!(!core_zero(status, "Max core file size 0 unlimited bytes\n"));
            assert!(!core_zero(
                &status.replace("Seccomp:\t2", "Seccomp:\t0"),
                "Max core file size 0 0 bytes\n"
            ));
            let line = format!(
                "1000-2000 r-xp 0000 08:01 77 {INNER}/distribution/{RELEASE}/jbr/lib/server/libjvm.so\n"
            );
            assert!(mapped_jvm(&line, 0x801, 77).unwrap());
            assert!(mapped_jvm(&line, 0x801, 78).is_err());
            assert!(mapped_jvm(&(line.trim_end().to_owned() + " (deleted)\n"), 0x801, 77).is_err());
        }
        #[test]
        fn container_list_and_state_never_substitute_for_teardown() {
            let cid = "b".repeat(64);
            let v = json!({"Id":cid,"State":{"Running":false,"Restarting":false,"Dead":false,"Pid":0,"ExitCode":0,"StartedAt":"original","FinishedAt":"original"}});
            assert!(stopped(&v, &cid));
            let mut changed = v.clone();
            changed["State"]["Pid"] = json!(42);
            assert!(!stopped(&changed, &cid));
            assert!(!stopped(&v, &"a".repeat(64)));
            assert!(one_inspect(b"[]").is_err());
            assert!(one_inspect(b"[{},{}]").is_err());
            assert!(properties(b"ActiveState=active\nActiveState=inactive\n").is_err());
        }

        #[test]
        fn production_lifecycle_peer_stops_failed_preflight_and_preserves_order() {
            let mut events = Vec::new();
            assert!(
                capture_steps(|step| {
                    events.push(step);
                    if step == CaptureStep::Preflight {
                        Err("missing protected admission/adapter".into())
                    } else {
                        panic!("mutation after failed preflight")
                    }
                })
                .is_err()
            );
            assert_eq!(events, [CaptureStep::Preflight]);
            events.clear();
            capture_steps(|step| {
                events.push(step);
                Ok(())
            })
            .unwrap();
            assert_eq!(
                events,
                [
                    CaptureStep::Preflight,
                    CaptureStep::Prepare,
                    CaptureStep::Create,
                    CaptureStep::InstallCleanup,
                    CaptureStep::Inspect,
                    CaptureStep::Seal,
                    CaptureStep::Arm,
                    CaptureStep::Readback,
                    CaptureStep::Start,
                    CaptureStep::Observe
                ]
            );
            for fail in [
                CaptureStep::Create,
                CaptureStep::InstallCleanup,
                CaptureStep::Inspect,
                CaptureStep::Seal,
                CaptureStep::Arm,
                CaptureStep::Readback,
                CaptureStep::Start,
            ] {
                let mut seen = Vec::new();
                assert!(
                    capture_steps(|step| {
                        seen.push(step);
                        if step == fail {
                            Err("synthetic operation failure".into())
                        } else {
                            Ok(())
                        }
                    })
                    .is_err()
                );
                assert_eq!(seen.last(), Some(&fail));
                assert!(!seen.contains(&CaptureStep::Observe));
            }
            assert_eq!(cleanup_owner(false, false), CleanupOwner::None);
            assert_eq!(cleanup_owner(true, false), CleanupOwner::DirectPrearm);
            assert_eq!(cleanup_owner(true, true), CleanupOwner::Service);
        }
        #[test]
        fn pending_start_service_failure_and_missing_evidence_never_confirm() {
            let b = binding_fixture();
            let intent = json!({"containerId":b.container_id,"nonce":b.nonce,"bootId":b.boot_id})
                .to_string();
            let complete = json!({"containerId":b.container_id,"complete":true}).to_string();
            assert!(settled_start(intent.as_bytes(), complete.as_bytes(), &b));
            for result in [
                b"".as_slice(),
                b"{",
                br#"{"containerId":"wrong","complete":true}"#,
            ] {
                assert!(!settled_start(intent.as_bytes(), result, &b));
            }
            let incomplete = json!({"containerId":b.container_id,"complete":false}).to_string();
            assert!(!settled_start(intent.as_bytes(), incomplete.as_bytes(), &b));
            for field in 0..5 {
                let mut facts = ClosureFacts {
                    proofs: true,
                    start_settled: true,
                    observer_stopped: true,
                    deadline_valid: true,
                    evidence_complete: true,
                };
                assert!(facts.confirmed());
                match field {
                    0 => facts.proofs = false,
                    1 => facts.start_settled = false,
                    2 => facts.observer_stopped = false,
                    3 => facts.deadline_valid = false,
                    _ => facts.evidence_complete = false,
                };
                assert!(!facts.confirmed());
            }
            let temp = Temp::new();
            let mut attempted = false;
            assert!(
                dispatch_despite_latch_error(Err("ENOSPC".into()), || {
                    attempted = true;
                    Ok(())
                })
                .is_err()
            );
            assert!(attempted);
            assert!(!temp.0.join("cleanup-requested").exists());
        }
        #[test]
        fn cleanup_interruption_keeps_original_deadline_and_reserves_actions() {
            let b = binding_fixture();
            let clock = CleanupClock {
                nonce: b.nonce.clone(),
                container_id: b.container_id.clone(),
                boot_id: b.boot_id.clone(),
                first_ms: 2000,
                end_ms: 62_000,
            };
            assert_eq!(validate_cleanup_clock(clock.clone(), &b).unwrap(), 62_000);
            let resumed: CleanupClock = parse(&serde_json::to_vec(&clock).unwrap()).unwrap();
            assert_eq!(validate_cleanup_clock(resumed, &b).unwrap(), 62_000);
            assert!(
                validate_cleanup_clock(
                    CleanupClock {
                        nonce: b.nonce.clone(),
                        container_id: b.container_id.clone(),
                        boot_id: b.boot_id.clone(),
                        first_ms: 2000,
                        end_ms: 100_000,
                    },
                    &b
                )
                .is_err()
            );
            let temp = Temp::new();
            let mut e = Evidence::preflight();
            let (end, trusted) = cleanup_clock(&temp.0, &b, &mut e, 63_000, true);
            assert_eq!(end, 63_000);
            assert!(!trusted);
            assert!(!temp.0.join("cleanup-begun.json").exists());
        }
        #[test]
        fn systemctl_spans_preserve_actual_microseconds_and_watchdog_bound() {
            for (span, expected) in [
                ("12min 34.123456s", 754_123_456),
                ("1d 2h 3min 4s 5ms 6us", 93_784_005_006),
                ("1.001ms", 1001),
                ("10min", 600_000_000),
                ("1µs", 1),
            ] {
                assert_eq!(systemctl_usec(span).unwrap(), expected);
            }
            for span in [
                "",
                "0",
                "0s",
                "infinity",
                "1s 1s",
                "1us 1ms",
                "1s ",
                " 1s",
                "1s\n",
                "1.s",
                ".1s",
                "1.0000001s",
                "0.1us",
                "1ss",
                "1s  2ms",
                "18446744073709551615d",
                "600000000",
            ] {
                assert!(systemctl_usec(span).is_err(), "{span:?}");
            }
            assert!(watchdog_within_bound(541_000_000, 1000, 2000));
            assert!(!watchdog_within_bound(541_000_001, 1000, 2000));
            assert!(!watchdog_within_bound(2_000_999, 1000, 2000));
            assert!(!watchdog_within_bound(u64::MAX, u64::MAX, 0));
        }
        #[test]
        fn production_cleanup_peer_emits_exact_cid_reserves_and_one_fallback() {
            struct Peer {
                now: u64,
                slow: bool,
                fail_kill: bool,
                fail_remove: bool,
                proof: bool,
                calls: Vec<(PathBuf, Vec<String>, u64, bool)>,
                proofs: Vec<u64>,
            }
            let b = binding_fixture();
            let run = |peer: &mut Peer| {
                contain_container(
                    peer,
                    &b,
                    CleanupBudget { end: 60_000 },
                    |peer, program, argv, end| {
                        let spawned = peer.now < end;
                        peer.calls
                            .push((program.into(), argv.to_vec(), end, spawned));
                        if !spawned {
                            return Err("original peer deadline expired".into());
                        }
                        if peer.slow {
                            peer.now = (peer.now + 10_000).min(end);
                        }
                        let mut out = Output {
                            stdout: Vec::new(),
                            stderr: Vec::new(),
                            code: Some(0),
                            signal: None,
                            eof: [true, true],
                            timeout: false,
                            truncated: false,
                            reaped: true,
                        };
                        match argv[0].as_str() {
                            "container" => {
                                let running = peer.calls.len() == 1;
                                out.stdout = serde_json::to_vec(&json!([{
                                    "Id":b.container_id,
                                    "State":{"Running":running,"Restarting":false,
                                        "Dead":false,"Pid":0,"ExitCode":7,
                                        "StartedAt":"original","FinishedAt":"stopped"}
                                }]))
                                .unwrap();
                            }
                            "kill" if peer.fail_kill => out.code = Some(1),
                            "wait" => out.stdout = b"7\n".to_vec(),
                            "rm" if peer.fail_remove && argv.len() == 2 => {
                                out.code = Some(1);
                            }
                            "rm" => out.stdout = format!("{}\n", b.container_id).into_bytes(),
                            _ => {}
                        }
                        Ok(out)
                    },
                    |peer, end| {
                        peer.proofs.push(end);
                        peer.proof && peer.now < end
                    },
                )
            };
            for (slow, fail_kill, fail_remove, proof, expected) in [
                (false, false, false, true, true),
                (false, true, false, true, false),
                (false, false, true, true, false),
                (false, false, false, false, false),
                (true, false, true, false, false),
            ] {
                let mut peer = Peer {
                    now: if slow { 30_000 } else { 0 },
                    slow,
                    fail_kill,
                    fail_remove,
                    proof,
                    calls: Vec::new(),
                    proofs: Vec::new(),
                };
                assert_eq!(run(&mut peer), expected);
                let mut expected_argv = vec![
                    args(&["container", "inspect", &b.container_id]),
                    args(&["kill", "--signal=KILL", &b.container_id]),
                    args(&["wait", &b.container_id]),
                    args(&["container", "inspect", &b.container_id]),
                    args(&["rm", &b.container_id]),
                ];
                let mut deadlines = vec![30_000, 40_000, 40_000, 40_000, 50_000];
                if fail_remove {
                    expected_argv.push(args(&["rm", "--force", &b.container_id]));
                    deadlines.push(60_000);
                }
                assert_eq!(peer.calls.len(), expected_argv.len());
                for ((program, argv, end, _), (expected, deadline)) in peer
                    .calls
                    .iter()
                    .zip(expected_argv.iter().zip(deadlines.iter()))
                {
                    assert_eq!(program, &b.docker);
                    assert_eq!(argv, expected);
                    assert_eq!(end, deadline);
                }
                if slow {
                    assert_eq!(peer.now, 60_000);
                    assert_eq!(
                        peer.calls.iter().map(|c| c.3).collect::<Vec<_>>(),
                        [false, true, false, false, true, true]
                    );
                } else {
                    assert_eq!(peer.proofs, [40_000]);
                }
            }
        }
        #[test]
        fn actual_argv_parent_exec_and_cgroup_replacement_are_refused() {
            let expected = launcher_argv();
            let mut bytes = expected.join("\0").into_bytes();
            bytes.push(0);
            assert!(actual_argv(&bytes, &expected));
            assert!(!actual_argv(&bytes[..bytes.len() - 1], &expected));
            let mut wrong = bytes.clone();
            wrong.extend_from_slice(b"--extra\0");
            assert!(!actual_argv(&wrong, &expected));
            let mut fields = vec!["S".to_string(); 20];
            fields[1] = "17".into();
            fields[19] = "90".into();
            let stat = format!("42 (comm) {}", fields.join(" "));
            assert_eq!(parent_pid(stat.as_bytes()).unwrap(), 17);
            assert_eq!(process_start(stat.as_bytes()).unwrap(), (42, 90));
            assert!(!object_matches((1, 2), (1, 3)));
            assert!(!object_matches((2, 2), (1, 2)));
            let mut visited = 255;
            assert!(queue_directory(&mut visited, 1, 0, 100).is_err());
            assert!(queue_directory(&mut 1, 0, 100, 100).is_err());
        }
        #[test]
        fn hidden_writable_partial_mount_tables_cannot_attest() {
            let roots = ["fixture", "distribution", "tool", "preflight"]
                .map(|s| format!("{INNER}/{s}"))
                .to_vec();
            let mut text = "1 0 0:1 / / ro - overlay overlay ro\n".to_string();
            for (i, p) in roots.iter().enumerate() {
                text.push_str(&format!("{} 1 0:2 / {p} ro - tmpfs tmpfs ro\n", i + 2));
            }
            assert!(mount_topology(text.as_bytes(), &roots, true).is_ok());
            assert!(
                mount_topology(
                    text.replace(&format!("{} ro", roots[0]), &format!("{} rw", roots[0]))
                        .as_bytes(),
                    &roots,
                    true
                )
                .is_err()
            );
            assert!(
                mount_topology(
                    format!("{text}8 1 0:3 / {}/hidden ro - tmpfs tmpfs ro\n", roots[0]).as_bytes(),
                    &roots,
                    true
                )
                .is_err()
            );
            assert!(mount_topology(text.trim_end().as_bytes(), &roots, true).is_err());
        }
        #[test]
        fn original_api_binding_rejects_replay_and_wrong_attempt() {
            let run = OriginalRun {
                repository: "anilbeesetti/ktlsp".into(),
                event: "workflow_dispatch".into(),
                attempt: 1,
                run_id: 1,
                job_id: 2,
                suite_id: 3,
                source_head: "a".repeat(40),
                source_tree: "b".repeat(40),
            };
            let value = json!({"id":1,"run_attempt":1,"status":"in_progress","conclusion":null,"event":"workflow_dispatch","head_sha":run.source_head,"check_suite_id":3,"repository":{"full_name":"anilbeesetti/ktlsp"},"path":".github/workflows/t045-pr174-official-symbols.yml"});
            let job =
                json!({"id":2,"run_id":1,"status":"in_progress","conclusion":null,"head_sha":run.source_head,"labels":["ubuntu-24.04"]});
            let commit = json!({"sha":run.source_head,"tree":{"sha":run.source_tree}});
            assert!(api_identity(&value, &job, &commit, &run).is_ok());
            let mut bad = value.clone();
            bad["run_attempt"] = json!(2);
            assert!(api_identity(&bad, &job, &commit, &run).is_err());
            let mut bad = job.clone();
            bad["run_id"] = json!(9);
            assert!(api_identity(&value, &bad, &commit, &run).is_err());
            let mut completed = value.clone();
            completed["status"] = json!("completed");
            completed["conclusion"] = json!("success");
            assert!(api_identity(&completed, &job, &commit, &run).is_err());
            let mut completed_job = job;
            completed_job["status"] = json!("completed");
            completed_job["conclusion"] = json!("success");
            assert!(api_identity(&value, &completed_job, &commit, &run).is_err());
        }
    }
}
fn main() -> std::process::ExitCode {
    #[cfg(unix)]
    let result = host::entry();
    #[cfg(not(unix))]
    let result: Result<serde_json::Value, String> = Err("host execution is Linux-only".into());
    match result {
        Ok(v) => {
            println!("{v}");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("PR174 host refusal/failure: {e}");
            std::process::ExitCode::from(2)
        }
    }
}

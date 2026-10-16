//! Consumer of the protected original-job host START handoff, not an authority producer.
#[cfg(any(test, all(target_os = "linux", target_arch = "x86_64")))]
use super::{Pr174Identity, Pr174Run};
#[cfg(any(test, all(target_os = "linux", target_arch = "x86_64")))]
use serde::Deserialize;
use serde_json::Value;
#[cfg(any(test, all(target_os = "linux", target_arch = "x86_64")))]
use sha2::{Digest, Sha256};
use std::ffi::OsString;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const ROOT: &str = "/tmp/ktlsp-t045-pr174-host";
#[cfg(any(test, all(target_os = "linux", target_arch = "x86_64")))]
const CONTRACT: &str = "65eb16c5bd73c99fe7fc000fddf0242bf6abab09283676612edf5ce9f285b61d";
#[cfg(any(test, all(target_os = "linux", target_arch = "x86_64")))]
const AUTHORITY: &str = "protected original-job host START gate; adapter consistency only; receiver API authentication required";
const ARGUMENTS: [&str; 8] = [
    "--identity",
    "/tmp/ktlsp-t045-pr174-host/preflight/identity.json",
    "--preflight",
    "/tmp/ktlsp-t045-pr174-host/preflight/host-preflight.json",
    "--authorization",
    "/tmp/ktlsp-t045-pr174-host/preflight/launch-authorization.json",
    "--fixture-directory",
    "/tmp/ktlsp-t045-pr174-host/fixture",
];

#[cfg(any(test, all(target_os = "linux", target_arch = "x86_64")))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Authorization {
    schema_version: u8,
    root_authorization_sha256: String,
    contract_sha256: String,
    identity: Pr174Run,
    preflight_sha256: String,
    boot_id: String,
    daemon_id: String,
    authority: String,
}
#[cfg(any(test, all(target_os = "linux", target_arch = "x86_64")))]
fn hex(s: &str, length: usize) -> bool {
    s.len() == length
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
#[cfg(any(test, all(target_os = "linux", target_arch = "x86_64")))]
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
#[cfg(any(test, all(target_os = "linux", target_arch = "x86_64")))]
fn authorization(
    identity: &Pr174Identity,
    preflight: &[u8],
    bytes: &[u8],
) -> Result<Value, String> {
    if bytes.len() > 16_384 {
        return Err("authorization byte limit".into());
    }
    let authorization: Authorization =
        serde_json::from_slice(bytes).map_err(|e| format!("authorization: {e}"))?;
    identity.validate()?;
    let a = &authorization;
    let boot = a.boot_id.as_bytes();
    if a.schema_version != 1
        || a.contract_sha256 != CONTRACT
        || a.identity != identity.original_run
        || !hex(&a.root_authorization_sha256, 64)
        || !hex(&a.preflight_sha256, 64)
        || a.preflight_sha256 != digest(preflight)
        || boot.len() != 36
        || !boot.iter().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                *b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(b)
            }
        })
        || a.daemon_id.is_empty()
        || a.daemon_id.len() > 128
        || a.daemon_id.bytes().any(|b| b.is_ascii_control())
        || a.authority != AUTHORITY
    {
        return Err("protected authorization identity, digest or authority mismatch".into());
    }
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}

/// Accept exactly the host producer's four fixed pairs. No alternate command,
/// output path, environment, or caller-provided authorization policy exists.
pub fn record_pr174(args: &[OsString]) -> Result<Value, String> {
    if args.len() != ARGUMENTS.len() || args.iter().zip(ARGUMENTS).any(|(a, b)| a != b) {
        return Err("expected exactly the four fixed protected-host argument pairs".into());
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        linux::run()
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        Err(
            "official PR174 requires the separately admitted Linux amd64 host; no output or launch"
                .into(),
        )
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux {
    use super::*;
    use crate::recorder::{
        CaptureContext, CaptureLimits, record_with_context, validate_pr174_inputs,
    };
    use std::{
        ffi::CString,
        fs::{self, File},
        io::Read,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::MetadataExt,
        },
        path::{Path, PathBuf},
    };
    unsafe extern "C" {
        fn openat(dirfd: i32, path: *const std::ffi::c_char, flags: i32, ...) -> i32;
        fn getuid() -> u32;
        fn geteuid() -> u32;
    }
    const NOFOLLOW: i32 = 0x20000;
    const DIRECTORY: i32 = 0x10000;
    const NONBLOCK: i32 = 0x800;
    const CLOEXEC: i32 = 0x80000;

    fn bounded(mut file: &File, cap: usize) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        (&mut file)
            .take(cap as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > cap {
            return Err("protected input byte limit".into());
        }
        Ok(bytes)
    }
    fn proc_bytes(path: &Path, cap: usize) -> Result<Vec<u8>, String> {
        bounded(&File::open(path).map_err(|e| e.to_string())?, cap)
    }
    fn mount_id(file: &File) -> Result<u64, String> {
        let bytes = proc_bytes(
            &PathBuf::from(format!("/proc/self/fdinfo/{}", file.as_raw_fd())),
            4096,
        )?;
        let text = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
        let mut ids = text.lines().filter_map(|s| s.strip_prefix("mnt_id:\t"));
        let id = ids
            .next()
            .ok_or("held input mount id missing")?
            .parse()
            .map_err(|_| "held input mount id malformed")?;
        if ids.next().is_some() {
            return Err("duplicate held mount id".into());
        }
        Ok(id)
    }
    struct Mounts(String);
    impl Mounts {
        fn read() -> Result<Self, String> {
            String::from_utf8(proc_bytes(Path::new("/proc/self/mountinfo"), 262_144)?)
                .map(Self)
                .map_err(|e| e.to_string())
        }
        fn check(&self, id: u64, target: &str, readonly: bool) -> Result<(), String> {
            let key = id.to_string();
            let mut matches = self
                .0
                .lines()
                .filter(|s| s.split_whitespace().next() == Some(key.as_str()));
            let row = matches.next().ok_or("held input mount absent")?;
            if matches.next().is_some() {
                return Err("duplicate mount identity".into());
            }
            let (left, right) = row.split_once(" - ").ok_or("mount grammar")?;
            let fields: Vec<_> = left.split_whitespace().collect();
            let info: Vec<_> = right.split_whitespace().collect();
            if fields.len() < 6
                || info.len() != 3
                || fields[4] != target
                || !fields[5]
                    .split(',')
                    .any(|s| s == if readonly { "ro" } else { "rw" })
                || fields[6..].iter().any(|s| {
                    s.starts_with("shared:")
                        || s.starts_with("master:")
                        || s.starts_with("propagate_from:")
                })
                || (!readonly
                    && (info[0] != "tmpfs"
                        || !["size=524288k", "nr_inodes=16384"]
                            .iter()
                            .all(|required| info[2].split(',').any(|s| s == *required))
                        || !["nosuid", "nodev"]
                            .iter()
                            .all(|required| fields[5].split(',').any(|s| s == *required))))
            {
                return Err("protected input effective mount mismatch".into());
            }
            // No hidden child bind can substitute any input underneath a sealed mount.
            let prefix = format!("{target}/");
            if readonly
                && self.0.lines().any(|s| {
                    s.split_whitespace()
                        .nth(4)
                        .is_some_and(|p| p.starts_with(&prefix))
                })
            {
                return Err("nested mount beneath protected input".into());
            }
            Ok(())
        }
        fn image_root(&self, root: u64, tmp: u64) -> Result<(), String> {
            if root != tmp {
                return Err("fixed /tmp is not covered by read-only image root".into());
            }
            let key = root.to_string();
            let rows: Vec<_> = self
                .0
                .lines()
                .filter(|s| s.split_whitespace().next() == Some(key.as_str()))
                .collect();
            if rows.len() != 1 {
                return Err("read-only image root identity missing/duplicate".into());
            }
            let (left, _) = rows[0]
                .split_once(" - ")
                .ok_or("image root mount grammar")?;
            let fields: Vec<_> = left.split_whitespace().collect();
            if fields.len() < 6 || fields[4] != "/" || !fields[5].split(',').any(|s| s == "ro") {
                return Err("image root must be effectively read-only".into());
            }
            // Only the producer's bounded state and four private input roots may
            // cover any part of the fixed /tmp launch/output namespace.
            for path in self.0.lines().filter_map(|s| s.split_whitespace().nth(4)) {
                if path.starts_with("/tmp/")
                    && path != ROOT
                    && !["preflight", "fixture", "distribution", "tool"]
                        .iter()
                        .any(|name| path == format!("{ROOT}/{name}"))
                {
                    return Err("unexpected mount in fixed /tmp namespace".into());
                }
            }
            Ok(())
        }
    }
    fn child(parent: &File, name: &str, directory: bool) -> Result<File, String> {
        let name = CString::new(name).map_err(|_| "path NUL")?;
        // SAFETY: parent remains held; each single component rejects symlinks. Nonblocking
        // prevents a substituted FIFO/device from blocking before metadata refusal.
        let fd = unsafe {
            openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                NOFOLLOW | NONBLOCK | CLOEXEC | if directory { DIRECTORY } else { 0 },
            )
        };
        if fd < 0 {
            return Err(format!(
                "protected open: {}",
                std::io::Error::last_os_error()
            ));
        }
        // SAFETY: openat returned a new owned descriptor.
        let file = unsafe { File::from_raw_fd(fd) };
        let m = file.metadata().map_err(|e| e.to_string())?;
        if (directory && !m.is_dir()) || (!directory && !m.is_file()) {
            return Err("protected input type".into());
        }
        Ok(file)
    }
    fn root_owned(file: &File, directory: bool) -> Result<(), String> {
        let m = file.metadata().map_err(|e| e.to_string())?;
        if m.uid() != 0 || m.mode() & 0o022 != 0 || (!directory && m.nlink() != 1) {
            return Err("protected input root custody or hardlink mismatch".into());
        }
        Ok(())
    }
    struct Input {
        file: File,
        stamp: (u64, u64, u64, i64, i64),
        mount: u64,
    }
    impl Input {
        fn open(parent: &File, name: &str, mount: u64) -> Result<Self, String> {
            let file = child(parent, name, false)?;
            root_owned(&file, false)?;
            if mount_id(&file)? != mount {
                return Err("protected file mount substitution".into());
            }
            let stamp = Self::stamp(&file)?;
            Ok(Self { file, stamp, mount })
        }
        fn stamp(file: &File) -> Result<(u64, u64, u64, i64, i64), String> {
            let m = file.metadata().map_err(|e| e.to_string())?;
            Ok((m.dev(), m.ino(), m.len(), m.ctime(), m.ctime_nsec()))
        }
        fn unchanged(&self) -> Result<(), String> {
            if Self::stamp(&self.file)? != self.stamp || mount_id(&self.file)? != self.mount {
                return Err("protected input changed during validation".into());
            }
            Ok(())
        }
        fn bytes(&self, cap: usize) -> Result<Vec<u8>, String> {
            if self.stamp.2 > cap as u64 {
                return Err("protected input byte limit".into());
            }
            let bytes = bounded(&self.file, cap)?;
            self.unchanged()?;
            Ok(bytes)
        }
        fn hash(&self, size: u64, expected: &str) -> Result<(), String> {
            if self.stamp.2 != size {
                return Err("materialized pinned input size mismatch".into());
            }
            let mut input = (&self.file).take(size + 1);
            let mut sha = Sha256::new();
            let mut buffer = [0; 8192];
            let mut read = 0;
            loop {
                let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                read += n as u64;
                sha.update(&buffer[..n]);
            }
            self.unchanged()?;
            if read != size || format!("{:x}", sha.finalize()) != expected {
                return Err("materialized pinned input SHA256 mismatch".into());
            }
            Ok(())
        }
    }
    fn directory(parent: &File, name: &str) -> Result<File, String> {
        let file = child(parent, name, true)?;
        root_owned(&file, true)?;
        Ok(file)
    }
    pub(super) fn run() -> Result<Value, String> {
        // SAFETY: getuid/geteuid have no arguments or memory effects.
        if unsafe { getuid() } != 65532 || unsafe { geteuid() } != 65532 {
            return Err("official adapter requires unprivileged UID65532".into());
        }
        let root = File::open("/").map_err(|e| e.to_string())?;
        root_owned(&root, true)?;
        let tmp = child(&root, "tmp", true)?;
        let meta = tmp.metadata().map_err(|e| e.to_string())?;
        if meta.uid() != 0 || meta.mode() & 0o1000 == 0 {
            return Err("fixed /tmp root sticky custody".into());
        }
        let state = child(&tmp, "ktlsp-t045-pr174-host", true)?;
        let meta = state.metadata().map_err(|e| e.to_string())?;
        if meta.uid() != 65532 || meta.gid() != 65532 || meta.mode() & 0o7777 != 0o755 {
            return Err("fixed bounded state custody".into());
        }
        let mounts = Mounts::read()?;
        mounts.image_root(mount_id(&root)?, mount_id(&tmp)?)?;
        mounts.check(mount_id(&state)?, ROOT, false)?;
        let mut held_dirs = vec![root, tmp, state];
        let mut mount_bindings = Vec::new();
        for name in ["preflight", "fixture", "distribution", "tool"] {
            let dir = directory(&held_dirs[2], name)?;
            let id = mount_id(&dir)?;
            let path = format!("{ROOT}/{name}");
            mounts.check(id, &path, true)?;
            if mount_bindings.iter().any(|(existing, _)| *existing == id) {
                return Err("protected input roots must have distinct mount identities".into());
            }
            mount_bindings.push((id, path));
            held_dirs.push(dir);
        }
        let preflight = &held_dirs[3];
        let id = mount_bindings[0].0;
        let identity = Input::open(preflight, "identity.json", id)?;
        let flight = Input::open(preflight, "host-preflight.json", id)?;
        let auth = Input::open(preflight, "launch-authorization.json", id)?;
        let identity_bytes = identity.bytes(16_384)?;
        let flight_bytes = flight.bytes(16_384)?;
        let auth_bytes = auth.bytes(16_384)?;
        let parsed: Pr174Identity =
            serde_json::from_slice(&identity_bytes).map_err(|e| e.to_string())?;
        let authorization = authorization(&parsed, &flight_bytes, &auth_bytes)?;
        let fixture = Path::new(ARGUMENTS[7]);
        validate_pr174_inputs(Path::new(ARGUMENTS[1]), Path::new(ARGUMENTS[3]), fixture)?;
        let mut held_inputs = vec![identity, flight, auth];
        for (name, expected) in [
            (
                "script.json",
                include_bytes!("../../fixtures/official-pr174-symbols/script.json").as_slice(),
            ),
            (
                "declarations.kt",
                include_bytes!("../../fixtures/official-pr174-symbols/declarations.kt").as_slice(),
            ),
            (
                "error-node.kt",
                include_bytes!("../../fixtures/official-pr174-symbols/error-node.kt").as_slice(),
            ),
        ] {
            let input = Input::open(&held_dirs[4], name, mount_bindings[1].0)?;
            if input.bytes(expected.len())? != expected {
                return Err("held fixture bytes mismatch".into());
            }
            held_inputs.push(input);
        }
        let dist = directory(&held_dirs[5], "kotlin-server-263.6379.0")?;
        let dist_mount = mount_bindings[2].0;
        // Every launch-critical regular file is read from held, nonsymlink directory
        // components on the same sealed read-only distribution mount.
        for (path, size, sha) in [
            (
                "kotlin-lsp.sh",
                441,
                "8d994fea993aa9f995e715369ef727cec15920f4502c1b326f3869fb2d1bdb43",
            ),
            (
                "bin/intellij-server",
                1073600,
                "adf02baec622b4fcd1e7db3ec96f7c7b3309d5ab59e170e60260d7511222878e",
            ),
            (
                "bin/intellij-server.vmoptions",
                873,
                "e2bf45f82006a58fdf5a5ae87d36f8a031d89b2c7eda743a29d6ca7a1381c1b4",
            ),
            (
                "jbr/bin/java",
                23456,
                "eeeaadc9662c5faed35be8ccfdc90b74e1de37ad672d219673aff294276072fc",
            ),
            (
                "jbr/release",
                1051,
                "5fb59d126d35d14775f7ab86e1c60e1624ff9e236f2f89f3752a869263f498bd",
            ),
            (
                "jbr/lib/server/libjvm.so",
                28937152,
                "ba231c43718732491a965b742b54a1f9206c2415c61502311c0172b88d6c79d5",
            ),
            (
                "jbr/lib/libjli.so",
                79096,
                "fce140d52f8945dfe0e16fa91313f9b5629b68fbeade937771951f1e69d49b7f",
            ),
            (
                "product-info.json",
                15038,
                "d7ffe547c1529603396ba9243aec138488ea1a2f7838173bd028b1d4ae454de4",
            ),
        ] {
            let mut parts = path.split('/').peekable();
            let mut parent = None;
            while let Some(part) = parts.next() {
                if parts.peek().is_none() {
                    let input = Input::open(parent.as_ref().unwrap_or(&dist), part, dist_mount)?;
                    input.hash(size, sha)?;
                    held_inputs.push(input);
                } else {
                    parent = Some(directory(parent.as_ref().unwrap_or(&dist), part)?);
                }
            }
        }
        let fresh_mounts = Mounts::read()?;
        fresh_mounts.image_root(mount_id(&held_dirs[0])?, mount_id(&held_dirs[1])?)?;
        fresh_mounts.check(mount_id(&held_dirs[2])?, ROOT, false)?;
        for (id, path) in &mount_bindings {
            fresh_mounts.check(*id, path, true)?;
        }
        for input in &held_inputs {
            input.unchanged()?;
        }
        let output = Path::new("/tmp/ktlsp-t045-pr174-host/capture");
        match fs::symlink_metadata(output) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            _ => return Err("official output must be absent".into()),
        }
        let script: Vec<Value> = serde_json::from_slice(include_bytes!(
            "../../fixtures/official-pr174-symbols/script.json"
        ))
        .map_err(|e| e.to_string())?;
        let command = vec![
            format!("{ROOT}/distribution/kotlin-server-263.6379.0/bin/intellij-server"),
            "--stdio".into(),
            "--data-sharing=none".into(),
            "--region=apac".into(),
            format!("--system-path={ROOT}/capture/system"),
        ];
        let context = CaptureContext::Official {
            identity: serde_json::from_slice(&identity_bytes).map_err(|e| e.to_string())?,
            authorization,
        };
        record_with_context(
            &script,
            &command,
            output,
            CaptureLimits {
                phase_ms: 60_000,
                session_ms: 300_000,
                cleanup_ms: 5_000,
                ..CaptureLimits::default()
            },
            &context,
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::unix::fs::{PermissionsExt, symlink};
        #[test]
        fn held_open_refuses_symlink_special_directory_and_retains_original_inode() {
            let path =
                std::env::temp_dir().join(format!("ktlsp-official-custody-{}", std::process::id()));
            fs::create_dir(&path).unwrap();
            fs::write(path.join("regular"), b"original").unwrap();
            fs::create_dir(path.join("directory")).unwrap();
            symlink("regular", path.join("symlink")).unwrap();
            let parent = File::open(&path).unwrap();
            assert!(child(&parent, "symlink", false).is_err());
            assert!(child(&parent, "directory", false).is_err());
            let dev = File::open("/dev").unwrap();
            assert!(child(&dev, "null", false).is_err());
            let original = child(&parent, "regular", false).unwrap();
            let stamp = Input::stamp(&original).unwrap();
            fs::rename(path.join("regular"), path.join("moved")).unwrap();
            fs::write(path.join("regular"), b"replacement").unwrap();
            assert_eq!(bounded(&original, 8).unwrap(), b"original");
            assert_ne!(
                Input::stamp(&child(&parent, "regular", false).unwrap()).unwrap(),
                stamp
            );
            assert!(bounded(&File::open(path.join("regular")).unwrap(), 8).is_err());
            fs::set_permissions(path.join("regular"), fs::Permissions::from_mode(0o666)).unwrap();
            assert!(root_owned(&File::open(path.join("regular")).unwrap(), false).is_err());
            fs::hard_link(path.join("moved"), path.join("linked")).unwrap();
            assert!(root_owned(&original, false).is_err());
            fs::remove_dir_all(path).unwrap();
        }
        #[test]
        fn exact_mount_topology_refuses_writable_shared_hidden_or_substituted_inputs() {
            let base = format!(
                "1 0 0:1 / / ro - overlay overlay rw\n2 1 0:2 / {ROOT} rw,nosuid,nodev - tmpfs tmpfs rw,size=524288k,nr_inodes=16384\n3 2 0:3 /sealed {ROOT}/preflight ro - ext4 disk rw\n"
            );
            let mounts = Mounts(base.clone());
            assert!(mounts.image_root(1, 1).is_ok());
            assert!(mounts.check(2, ROOT, false).is_ok());
            assert!(mounts.check(3, &format!("{ROOT}/preflight"), true).is_ok());
            assert!(mounts.image_root(1, 2).is_err());
            for changed in [
                base.replace("/ / ro", "/ / rw"),
                format!("{base}4 2 0:4 / /tmp/alternate rw - tmpfs tmpfs rw\n"),
            ] {
                assert!(Mounts(changed).image_root(1, 1).is_err());
            }
            for changed in [
                base.replace("/preflight ro", "/preflight rw"),
                base.replace("/preflight ro", "/preflight ro shared:1"),
                format!("{base}4 3 0:4 / {ROOT}/preflight/identity.json ro - ext4 disk rw\n"),
                format!("{base}3 2 0:3 /sealed {ROOT}/preflight ro - ext4 disk rw\n"),
            ] {
                assert!(
                    Mounts(changed)
                        .check(3, &format!("{ROOT}/preflight"), true)
                        .is_err()
                );
            }
            for changed in [
                base.replace("size=524288k", "size=1048576k"),
                base.replace("nr_inodes=16384", "nr_inodes=32768"),
                base.replace("rw,nosuid,nodev", "rw,nodev"),
            ] {
                assert!(Mounts(changed).check(2, ROOT, false).is_err());
            }
            assert!(mounts.check(4, &format!("{ROOT}/preflight"), true).is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn identity() -> (Pr174Identity, Value) {
        let value = json!({"schemaVersion":1,"contractSha256":CONTRACT,"sourceBaseline":"b4fa25286a5fa0d7600b0b0420a7bcae6d520edc",
            "originalRun":{"repository":"anilbeesetti/ktlsp","event":"workflow_dispatch","attempt":1,"runId":1,"jobId":2,"suiteId":3,
                "sourceHead":"1".repeat(40),"sourceTree":"2".repeat(40),"workflowSha256":"3".repeat(64),"containerId":"4".repeat(64),"imageSha256":"5".repeat(64),"nonce":"6".repeat(64)}});
        (serde_json::from_value(value.clone()).unwrap(), value)
    }
    fn auth(identity: &Value, preflight: &[u8]) -> Value {
        json!({"schemaVersion":1,"rootAuthorizationSha256":"a".repeat(64),"contractSha256":CONTRACT,
            "identity":identity["originalRun"],"preflightSha256":digest(preflight),"bootId":"12345678-1234-1234-1234-123456789abc",
            "daemonId":"synthetic-daemon-not-evidence","authority":AUTHORITY})
    }
    #[test]
    fn authorization_matches_flattened_original_run_and_exact_raw_bytes() {
        let (identity, value) = identity();
        let raw = b"{\"preflight\": true}\n";
        let a = auth(&value, raw);
        assert!(authorization(&identity, raw, a.to_string().as_bytes()).is_ok());
        assert!(
            authorization(&identity, b"{\"preflight\":true}", a.to_string().as_bytes()).is_err()
        );
        let mut wrapped = a.clone();
        wrapped["identity"] = value;
        assert!(authorization(&identity, raw, wrapped.to_string().as_bytes()).is_err());
        for (key, wrong) in [
            ("schemaVersion", json!(2)),
            ("rootAuthorizationSha256", json!("A".repeat(64))),
            ("contractSha256", json!("0".repeat(64))),
            ("bootId", json!("synthetic")),
            ("daemonId", json!("")),
            ("authority", json!("self-authorized")),
        ] {
            let mut bad = a.clone();
            bad[key] = wrong;
            assert!(
                authorization(&identity, raw, bad.to_string().as_bytes()).is_err(),
                "{key}"
            );
        }
        let mut bad = a.clone();
        bad["identity"]["nonce"] = json!("7".repeat(64));
        assert!(authorization(&identity, raw, bad.to_string().as_bytes()).is_err());
    }
    #[test]
    fn authorization_rejects_unknown_duplicate_trailing_and_oversized_inputs() {
        let (identity, value) = identity();
        let raw = b"{}";
        let a = auth(&value, raw).to_string();
        for bad in [
            format!("{{\"unknown\":true,{}", &a[1..]),
            format!("{{\"schemaVersion\":1,{}", &a[1..]),
            format!("{a} null"),
            " ".repeat(16_385),
        ] {
            assert!(authorization(&identity, raw, bad.as_bytes()).is_err());
        }
    }
    #[test]
    fn production_cli_has_no_command_output_or_authorization_bypass() {
        let exact: Vec<_> = ARGUMENTS.iter().map(OsString::from).collect();
        for args in [
            Vec::new(),
            exact[..6].to_vec(),
            {
                let mut a = exact.clone();
                a.push("--synthetic-only".into());
                a
            },
            {
                let mut a = exact.clone();
                a[1] = "/tmp/caller.json".into();
                a
            },
            {
                let mut a = exact.clone();
                a.swap(0, 2);
                a
            },
        ] {
            assert!(record_pr174(&args).unwrap_err().contains("four fixed"));
        }
    }
}

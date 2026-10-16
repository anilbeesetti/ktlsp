//! Child-only PR174 evidence custody. No unsupported-kernel or setup downgrade.
//!
//! Independently authored from the Linux v6.12 UAPI (not the sample sandbox):
//! https://raw.githubusercontent.com/torvalds/linux/v6.12/include/uapi/linux/landlock.h
//! https://raw.githubusercontent.com/torvalds/linux/v6.12/Documentation/userspace-api/landlock.rst
//! Landlock does not restrict chmod/chown/xattrs/utime. Metadata denial of service
//! must remain a failed observation; this policy only denies the handled actions.

use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    process::{Child, Command, Stdio},
};

const SCHEMA_VERSION: u32 = 1;
const PROFILE_ID: &str = "pr174-evidence-custody-landlock-v1";
const MINIMUM_ABI: u32 = 6;
const WRITE_LEAVES: [&str; 6] = ["home", "config", "system", "log", "tmp", "workspace"];
const WRITE_FILE: u64 = 1 << 1;
const REMOVE_DIR: u64 = 1 << 4;
const REMOVE_FILE: u64 = 1 << 5;
const MAKE_CHAR: u64 = 1 << 6;
const MAKE_DIR: u64 = 1 << 7;
const MAKE_REG: u64 = 1 << 8;
const MAKE_SOCK: u64 = 1 << 9;
const MAKE_FIFO: u64 = 1 << 10;
const MAKE_BLOCK: u64 = 1 << 11;
const MAKE_SYM: u64 = 1 << 12;
const REFER: u64 = 1 << 13;
const TRUNCATE: u64 = 1 << 14;
const IOCTL_DEV: u64 = 1 << 15;
const HANDLED_ACCESS_FS: u64 = WRITE_FILE
    | REMOVE_DIR
    | REMOVE_FILE
    | MAKE_CHAR
    | MAKE_DIR
    | MAKE_REG
    | MAKE_SOCK
    | MAKE_FIFO
    | MAKE_BLOCK
    | MAKE_SYM
    | REFER
    | TRUNCATE
    | IOCTL_DEV;
const ALLOWED_ACCESS_FS: u64 = HANDLED_ACCESS_FS & !(MAKE_CHAR | MAKE_BLOCK | IOCTL_DEV);
const SCOPED: u64 = (1 << 0) | (1 << 1);
#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
)))]
const UNSUPPORTED: &str =
    "confinement requires Linux x86_64 (64-bit) with Landlock ABI >= 6; no process launched";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Profile {
    pub schema_version: u32,
    pub id: String,
    pub minimum_abi: u32,
    pub handled_access_fs: u64,
    pub allowed_access_fs: u64,
    pub handled_access_net: u64,
    pub scoped: u64,
    pub write_leaves: [String; 6],
    pub parent_dumpable: i32,
    pub no_new_privs: bool,
    pub close_range_cloexec: bool,
    pub child_only: bool,
}

pub fn profile() -> Profile {
    Profile {
        schema_version: SCHEMA_VERSION,
        id: PROFILE_ID.into(),
        minimum_abi: MINIMUM_ABI,
        handled_access_fs: HANDLED_ACCESS_FS,
        allowed_access_fs: ALLOWED_ACCESS_FS,
        handled_access_net: 0,
        scoped: SCOPED,
        write_leaves: WRITE_LEAVES.map(String::from),
        parent_dumpable: 0,
        no_new_privs: true,
        close_range_cloexec: true,
        child_only: true,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Receipt {
    pub schema_version: u32,
    pub profile: Profile,
    pub landlock_abi: Option<u32>,
    pub parent_pid: u32,
    pub parent_dumpable: Option<i32>,
    pub hook_outcome: String,
}

impl Receipt {
    /// Validate the fixed production policy and the identity of its recorder.
    pub fn validate_enforced(&self, expected_parent_pid: u32) -> Result<(), String> {
        let p = &self.profile;
        // Compare against profile() without allocating another set of strings.
        if self.schema_version != SCHEMA_VERSION
            || p.schema_version != SCHEMA_VERSION
            || p.id != PROFILE_ID
            || p.minimum_abi != MINIMUM_ABI
            || p.handled_access_fs != HANDLED_ACCESS_FS
            || p.allowed_access_fs != ALLOWED_ACCESS_FS
            || p.handled_access_net != 0
            || p.scoped != SCOPED
            || !p.write_leaves.iter().zip(WRITE_LEAVES).all(|(a, b)| a == b)
            || p.parent_dumpable != 0
            || !p.no_new_privs
            || !p.close_range_cloexec
            || !p.child_only
        {
            return Err("confinement receipt schema or profile mismatch".into());
        }
        if !matches!(self.landlock_abi, Some(version) if version >= MINIMUM_ABI)
            || expected_parent_pid == 0
            || self.parent_pid != expected_parent_pid
            || self.parent_dumpable != Some(0)
            || self.hook_outcome != "ENFORCED_BEFORE_EXEC"
        {
            return Err("confinement receipt is not enforced for the expected parent".into());
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct SpawnFailure {
    pub receipt: Receipt,
    pub error: String,
}

/// Query the native kernel independently; this does not change process policy.
pub fn abi() -> Result<u32, String> {
    #[cfg(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))]
    {
        linux::abi()
    }
    #[cfg(not(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    )))]
    {
        Err(UNSUPPORTED.into())
    }
}

/// Protect the recorder before publishing a readiness gate. No child is started,
/// no Landlock policy is applied to the parent, and success is only NOT_RUN.
/// Parent dumpability is intentionally never restored.
pub fn prepare_parent() -> Result<Receipt, Box<SpawnFailure>> {
    let mut receipt = Receipt {
        schema_version: SCHEMA_VERSION,
        profile: profile(),
        landlock_abi: None,
        parent_pid: std::process::id(),
        parent_dumpable: None,
        hook_outcome: "NOT_RUN".into(),
    };
    #[cfg(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))]
    let result = linux::prepare_parent(&mut receipt);
    #[cfg(not(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    )))]
    let result: Result<(), String> = Err(UNSUPPORTED.into());
    if let Err(error) = result {
        receipt.hook_outcome = "REFUSED_NO_OFFICIAL_EXEC".into();
        return Err(Box::new(SpawnFailure { receipt, error }));
    }
    Ok(receipt)
}

/// The only production launcher. The caller supplies an existing private capture
/// directory with the six existing private leaves. Fresh pipe stdio is forced;
/// all nonstdio descriptors are close-on-exec, and only the child is landlocked.
pub fn spawn(command: &mut Command, capture: &Path) -> Result<(Child, Receipt), Box<SpawnFailure>> {
    let mut receipt = prepare_parent()?;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))]
    let result = linux::spawn(command, capture);
    #[cfg(not(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    )))]
    let result: Result<Child, String> = {
        let _ = (command, capture);
        Err(UNSUPPORTED.into())
    };
    match result {
        Ok(child) => {
            // Command::spawn's exec handshake succeeded, including every hook.
            receipt.hook_outcome = "ENFORCED_BEFORE_EXEC".into();
            Ok((child, receipt))
        }
        Err(error) => {
            receipt.hook_outcome = "REFUSED_NO_OFFICIAL_EXEC".into();
            Err(Box::new(SpawnFailure { receipt, error }))
        }
    }
}

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
mod linux {
    use super::*;
    use std::{
        ffi::{CStr, c_long},
        fs::{File, OpenOptions},
        io,
        mem::size_of,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::{
                fs::{MetadataExt, OpenOptionsExt},
                process::CommandExt,
            },
        },
    };

    // Native x86_64 syscall table and Linux UAPI; x32 is deliberately excluded.
    // https://raw.githubusercontent.com/torvalds/linux/v6.12/arch/x86/entry/syscalls/syscall_64.tbl
    const SYS_FCNTL: c_long = 72;
    const SYS_GETEUID: c_long = 107;
    const SYS_PRCTL: c_long = 157;
    const SYS_OPENAT: c_long = 257;
    const SYS_CLOSE_RANGE: c_long = 436;
    const SYS_LANDLOCK_CREATE_RULESET: c_long = 444;
    const SYS_LANDLOCK_ADD_RULE: c_long = 445;
    const SYS_LANDLOCK_RESTRICT_SELF: c_long = 446;
    const PR_GET_DUMPABLE: c_long = 3;
    const PR_SET_DUMPABLE: c_long = 4;
    const PR_SET_NO_NEW_PRIVS: c_long = 38;
    const F_DUPFD_CLOEXEC: c_long = 1030;
    const CLOSE_RANGE_CLOEXEC: c_long = 1 << 2;
    const O_DIRECTORY: i32 = 0o200000;
    const O_NOFOLLOW: i32 = 0o400000;
    const O_CLOEXEC: i32 = 0o2000000;
    const O_PATH: i32 = 0o10000000;
    const DIRECTORY_FLAGS: i32 = O_PATH | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC;
    const LANDLOCK_CREATE_RULESET_VERSION: c_long = 1;
    const LANDLOCK_RULE_PATH_BENEATH: c_long = 1;
    const LEAF_NAMES: [&CStr; 6] = [c"home", c"config", c"system", c"log", c"tmp", c"workspace"];

    #[repr(C)]
    struct RulesetAttr {
        handled_access_fs: u64,
        handled_access_net: u64,
        scoped: u64,
    }

    #[repr(C, packed)]
    struct PathBeneathAttr {
        allowed_access: u64,
        parent_fd: i32,
    }

    unsafe extern "C" {
        fn syscall(number: c_long, ...) -> c_long;
    }

    fn syscall_error(operation: &str) -> String {
        format!("confinement {operation}: {}", io::Error::last_os_error())
    }

    pub(super) fn abi() -> Result<u32, String> {
        // SAFETY: the ABI query requires a null pointer, zero size and flag 1.
        let result = unsafe {
            syscall(
                SYS_LANDLOCK_CREATE_RULESET,
                std::ptr::null::<RulesetAttr>(),
                0usize,
                LANDLOCK_CREATE_RULESET_VERSION,
            )
        };
        if result < 0 {
            return Err(syscall_error("Landlock ABI query"));
        }
        u32::try_from(result)
            .ok()
            .filter(|version| *version != 0)
            .ok_or_else(|| "confinement Landlock ABI query returned an invalid version".into())
    }

    pub(super) fn prepare_parent(receipt: &mut Receipt) -> Result<(), String> {
        let version = abi()?;
        receipt.landlock_abi = Some(version);
        if version < MINIMUM_ABI {
            return Err(format!(
                "confinement requires Landlock ABI >= {MINIMUM_ABI}; found {version}"
            ));
        }
        // SAFETY: PR_SET_DUMPABLE takes the scalar value 0; no pointer arguments.
        if unsafe {
            syscall(
                SYS_PRCTL,
                PR_SET_DUMPABLE,
                0 as c_long,
                0 as c_long,
                0 as c_long,
                0 as c_long,
            )
        } != 0
        {
            return Err(syscall_error("PR_SET_DUMPABLE"));
        }
        // SAFETY: PR_GET_DUMPABLE has no pointer arguments.
        let dumpable = unsafe {
            syscall(
                SYS_PRCTL,
                PR_GET_DUMPABLE,
                0 as c_long,
                0 as c_long,
                0 as c_long,
                0 as c_long,
            )
        };
        if dumpable < 0 {
            return Err(syscall_error("PR_GET_DUMPABLE"));
        }
        let dumpable = i32::try_from(dumpable)
            .map_err(|_| "confinement PR_GET_DUMPABLE returned an invalid value".to_string())?;
        receipt.parent_dumpable = Some(dumpable);
        if dumpable != 0 {
            return Err("confinement parent remained dumpable; no process launched".into());
        }
        Ok(())
    }

    fn owned_file(result: c_long, operation: &str) -> Result<File, String> {
        if result < 0 {
            return Err(syscall_error(operation));
        }
        // SAFETY: a successful descriptor-producing syscall returned a fresh fd.
        Ok(unsafe { File::from_raw_fd(result as i32) })
    }

    fn private_directory(file: &File, uid: u32) -> Result<(), String> {
        let metadata = file
            .metadata()
            .map_err(|e| format!("confinement directory metadata: {e}"))?;
        if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o7777 != 0o700 {
            return Err(
                "confinement requires owned 0700 nonsymlink capture and write-leaf directories"
                    .into(),
            );
        }
        Ok(())
    }

    struct Prepared {
        ruleset: File,
        _capture: File,
        _leaves: [Option<File>; 6],
    }

    impl Prepared {
        fn new(capture: &Path) -> Result<Self, String> {
            let root = OpenOptions::new()
                .read(true)
                .custom_flags(DIRECTORY_FLAGS)
                .open(capture)
                .map_err(|e| format!("confinement capture directory open: {e}"))?;
            // SAFETY: geteuid has no arguments and returns the effective UID.
            let uid = unsafe { syscall(SYS_GETEUID) };
            if uid < 0 {
                return Err(syscall_error("effective UID query"));
            }
            let uid =
                u32::try_from(uid).map_err(|_| "confinement invalid effective UID".to_string())?;
            private_directory(&root, uid)?;
            let attr = RulesetAttr {
                handled_access_fs: HANDLED_ACCESS_FS,
                handled_access_net: 0,
                scoped: SCOPED,
            };
            // SAFETY: attr is a live repr(C) UAPI buffer with its exact ABI size.
            // The kernel returns this ruleset descriptor with O_CLOEXEC.
            let ruleset = owned_file(
                unsafe {
                    syscall(
                        SYS_LANDLOCK_CREATE_RULESET,
                        &attr as *const RulesetAttr,
                        size_of::<RulesetAttr>(),
                        0 as c_long,
                    )
                },
                "ruleset creation",
            )?;
            // Command's stdio dup2 must never overwrite the ruleset descriptor
            // when this process happened to start with a closed standard fd.
            let ruleset = if ruleset.as_raw_fd() < 3 {
                // SAFETY: duplicate our live fd above stdio, atomically CLOEXEC.
                owned_file(
                    unsafe {
                        syscall(
                            SYS_FCNTL,
                            ruleset.as_raw_fd() as c_long,
                            F_DUPFD_CLOEXEC,
                            3 as c_long,
                        )
                    },
                    "ruleset descriptor duplication",
                )?
            } else {
                ruleset
            };
            let mut leaves = [const { None }; 6];
            for (slot, name) in leaves.iter_mut().zip(LEAF_NAMES) {
                // SAFETY: root is held, name is a single NUL-terminated literal,
                // and O_DIRECTORY|O_NOFOLLOW rejects a symlink instead of following it.
                let leaf = owned_file(
                    unsafe {
                        syscall(
                            SYS_OPENAT,
                            root.as_raw_fd() as c_long,
                            name.as_ptr(),
                            DIRECTORY_FLAGS as c_long,
                            0 as c_long,
                        )
                    },
                    "write-leaf open",
                )?;
                private_directory(&leaf, uid)?;
                let rule = PathBeneathAttr {
                    allowed_access: ALLOWED_ACCESS_FS,
                    parent_fd: leaf.as_raw_fd(),
                };
                // SAFETY: descriptors are live; the packed UAPI buffer is exactly
                // 12 bytes and is accessed by the kernel through a raw pointer.
                if unsafe {
                    syscall(
                        SYS_LANDLOCK_ADD_RULE,
                        ruleset.as_raw_fd() as c_long,
                        LANDLOCK_RULE_PATH_BENEATH,
                        &rule as *const PathBeneathAttr,
                        0 as c_long,
                    )
                } != 0
                {
                    return Err(syscall_error("write-leaf rule addition"));
                }
                *slot = Some(leaf);
            }
            Ok(Self {
                ruleset,
                _capture: root,
                _leaves: leaves,
            })
        }

        // Called only after fork: scalar syscalls and allocation-free errno
        // errors, with no formatting, locks, directory operations or destructors.
        fn enforce(&self) -> io::Result<()> {
            // SAFETY: documented scalar prctl arguments; no_new_privs cannot be undone.
            if unsafe {
                syscall(
                    SYS_PRCTL,
                    PR_SET_NO_NEW_PRIVS,
                    1 as c_long,
                    0 as c_long,
                    0 as c_long,
                    0 as c_long,
                )
            } != 0
            {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: the closure owns the live ruleset through the exec handshake.
            if unsafe {
                syscall(
                    SYS_LANDLOCK_RESTRICT_SELF,
                    self.ruleset.as_raw_fd() as c_long,
                    0 as c_long,
                )
            } != 0
            {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: CLOEXEC marks rather than closes all nonstdio fds, including
            // Rust's exec-error pipe; that pipe must still report any exec failure.
            if unsafe {
                syscall(
                    SYS_CLOSE_RANGE,
                    3 as c_long,
                    u32::MAX as c_long,
                    CLOSE_RANGE_CLOEXEC,
                )
            } != 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }
    }

    pub(super) fn spawn(command: &mut Command, capture: &Path) -> Result<Child, String> {
        let prepared = Prepared::new(capture)?;
        // SAFETY: enforce uses only async-signal-safe syscalls and raw errno;
        // owned capture/leaf/ruleset fds remain held by this closure in Command.
        unsafe { command.pre_exec(move || prepared.enforce()) };
        command
            .spawn()
            .map_err(|e| format!("confinement pre-exec or exec refused: {e}"))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::mem::{align_of, offset_of};

        #[test]
        fn linux_uapi_layout_is_exact() {
            assert_eq!(size_of::<RulesetAttr>(), 24);
            assert_eq!(align_of::<RulesetAttr>(), 8);
            assert_eq!(offset_of!(RulesetAttr, handled_access_fs), 0);
            assert_eq!(offset_of!(RulesetAttr, handled_access_net), 8);
            assert_eq!(offset_of!(RulesetAttr, scoped), 16);
            assert_eq!(size_of::<PathBeneathAttr>(), 12);
            assert_eq!(align_of::<PathBeneathAttr>(), 1);
            assert_eq!(offset_of!(PathBeneathAttr, allowed_access), 0);
            assert_eq!(offset_of!(PathBeneathAttr, parent_fd), 8);
            assert_eq!(LEAF_NAMES.map(|name| name.to_str().unwrap()), WRITE_LEAVES);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_is_exact_and_strict() {
        let value = serde_json::to_value(profile()).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "schemaVersion": 1,
                "id": "pr174-evidence-custody-landlock-v1",
                "minimumAbi": 6,
                "handledAccessFs": 0xfff2u64,
                "allowedAccessFs": 0x77b2u64,
                "handledAccessNet": 0,
                "scoped": 3,
                "writeLeaves": ["home", "config", "system", "log", "tmp", "workspace"],
                "parentDumpable": 0,
                "noNewPrivs": true,
                "closeRangeCloexec": true,
                "childOnly": true
            })
        );
        assert_eq!(
            serde_json::from_value::<Profile>(value.clone()).unwrap(),
            profile()
        );
        let mut extra = value;
        extra["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<Profile>(extra).is_err());
    }

    #[test]
    fn receipt_validation_requires_actual_enforcement() {
        let mut receipt = Receipt {
            schema_version: 1,
            profile: profile(),
            landlock_abi: Some(6),
            parent_pid: 7,
            parent_dumpable: Some(0),
            hook_outcome: "ENFORCED_BEFORE_EXEC".into(),
        };
        assert_eq!(receipt.validate_enforced(7), Ok(()));
        assert!(receipt.validate_enforced(0).is_err());
        assert!(receipt.validate_enforced(8).is_err());
        receipt.hook_outcome = "NOT_RUN".into();
        assert!(receipt.validate_enforced(7).is_err());
        receipt.hook_outcome = "ENFORCED_BEFORE_EXEC".into();
        receipt.landlock_abi = Some(5);
        assert!(receipt.validate_enforced(7).is_err());
        receipt.landlock_abi = Some(6);
        receipt.parent_dumpable = Some(1);
        assert!(receipt.validate_enforced(7).is_err());
        receipt.parent_dumpable = Some(0);
        receipt.profile.allowed_access_fs |= MAKE_CHAR;
        assert!(receipt.validate_enforced(7).is_err());
        let mut value = serde_json::to_value(receipt).unwrap();
        value["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<Receipt>(value).is_err());
    }

    #[cfg(not(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    )))]
    #[test]
    fn unsupported_platform_refuses_before_exec() {
        assert!(abi().is_err());
        let failure = prepare_parent().unwrap_err();
        assert_eq!(failure.receipt.hook_outcome, "REFUSED_NO_OFFICIAL_EXEC");
        assert_eq!(failure.receipt.landlock_abi, None);
        assert_eq!(failure.receipt.parent_dumpable, None);
        let mut command = Command::new("must-not-be-executed-confinement-test");
        let failure = spawn(&mut command, Path::new("/unused-capture")).unwrap_err();
        assert_eq!(failure.receipt.hook_outcome, "REFUSED_NO_OFFICIAL_EXEC");
        assert!(
            failure
                .receipt
                .validate_enforced(std::process::id())
                .is_err()
        );
    }
}

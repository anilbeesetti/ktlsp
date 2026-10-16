//! Bounded peer self-observations, not protected host or official oracle evidence.
//! The host must independently join original /proc identities and containment
//! resources before creating the fixed root-owned observer gate. Neither a peer
//! outcome nor a probe's accepted flag authenticates kill, cancellation, closure,
//! or absence of host mounts.
pub use crate::authority::CalibrationCase as Case;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub parent_pid: u32,
    pub process_group: u32,
    pub session: u32,
    pub start_time_ticks: u64,
    pub executable_device: u64,
    pub executable_inode: u64,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Probe {
    pub name: String,
    pub operation: String,
    pub return_value: i64,
    pub errno: Option<i32>,
    pub accepted: bool,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Exhaustion {
    pub bytes_written: u64,
    pub files_created: u32,
    pub terminal_errno: Option<i32>,
    pub reached_enospc: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    AwaitingObserver,
    Completed,
    AwaitingHostAction,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CustodyTarget {
    ClientIntended,
    Stdout,
    Stderr,
    Manifest,
    ReadyReport,
    Home,
    Config,
    System,
    Log,
    Tmp,
    Workspace,
}
impl CustodyTarget {
    pub const ALL: [Self; 5] = [
        Self::ClientIntended,
        Self::Stdout,
        Self::Stderr,
        Self::Manifest,
        Self::ReadyReport,
    ];
    pub const WRITE_LEAVES: [Self; 6] = [
        Self::Home,
        Self::Config,
        Self::System,
        Self::Log,
        Self::Tmp,
        Self::Workspace,
    ];
    pub fn filename(self) -> &'static str {
        match self {
            Self::ClientIntended => "client-intended.raw",
            Self::Stdout => "stdout.raw",
            Self::Stderr => "stderr.raw",
            Self::Manifest => "manifest.json",
            Self::ReadyReport => "calibration-ready.json",
            Self::Home => "home",
            Self::Config => "config",
            Self::System => "system",
            Self::Log => "log",
            Self::Tmp => "tmp",
            Self::Workspace => "workspace",
        }
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CustodyAction {
    Write,
    Truncate,
    Rename,
    Unlink,
    Hardlink,
    InheritedFd,
    ProcFd,
    PidfdOpen,
    PidfdGetfd,
    PidfdSignal,
    Ptrace,
    ProcessVmWrite,
    Signal,
    AbstractUnix,
    PrivateWrite,
}
impl CustodyAction {
    pub const FILE_ACTIONS: [Self; 5] = [
        Self::Write,
        Self::Truncate,
        Self::Rename,
        Self::Unlink,
        Self::Hardlink,
    ];
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CustodyProbe {
    pub action: CustodyAction,
    pub target: Option<CustodyTarget>,
    pub return_value: i64,
    pub errno: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readback: Option<u8>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CustodyFile {
    pub target: CustodyTarget,
    pub device: u64,
    pub inode: u64,
    pub bytes: u64,
    pub unchanged: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CustodyRole {
    Direct,
    InheritedSetsid,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CustodyChild {
    pub role: CustodyRole,
    pub original_process: ProcessIdentity,
    pub recorder_pid: u32,
    pub recorder_start_time_ticks: u64,
    pub probes: Vec<CustodyProbe>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CustodyReceipt {
    pub confinement: crate::recorder::confinement::Receipt,
    pub files: Vec<CustodyFile>,
    pub memory_unchanged: bool,
    pub children: Vec<CustodyChild>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Report {
    pub schema_version: u8,
    pub scope: String,
    pub case: Case,
    pub outcome: Outcome,
    pub original_process: ProcessIdentity,
    pub children: Vec<ProcessIdentity>,
    pub observer_gate_observed: bool,
    pub probes: Vec<Probe>,
    pub exhaustion: Option<Exhaustion>,
    pub custody: Option<CustodyReceipt>,
    pub first_failure: Option<String>,
    pub elapsed_ms: u64,
}

pub fn permission_denied(errno: Option<i32>) -> bool {
    matches!(errno, Some(1 | 13 | 30)) // EPERM, EACCES, EROFS; not missing input.
}
pub fn namespace_denied(errno: Option<i32>) -> bool {
    matches!(errno, Some(1 | 13))
}
pub fn absent_or_denied(errno: Option<i32>) -> bool {
    matches!(errno, Some(1 | 2 | 13))
}
pub fn egress_denied(errno: Option<i32>) -> bool {
    matches!(errno, Some(1 | 13 | 101 | 113))
}
pub fn exhaustion_reached(errno: Option<i32>) -> bool {
    errno == Some(28) // ENOSPC only, never EDQUOT.
}
pub fn mountpoint_denied(errno: Option<i32>) -> bool {
    permission_denied(errno) || errno == Some(16) // EBUSY.
}
impl ProcessIdentity {
    fn validate(&self) -> Result<(), String> {
        if self.pid < 2
            || self.parent_pid == 0
            || self.pid == self.parent_pid
            || self.process_group == 0
            || self.session == 0
            || self.start_time_ticks == 0
            || self.executable_device == 0
            || self.executable_inode == 0
        {
            return Err("missing original peer process identity".into());
        }
        Ok(())
    }
}
impl Report {
    /// Validates only finite final peer self-observations. The caller still
    /// needs independently authenticated original-host identity and teardown.
    pub fn validate_case(&self, expected: Case) -> Result<(), String> {
        if self.schema_version != 1
            || self.scope != "PR174_RUST_CALIBRATION_PEER"
            || self.case != expected
            || self.first_failure.is_some()
            || !self.observer_gate_observed
            || self.elapsed_ms >= 300_000
        {
            return Err("invalid final peer scope, gate, error, case, or lifetime".into());
        }
        let outcome = if expected.forced() {
            Outcome::AwaitingHostAction
        } else {
            Outcome::Completed
        };
        if self.outcome != outcome {
            return Err("peer final outcome does not match the finite case".into());
        }
        self.original_process.validate()?;
        let parent = &self.original_process;
        if matches!(expected, Case::SameGroup | Case::Setsid | Case::Setpgid) {
            if self.children.len() != 1 {
                return Err("descendant case requires exactly one original Rust child".into());
            }
            let child = &self.children[0];
            child.validate()?;
            if child.pid == parent.pid
                || child.parent_pid != parent.pid
                || child.start_time_ticks < parent.start_time_ticks
                || child.executable_device != parent.executable_device
                || child.executable_inode != parent.executable_inode
            {
                return Err(
                    "descendant original executable, parent, or start identity mismatch".into(),
                );
            }
            let valid_group = match expected {
                Case::SameGroup => {
                    child.process_group == parent.process_group && child.session == parent.session
                }
                Case::Setsid => {
                    child.process_group == child.pid
                        && child.session == child.pid
                        && child.session != parent.session
                }
                Case::Setpgid => {
                    child.process_group == child.pid
                        && child.process_group != parent.process_group
                        && child.session == parent.session
                }
                _ => false,
            };
            if !valid_group {
                return Err("descendant case did not establish its actual group/session".into());
            }
        } else if !self.children.is_empty() {
            return Err("unexpected descendants in this finite peer case".into());
        }
        match expected {
            Case::StorageExhaustion | Case::InodeExhaustion => {
                let exhaustion = self
                    .exhaustion
                    .as_ref()
                    .ok_or("missing real exhaustion observation")?;
                if !exhaustion.reached_enospc || !exhaustion_reached(exhaustion.terminal_errno) {
                    return Err("exhaustion did not terminate with exact ENOSPC".into());
                }
                let valid = if expected == Case::StorageExhaustion {
                    exhaustion.bytes_written > 0
                        && exhaustion.bytes_written < 536_870_912
                        && exhaustion.files_created == 1
                } else {
                    exhaustion.bytes_written == 0
                        && exhaustion.files_created > 0
                        && exhaustion.files_created < 16_384
                };
                if !valid {
                    return Err("exhaustion counters do not fit the exact bounded resource".into());
                }
            }
            _ if self.exhaustion.is_some() => {
                return Err("unexpected exhaustion observation".into());
            }
            _ => {}
        }
        if expected == Case::BoundaryDenials {
            self.validate_denials()?;
            self.custody
                .as_ref()
                .ok_or("missing real parent-custody child cohort")?
                .validate(&self.original_process)?;
        } else if !self.probes.is_empty() || self.custody.is_some() {
            return Err(
                "unexpected boundary probes or custody cohort outside boundary-denials".into(),
            );
        }
        Ok(())
    }
    fn probe(&self, name: &str, operation: &str) -> Result<&Probe, String> {
        // Fixed ceiling of 34 entries; scanning avoids allocating a second
        // attacker-controlled name map for this deliberately closed schema.
        let mut matching = self.probes.iter().filter(|probe| probe.name == name);
        let probe = matching
            .next()
            .ok_or_else(|| format!("missing boundary probe {name}"))?;
        if matching.next().is_some() || probe.operation != operation || !probe.accepted {
            return Err(format!(
                "duplicate, rejected, or wrong-operation boundary probe {name}"
            ));
        }
        Ok(probe)
    }
    fn denied_probe(
        &self,
        name: &str,
        operation: &str,
        classifier: fn(Option<i32>) -> bool,
    ) -> Result<(), String> {
        let probe = self.probe(name, operation)?;
        if probe.return_value != -1 || !classifier(probe.errno) {
            return Err(format!(
                "boundary probe {name} has no actual expected denial"
            ));
        }
        Ok(())
    }
    fn socket_created(&self, name: &str) -> Result<bool, String> {
        let probe = self.probe(name, "socket")?;
        if (0..=i64::from(i32::MAX)).contains(&probe.return_value) && probe.errno.is_none() {
            return Ok(true);
        }
        if probe.return_value == -1 && namespace_denied(probe.errno) {
            return Ok(false);
        }
        Err(format!("socket probe {name} has invalid syscall facts"))
    }
    fn validate_denials(&self) -> Result<(), String> {
        if self.probes.len() > 34 {
            return Err("boundary probe count exceeds fixed ceiling".into());
        }
        self.denied_probe(
            "host-proc-unavailable",
            "open /host/proc/1/stat",
            absent_or_denied,
        )?;
        self.denied_probe(
            "ancestor-pid-namespace",
            "ioctl NS_GET_PARENT",
            namespace_denied,
        )?;
        let mut count = 3; // two denials above plus the mandatory egress socket.
        if self.socket_created("egress-socket")? {
            self.denied_probe("network-egress", "connect 203.0.113.1:443", egress_denied)?;
            count += 1;
        }
        for path in ["/run/docker.sock", "/var/run/docker.sock"] {
            count += 1;
            if self.socket_created(&format!("daemon-socket-{path}"))? {
                self.denied_probe(
                    &format!("daemon-connect-{path}"),
                    "connect fixed daemon socket",
                    absent_or_denied,
                )?;
                count += 1;
            }
        }
        for input in [
            "fixture/declarations.kt",
            "fixture/error-node.kt",
            "preflight/identity.json",
        ] {
            self.denied_probe(input, "open existing input O_WRONLY", permission_denied)?;
            count += 1;
        }
        for root in ["fixture", "distribution", "tool", "preflight"] {
            self.denied_probe(
                &format!("rename-{root}"),
                "rename mountpoint",
                mountpoint_denied,
            )?;
            self.denied_probe(
                &format!("remove-{root}"),
                "rmdir mountpoint",
                mountpoint_denied,
            )?;
            self.denied_probe(
                &format!("unmount-{root}"),
                "umount2 mountpoint",
                namespace_denied,
            )?;
            self.denied_probe(
                &format!("bind-over-{root}"),
                "mount MS_BIND over input",
                namespace_denied,
            )?;
            count += 4;
        }
        for namespace in ["user", "mount", "pid", "network", "ipc", "uts", "cgroup"] {
            self.denied_probe(
                &format!("unshare-{namespace}"),
                "unshare namespace",
                namespace_denied,
            )?;
            count += 1;
        }
        if count != self.probes.len() {
            return Err("unknown or inapplicable conditional boundary probes".into());
        }
        Ok(())
    }
}

impl CustodyChild {
    fn probe(
        &self,
        action: CustodyAction,
        target: Option<CustodyTarget>,
    ) -> Result<&CustodyProbe, String> {
        let mut matching = self
            .probes
            .iter()
            .filter(|probe| probe.action == action && probe.target == target);
        let probe = matching.next().ok_or("missing custody syscall probe")?;
        if matching.next().is_some() {
            return Err("duplicate custody syscall probe".into());
        }
        let valid = match action {
            CustodyAction::InheritedFd => probe.return_value == -1 && probe.errno == Some(9),
            CustodyAction::PidfdOpen => {
                (3..=i64::from(i32::MAX)).contains(&probe.return_value) && probe.errno.is_none()
            }
            CustodyAction::PrivateWrite => {
                probe.return_value == 1 && probe.errno.is_none() && probe.readback == Some(0x5a)
            }
            CustodyAction::Rename | CustodyAction::Hardlink => {
                probe.return_value == -1
                    && (namespace_denied(probe.errno) || probe.errno == Some(18))
            }
            _ => probe.return_value == -1 && namespace_denied(probe.errno),
        };
        if !valid || (action != CustodyAction::PrivateWrite && probe.readback.is_some()) {
            return Err(format!(
                "custody syscall {action:?}/{target:?} did not establish the required result"
            ));
        }
        Ok(probe)
    }
    fn validate(&self, recorder: &ProcessIdentity) -> Result<(), String> {
        self.original_process.validate()?;
        if self.recorder_pid != recorder.pid
            || self.recorder_start_time_ticks != recorder.start_time_ticks
            || self.original_process.pid == recorder.pid
            || self.original_process.start_time_ticks < recorder.start_time_ticks
            || self.original_process.executable_device != recorder.executable_device
            || self.original_process.executable_inode != recorder.executable_inode
            || self.probes.len() != 44
        {
            return Err(
                "custody child original identity, target, or finite probe count mismatch".into(),
            );
        }
        for target in CustodyTarget::ALL {
            for action in CustodyAction::FILE_ACTIONS {
                self.probe(action, Some(target))?;
            }
            self.probe(CustodyAction::InheritedFd, Some(target))?;
        }
        for action in [CustodyAction::ProcFd, CustodyAction::PidfdGetfd] {
            self.probe(action, Some(CustodyTarget::ClientIntended))?;
        }
        for action in [
            CustodyAction::PidfdOpen,
            CustodyAction::PidfdSignal,
            CustodyAction::Ptrace,
            CustodyAction::ProcessVmWrite,
            CustodyAction::Signal,
            CustodyAction::AbstractUnix,
        ] {
            self.probe(action, None)?;
        }
        for leaf in CustodyTarget::WRITE_LEAVES {
            self.probe(CustodyAction::PrivateWrite, Some(leaf))?;
        }
        Ok(())
    }
}
impl CustodyReceipt {
    fn validate(&self, recorder: &ProcessIdentity) -> Result<(), String> {
        self.confinement.validate_enforced(recorder.pid)?;
        if !self.memory_unchanged || self.files.len() != 5 || self.children.len() != 2 {
            return Err("custody parent bytes changed or finite cohort incomplete".into());
        }
        for target in CustodyTarget::ALL {
            let mut matching = self.files.iter().filter(|file| file.target == target);
            let file = matching.next().ok_or("missing original custody file")?;
            if matching.next().is_some()
                || !file.unchanged
                || file.device == 0
                || file.inode == 0
                || file.bytes == 0
                || file.bytes > 16_384
                || self.files.iter().any(|other| {
                    other.target != target
                        && other.device == file.device
                        && other.inode == file.inode
                })
            {
                return Err("custody file original bytes or inode identity mismatch".into());
            }
        }
        let direct = &self.children[0];
        let inherited = &self.children[1];
        direct.validate(recorder)?;
        inherited.validate(recorder)?;
        if direct.role != CustodyRole::Direct
            || inherited.role != CustodyRole::InheritedSetsid
            || direct.original_process.parent_pid != recorder.pid
            || direct.original_process.process_group != recorder.process_group
            || direct.original_process.session != recorder.session
            || inherited.original_process.parent_pid != direct.original_process.pid
            || inherited.original_process.pid == direct.original_process.pid
            || inherited.original_process.start_time_ticks
                < direct.original_process.start_time_ticks
            || inherited.original_process.process_group != inherited.original_process.pid
            || inherited.original_process.session != inherited.original_process.pid
        {
            return Err(
                "custody direct/fork/setsid relationship did not inherit the original domain"
                    .into(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn synthetic_custody(parent: &ProcessIdentity) -> CustodyReceipt {
        let mut children = Vec::new();
        for (role, pid, parent_pid) in [
            (CustodyRole::Direct, 3, parent.pid),
            (CustodyRole::InheritedSetsid, 4, 3),
        ] {
            let inherited = role == CustodyRole::InheritedSetsid;
            let mut child = CustodyChild {
                role,
                recorder_pid: parent.pid,
                recorder_start_time_ticks: parent.start_time_ticks,
                original_process: ProcessIdentity {
                    pid,
                    parent_pid,
                    process_group: if inherited { pid } else { parent.process_group },
                    session: if inherited { pid } else { parent.session },
                    start_time_ticks: parent.start_time_ticks + u64::from(pid),
                    executable_device: parent.executable_device,
                    executable_inode: parent.executable_inode,
                },
                probes: Vec::new(),
            };
            for target in CustodyTarget::ALL {
                for action in CustodyAction::FILE_ACTIONS {
                    child.probes.push(CustodyProbe {
                        action,
                        target: Some(target),
                        return_value: -1,
                        errno: Some(13),
                        readback: None,
                    });
                }
                child.probes.push(CustodyProbe {
                    action: CustodyAction::InheritedFd,
                    target: Some(target),
                    return_value: -1,
                    errno: Some(9),
                    readback: None,
                });
            }
            for action in [CustodyAction::ProcFd, CustodyAction::PidfdGetfd] {
                child.probes.push(CustodyProbe {
                    action,
                    target: Some(CustodyTarget::ClientIntended),
                    return_value: -1,
                    errno: Some(1),
                    readback: None,
                });
            }
            for action in [
                CustodyAction::PidfdOpen,
                CustodyAction::PidfdSignal,
                CustodyAction::Ptrace,
                CustodyAction::ProcessVmWrite,
                CustodyAction::Signal,
                CustodyAction::AbstractUnix,
            ] {
                let (return_value, errno) = match action {
                    CustodyAction::PidfdOpen => (5, None),
                    _ => (-1, Some(1)),
                };
                child.probes.push(CustodyProbe {
                    action,
                    target: None,
                    return_value,
                    errno,
                    readback: None,
                });
            }
            for leaf in CustodyTarget::WRITE_LEAVES {
                child.probes.push(CustodyProbe {
                    action: CustodyAction::PrivateWrite,
                    target: Some(leaf),
                    return_value: 1,
                    errno: None,
                    readback: Some(0x5a),
                });
            }
            children.push(child);
        }
        CustodyReceipt {
            // Explicit synthetic fixture: never a production or host receipt.
            confinement: crate::recorder::confinement::Receipt {
                schema_version: 1,
                profile: crate::recorder::confinement::profile(),
                landlock_abi: Some(6),
                parent_pid: parent.pid,
                parent_dumpable: Some(0),
                hook_outcome: "ENFORCED_BEFORE_EXEC".into(),
            },
            files: CustodyTarget::ALL
                .into_iter()
                .enumerate()
                .map(|(index, target)| CustodyFile {
                    target,
                    device: 1,
                    inode: 100 + index as u64,
                    bytes: 64,
                    unchanged: true,
                })
                .collect(),
            memory_unchanged: true,
            children,
        }
    }

    fn synthetic(case: Case) -> Report {
        let parent = ProcessIdentity {
            pid: 2,
            parent_pid: 1,
            process_group: 1,
            session: 1,
            start_time_ticks: 10,
            executable_device: 1,
            executable_inode: 20,
        };
        let mut report = Report {
            schema_version: 1,
            scope: "PR174_RUST_CALIBRATION_PEER".into(),
            case,
            outcome: if case.forced() {
                Outcome::AwaitingHostAction
            } else {
                Outcome::Completed
            },
            original_process: parent.clone(),
            children: Vec::new(),
            observer_gate_observed: true,
            probes: Vec::new(),
            exhaustion: None,
            custody: None,
            first_failure: None,
            elapsed_ms: 1,
        };
        if matches!(case, Case::SameGroup | Case::Setsid | Case::Setpgid) {
            let mut child = ProcessIdentity {
                pid: 3,
                parent_pid: 2,
                start_time_ticks: 11,
                ..parent
            };
            if case != Case::SameGroup {
                child.process_group = 3;
            }
            if case == Case::Setsid {
                child.session = 3;
            }
            report.children.push(child);
        }
        match case {
            Case::StorageExhaustion => {
                report.exhaustion = Some(Exhaustion {
                    bytes_written: 4096,
                    files_created: 1,
                    terminal_errno: Some(28),
                    reached_enospc: true,
                })
            }
            Case::InodeExhaustion => {
                report.exhaustion = Some(Exhaustion {
                    bytes_written: 0,
                    files_created: 10,
                    terminal_errno: Some(28),
                    reached_enospc: true,
                })
            }
            Case::BoundaryDenials => {
                let mut deny = |name: String, operation: &str| {
                    report.probes.push(Probe {
                        name,
                        operation: operation.into(),
                        return_value: -1,
                        errno: Some(1),
                        accepted: true,
                    })
                };
                deny("host-proc-unavailable".into(), "open /host/proc/1/stat");
                deny("ancestor-pid-namespace".into(), "ioctl NS_GET_PARENT");
                deny("egress-socket".into(), "socket");
                for path in ["/run/docker.sock", "/var/run/docker.sock"] {
                    deny(format!("daemon-socket-{path}"), "socket");
                }
                for input in [
                    "fixture/declarations.kt",
                    "fixture/error-node.kt",
                    "preflight/identity.json",
                ] {
                    deny(input.into(), "open existing input O_WRONLY");
                }
                for root in ["fixture", "distribution", "tool", "preflight"] {
                    deny(format!("rename-{root}"), "rename mountpoint");
                    deny(format!("remove-{root}"), "rmdir mountpoint");
                    deny(format!("unmount-{root}"), "umount2 mountpoint");
                    deny(format!("bind-over-{root}"), "mount MS_BIND over input");
                }
                for namespace in ["user", "mount", "pid", "network", "ipc", "uts", "cgroup"] {
                    deny(format!("unshare-{namespace}"), "unshare namespace");
                }
                report.custody = Some(synthetic_custody(&report.original_process));
            }
            _ => {}
        }
        report
    }
    #[test]
    fn synthetic_final_cases_are_distinct_from_ready_failure_and_host_evidence() {
        for case in Case::ALL {
            let report = synthetic(case);
            assert!(report.validate_case(case).is_ok());
            let mut invalid = report.clone();
            invalid.case = if case == Case::Normal {
                Case::Kill
            } else {
                Case::Normal
            };
            assert!(invalid.validate_case(case).is_err());
            for outcome in [Outcome::AwaitingObserver, Outcome::Failed] {
                let mut invalid = report.clone();
                invalid.outcome = outcome;
                assert!(invalid.validate_case(case).is_err());
            }
            let mut invalid = report.clone();
            invalid.observer_gate_observed = false;
            assert!(invalid.validate_case(case).is_err());
            let mut invalid = report.clone();
            invalid.first_failure = Some("original error".into());
            assert!(invalid.validate_case(case).is_err());
            let mut invalid = report.clone();
            invalid.elapsed_ms = 300_000;
            assert!(invalid.validate_case(case).is_err());
            let mut invalid = report.clone();
            invalid.original_process.executable_inode = 0;
            assert!(invalid.validate_case(case).is_err());
            let mut invalid = report;
            invalid.outcome = if case.forced() {
                Outcome::Completed
            } else {
                Outcome::AwaitingHostAction
            };
            assert!(invalid.validate_case(case).is_err());
        }
    }
    #[test]
    fn synthetic_descendants_require_original_executable_and_actual_group_relationships() {
        for case in [Case::SameGroup, Case::Setsid, Case::Setpgid] {
            let report = synthetic(case);
            let mut invalid = report.clone();
            invalid.children[0].executable_inode += 1;
            assert!(invalid.validate_case(case).is_err());
            let mut invalid = report.clone();
            invalid.children[0].parent_pid = 1;
            assert!(invalid.validate_case(case).is_err());
            let mut invalid = report.clone();
            invalid.children[0].start_time_ticks = 1;
            assert!(invalid.validate_case(case).is_err());
            let mut invalid = report;
            invalid.children[0].process_group = 99;
            assert!(invalid.validate_case(case).is_err());
        }
        let mut ordinary = synthetic(Case::Normal);
        ordinary.children.push(ordinary.original_process.clone());
        assert!(ordinary.validate_case(Case::Normal).is_err());
    }
    #[test]
    fn synthetic_denials_require_exact_sets_and_real_syscall_errors() {
        let report = synthetic(Case::BoundaryDenials);
        let mut invalid = report.clone();
        invalid.probes.pop();
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        let mut invalid = report.clone();
        invalid.probes.push(invalid.probes[0].clone());
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        for (return_value, errno, accepted) in
            [(0, None, true), (-1, Some(22), true), (-1, Some(1), false)]
        {
            let mut invalid = report.clone();
            let probe = &mut invalid.probes[1];
            probe.return_value = return_value;
            probe.errno = errno;
            probe.accepted = accepted;
            assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        }
        let mut invalid = report;
        invalid.probes[0].operation = "fabricated observation".into();
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
    }
    #[test]
    fn synthetic_socket_success_requires_the_exact_conditional_connect_denial() {
        let mut report = synthetic(Case::BoundaryDenials);
        for (socket_name, connect_name, operation, errno) in [
            (
                "egress-socket",
                "network-egress",
                "connect 203.0.113.1:443",
                101,
            ),
            (
                "daemon-socket-/run/docker.sock",
                "daemon-connect-/run/docker.sock",
                "connect fixed daemon socket",
                2,
            ),
            (
                "daemon-socket-/var/run/docker.sock",
                "daemon-connect-/var/run/docker.sock",
                "connect fixed daemon socket",
                2,
            ),
        ] {
            let socket = report
                .probes
                .iter_mut()
                .find(|p| p.name == socket_name)
                .unwrap();
            socket.return_value = 3;
            socket.errno = None;
            assert!(report.validate_case(Case::BoundaryDenials).is_err());
            report.probes.push(Probe {
                name: connect_name.into(),
                operation: operation.into(),
                return_value: -1,
                errno: Some(errno),
                accepted: true,
            });
            assert!(report.validate_case(Case::BoundaryDenials).is_ok());
        }
        assert_eq!(report.probes.len(), 34);
        let mut invalid = report.clone();
        invalid
            .probes
            .iter_mut()
            .find(|p| p.name == "network-egress")
            .unwrap()
            .errno = Some(115);
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        let mut invalid = report;
        let socket = invalid
            .probes
            .iter_mut()
            .find(|p| p.name == "egress-socket")
            .unwrap();
        socket.return_value = -1;
        socket.errno = Some(1);
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
    }
    #[test]
    fn synthetic_exhaustion_requires_nonzero_bounded_counters_and_exact_enospc() {
        for case in [Case::StorageExhaustion, Case::InodeExhaustion] {
            let report = synthetic(case);
            let mut invalid = report.clone();
            invalid.exhaustion.as_mut().unwrap().terminal_errno = Some(122);
            assert!(invalid.validate_case(case).is_err());
            let mut invalid = report.clone();
            invalid.exhaustion.as_mut().unwrap().reached_enospc = false;
            assert!(invalid.validate_case(case).is_err());
            let mut invalid = report.clone();
            if case == Case::StorageExhaustion {
                invalid.exhaustion.as_mut().unwrap().bytes_written = 536_870_912;
            } else {
                invalid.exhaustion.as_mut().unwrap().files_created = 16_384;
            }
            assert!(invalid.validate_case(case).is_err());
            let mut invalid = report;
            let exhaustion = invalid.exhaustion.as_mut().unwrap();
            exhaustion.bytes_written = 0;
            exhaustion.files_created = 0;
            assert!(invalid.validate_case(case).is_err());
        }
    }
    #[test]
    fn synthetic_custody_rejects_missing_unapplied_or_wrong_parent_policy() {
        let report = synthetic(Case::BoundaryDenials);
        let mut invalid = report.clone();
        invalid.custody = None;
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        for outcome in ["NOT_RUN", "REFUSED_NO_OFFICIAL_EXEC"] {
            let mut invalid = report.clone();
            invalid.custody.as_mut().unwrap().confinement.hook_outcome = outcome.into();
            assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        }
        let mut invalid = report.clone();
        invalid.custody.as_mut().unwrap().confinement.parent_pid += 1;
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        let mut invalid = report.clone();
        invalid.custody.as_mut().unwrap().confinement.landlock_abi = Some(5);
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        let mut invalid = report.clone();
        invalid
            .custody
            .as_mut()
            .unwrap()
            .confinement
            .parent_dumpable = Some(1);
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        let mut invalid = report.clone();
        invalid.custody.as_mut().unwrap().confinement.profile.scoped = 0;
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        let mut invalid = report.clone();
        let custody = invalid.custody.as_mut().unwrap();
        custody.files[0] = custody.files[1].clone();
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        let mut invalid = report;
        invalid.case = Case::Normal;
        invalid.probes.clear();
        assert!(invalid.validate_case(Case::Normal).is_err());
    }
    #[test]
    fn synthetic_custody_requires_original_bytes_and_genuine_fork_setsid_lineage() {
        let report = synthetic(Case::BoundaryDenials);
        let mut invalid = report.clone();
        invalid.custody.as_mut().unwrap().memory_unchanged = false;
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        for index in 0..5 {
            let mut invalid = report.clone();
            invalid.custody.as_mut().unwrap().files[index].unchanged = false;
            assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        }
        let mut invalid = report.clone();
        invalid.custody.as_mut().unwrap().children[1]
            .original_process
            .parent_pid = report.original_process.pid;
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        let mut invalid = report.clone();
        invalid.custody.as_mut().unwrap().children[1].recorder_pid = 3; // Same-domain direct parent is not the protected recorder.
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        let mut invalid = report.clone();
        invalid.custody.as_mut().unwrap().children[1]
            .original_process
            .session = 1;
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        let mut invalid = report;
        invalid.custody.as_mut().unwrap().children[1].recorder_start_time_ticks += 1;
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
    }
    #[test]
    fn synthetic_custody_never_accepts_syscall_success_as_denial_or_failed_allowed_write() {
        let report = synthetic(Case::BoundaryDenials);
        for child_index in 0..2 {
            let child = &report.custody.as_ref().unwrap().children[child_index];
            assert_eq!(child.probes.len(), 44);
            for probe_index in 0..child.probes.len() {
                let mut invalid = report.clone();
                let probe = &mut invalid.custody.as_mut().unwrap().children[child_index].probes
                    [probe_index];
                if matches!(
                    probe.action,
                    CustodyAction::PidfdOpen | CustodyAction::PrivateWrite
                ) {
                    probe.return_value = -1;
                    probe.errno = Some(1);
                } else {
                    probe.return_value = 0;
                    probe.errno = None;
                }
                assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
            }
        }
        let mut invalid = report.clone();
        let child = &mut invalid.custody.as_mut().unwrap().children[0];
        child.probes[0] = child.probes[1].clone();
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        assert!(serde_json::to_vec(&report).unwrap().len() <= 16_384);
    }
    #[test]
    fn synthetic_each_production_leaf_requires_exact_success_and_actual_readback() {
        let profile = crate::recorder::confinement::profile();
        assert_eq!(
            CustodyTarget::WRITE_LEAVES.map(|leaf| leaf.filename()),
            profile.write_leaves.each_ref().map(String::as_str)
        );
        let report = synthetic(Case::BoundaryDenials);
        for child_index in 0..2 {
            for leaf in CustodyTarget::WRITE_LEAVES {
                for readback in [None, Some(0)] {
                    let mut invalid = report.clone();
                    let probe = invalid.custody.as_mut().unwrap().children[child_index]
                        .probes
                        .iter_mut()
                        .find(|probe| {
                            probe.action == CustodyAction::PrivateWrite
                                && probe.target == Some(leaf)
                        })
                        .unwrap();
                    probe.readback = readback;
                    assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
                }
                let mut invalid = report.clone();
                invalid.custody.as_mut().unwrap().children[child_index]
                    .probes
                    .retain(|probe| {
                        probe.action != CustodyAction::PrivateWrite || probe.target != Some(leaf)
                    });
                assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
            }
        }
        let mut invalid = report;
        invalid.custody.as_mut().unwrap().children[0].probes[0].readback = Some(0x5a);
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
    }
    #[test]
    fn synthetic_maximum_final_schema_stays_within_the_reserved_16384_bytes() {
        let mut report = synthetic(Case::BoundaryDenials);
        for (socket_name, connect_name, operation, errno) in [
            (
                "egress-socket",
                "network-egress",
                "connect 203.0.113.1:443",
                113,
            ),
            (
                "daemon-socket-/run/docker.sock",
                "daemon-connect-/run/docker.sock",
                "connect fixed daemon socket",
                13,
            ),
            (
                "daemon-socket-/var/run/docker.sock",
                "daemon-connect-/var/run/docker.sock",
                "connect fixed daemon socket",
                13,
            ),
        ] {
            let socket = report
                .probes
                .iter_mut()
                .find(|probe| probe.name == socket_name)
                .unwrap();
            socket.return_value = i64::from(i32::MAX);
            socket.errno = None;
            report.probes.push(Probe {
                name: connect_name.into(),
                operation: operation.into(),
                return_value: -1,
                errno: Some(errno),
                accepted: true,
            });
        }
        // Conservative maximum decimal widths, not real kernel observations.
        report.original_process = ProcessIdentity {
            pid: u32::MAX - 2,
            parent_pid: u32::MAX - 5,
            process_group: u32::MAX - 3,
            session: u32::MAX - 4,
            start_time_ticks: u64::MAX - 2,
            executable_device: u64::MAX,
            executable_inode: u64::MAX,
        };
        report.elapsed_ms = 299_999;
        let custody = report.custody.as_mut().unwrap();
        custody.confinement.parent_pid = report.original_process.pid;
        custody.confinement.landlock_abi = Some(u32::MAX);
        for (index, file) in custody.files.iter_mut().enumerate() {
            file.device = u64::MAX;
            file.inode = u64::MAX - index as u64;
            file.bytes = 16_384;
        }
        for (index, child) in custody.children.iter_mut().enumerate() {
            let inherited = index == 1;
            let pid = u32::MAX - 1 + index as u32;
            child.original_process = ProcessIdentity {
                pid,
                parent_pid: if inherited {
                    u32::MAX - 1
                } else {
                    report.original_process.pid
                },
                process_group: if inherited {
                    pid
                } else {
                    report.original_process.process_group
                },
                session: if inherited {
                    pid
                } else {
                    report.original_process.session
                },
                start_time_ticks: u64::MAX - 1 + index as u64,
                executable_device: u64::MAX,
                executable_inode: u64::MAX,
            };
            child.recorder_pid = report.original_process.pid;
            child.recorder_start_time_ticks = report.original_process.start_time_ticks;
            child
                .probes
                .iter_mut()
                .find(|probe| probe.action == CustodyAction::PidfdOpen)
                .unwrap()
                .return_value = i64::from(i32::MAX);
        }
        assert_eq!(report.probes.len(), 34);
        assert!(report.validate_case(Case::BoundaryDenials).is_ok());
        assert!(serde_json::to_vec(&report).unwrap().len() <= 16_384);
    }
}

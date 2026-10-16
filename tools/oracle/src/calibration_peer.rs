//! Bounded peer self-observations, not protected host or official oracle evidence.
//! The host must independently join original /proc identities and containment
//! resources before creating the fixed root-owned observer gate. Neither a peer
//! outcome nor a probe's accepted flag authenticates kill, cancellation, closure,
//! or absence of host mounts.
use serde::{Deserialize, Serialize};
pub use crate::authority::CalibrationCase as Case;

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
        if self.pid < 2 || self.parent_pid == 0 || self.pid == self.parent_pid
            || self.process_group == 0 || self.session == 0 || self.start_time_ticks == 0
            || self.executable_device == 0 || self.executable_inode == 0 {
            return Err("missing original peer process identity".into());
        }
        Ok(())
    }
}
impl Report {
    /// Validates only finite final peer self-observations. The caller still
    /// needs independently authenticated original-host identity and teardown.
    pub fn validate_case(&self, expected: Case) -> Result<(), String> {
        if self.schema_version != 1 || self.scope != "PR174_RUST_CALIBRATION_PEER"
            || self.case != expected || self.first_failure.is_some()
            || !self.observer_gate_observed || self.elapsed_ms >= 300_000 {
            return Err("invalid final peer scope, gate, error, case, or lifetime".into());
        }
        let outcome = if expected.forced() { Outcome::AwaitingHostAction } else { Outcome::Completed };
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
            if child.pid == parent.pid || child.parent_pid != parent.pid
                || child.start_time_ticks < parent.start_time_ticks
                || child.executable_device != parent.executable_device
                || child.executable_inode != parent.executable_inode {
                return Err("descendant original executable, parent, or start identity mismatch".into());
            }
            let valid_group = match expected {
                Case::SameGroup => child.process_group == parent.process_group && child.session == parent.session,
                Case::Setsid => child.process_group == child.pid && child.session == child.pid && child.session != parent.session,
                Case::Setpgid => child.process_group == child.pid && child.process_group != parent.process_group && child.session == parent.session,
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
                let exhaustion = self.exhaustion.as_ref().ok_or("missing real exhaustion observation")?;
                if !exhaustion.reached_enospc || !exhaustion_reached(exhaustion.terminal_errno) {
                    return Err("exhaustion did not terminate with exact ENOSPC".into());
                }
                let valid = if expected == Case::StorageExhaustion {
                    exhaustion.bytes_written > 0 && exhaustion.bytes_written < 536_870_912 && exhaustion.files_created == 1
                } else {
                    exhaustion.bytes_written == 0 && exhaustion.files_created > 0 && exhaustion.files_created < 16_384
                };
                if !valid {
                    return Err("exhaustion counters do not fit the exact bounded resource".into());
                }
            }
            _ if self.exhaustion.is_some() => return Err("unexpected exhaustion observation".into()),
            _ => {}
        }
        if expected == Case::BoundaryDenials {
            self.validate_denials()?;
        } else if !self.probes.is_empty() {
            return Err("unexpected boundary probes outside boundary-denials".into());
        }
        Ok(())
    }
    fn probe(&self, name: &str, operation: &str) -> Result<&Probe, String> {
        // Fixed ceiling of 34 entries; scanning avoids allocating a second
        // attacker-controlled name map for this deliberately closed schema.
        let mut matching = self.probes.iter().filter(|probe| probe.name == name);
        let probe = matching.next().ok_or_else(|| format!("missing boundary probe {name}"))?;
        if matching.next().is_some() || probe.operation != operation || !probe.accepted {
            return Err(format!("duplicate, rejected, or wrong-operation boundary probe {name}"));
        }
        Ok(probe)
    }
    fn denied_probe(&self, name: &str, operation: &str, classifier: fn(Option<i32>) -> bool) -> Result<(), String> {
        let probe = self.probe(name, operation)?;
        if probe.return_value != -1 || !classifier(probe.errno) {
            return Err(format!("boundary probe {name} has no actual expected denial"));
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
        self.denied_probe("host-proc-unavailable", "open /host/proc/1/stat", absent_or_denied)?;
        self.denied_probe("ancestor-pid-namespace", "ioctl NS_GET_PARENT", namespace_denied)?;
        let mut count = 3; // two denials above plus the mandatory egress socket.
        if self.socket_created("egress-socket")? {
            self.denied_probe("network-egress", "connect 203.0.113.1:443", egress_denied)?;
            count += 1;
        }
        for path in ["/run/docker.sock", "/var/run/docker.sock"] {
            count += 1;
            if self.socket_created(&format!("daemon-socket-{path}"))? {
                self.denied_probe(&format!("daemon-connect-{path}"), "connect fixed daemon socket", absent_or_denied)?;
                count += 1;
            }
        }
        for input in ["fixture/declarations.kt", "fixture/error-node.kt", "preflight/identity.json"] {
            self.denied_probe(input, "open existing input O_WRONLY", permission_denied)?;
            count += 1;
        }
        for root in ["fixture", "distribution", "tool", "preflight"] {
            self.denied_probe(&format!("rename-{root}"), "rename mountpoint", mountpoint_denied)?;
            self.denied_probe(&format!("remove-{root}"), "rmdir mountpoint", mountpoint_denied)?;
            self.denied_probe(&format!("unmount-{root}"), "umount2 mountpoint", namespace_denied)?;
            self.denied_probe(&format!("bind-over-{root}"), "mount MS_BIND over input", namespace_denied)?;
            count += 4;
        }
        for namespace in ["user", "mount", "pid", "network", "ipc", "uts", "cgroup"] {
            self.denied_probe(&format!("unshare-{namespace}"), "unshare namespace", namespace_denied)?;
            count += 1;
        }
        if count != self.probes.len() {
            return Err("unknown or inapplicable conditional boundary probes".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic(case: Case) -> Report {
        let parent = ProcessIdentity {
            pid: 2, parent_pid: 1, process_group: 1, session: 1,
            start_time_ticks: 10, executable_device: 1, executable_inode: 20,
        };
        let mut report = Report {
            schema_version: 1, scope: "PR174_RUST_CALIBRATION_PEER".into(), case,
            outcome: if case.forced() { Outcome::AwaitingHostAction } else { Outcome::Completed },
            original_process: parent.clone(), children: Vec::new(), observer_gate_observed: true,
            probes: Vec::new(), exhaustion: None, first_failure: None, elapsed_ms: 1,
        };
        if matches!(case, Case::SameGroup | Case::Setsid | Case::Setpgid) {
            let mut child = ProcessIdentity { pid: 3, parent_pid: 2, start_time_ticks: 11, ..parent };
            if case != Case::SameGroup { child.process_group = 3; }
            if case == Case::Setsid { child.session = 3; }
            report.children.push(child);
        }
        match case {
            Case::StorageExhaustion => report.exhaustion = Some(Exhaustion {
                bytes_written: 4096, files_created: 1, terminal_errno: Some(28), reached_enospc: true,
            }),
            Case::InodeExhaustion => report.exhaustion = Some(Exhaustion {
                bytes_written: 0, files_created: 10, terminal_errno: Some(28), reached_enospc: true,
            }),
            Case::BoundaryDenials => {
                let mut deny = |name: String, operation: &str| report.probes.push(Probe {
                    name, operation: operation.into(), return_value: -1, errno: Some(1), accepted: true,
                });
                deny("host-proc-unavailable".into(), "open /host/proc/1/stat");
                deny("ancestor-pid-namespace".into(), "ioctl NS_GET_PARENT");
                deny("egress-socket".into(), "socket");
                for path in ["/run/docker.sock", "/var/run/docker.sock"] {
                    deny(format!("daemon-socket-{path}"), "socket");
                }
                for input in ["fixture/declarations.kt", "fixture/error-node.kt", "preflight/identity.json"] {
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
            invalid.case = if case == Case::Normal { Case::Kill } else { Case::Normal };
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
            invalid.outcome = if case.forced() { Outcome::Completed } else { Outcome::AwaitingHostAction };
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
        for (return_value, errno, accepted) in [(0, None, true), (-1, Some(22), true), (-1, Some(1), false)] {
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
            ("egress-socket", "network-egress", "connect 203.0.113.1:443", 101),
            ("daemon-socket-/run/docker.sock", "daemon-connect-/run/docker.sock", "connect fixed daemon socket", 2),
            ("daemon-socket-/var/run/docker.sock", "daemon-connect-/var/run/docker.sock", "connect fixed daemon socket", 2),
        ] {
            let socket = report.probes.iter_mut().find(|p| p.name == socket_name).unwrap();
            socket.return_value = 3;
            socket.errno = None;
            assert!(report.validate_case(Case::BoundaryDenials).is_err());
            report.probes.push(Probe {
                name: connect_name.into(), operation: operation.into(), return_value: -1, errno: Some(errno), accepted: true,
            });
            assert!(report.validate_case(Case::BoundaryDenials).is_ok());
        }
        assert_eq!(report.probes.len(), 34);
        let mut invalid = report.clone();
        invalid.probes.iter_mut().find(|p| p.name == "network-egress").unwrap().errno = Some(115);
        assert!(invalid.validate_case(Case::BoundaryDenials).is_err());
        let mut invalid = report;
        let socket = invalid.probes.iter_mut().find(|p| p.name == "egress-socket").unwrap();
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
}

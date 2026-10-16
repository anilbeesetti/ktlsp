//! Source authority is independent ROOT input, not a caller-supplied digest.
//! Protected workflow/OS-root custody is a trust assumption; runtime facts remain measured evidence.
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{collections::BTreeSet, path::{Component, Path}};

pub const CONTRACT_SHA256: &str = "65eb16c5bd73c99fe7fc000fddf0242bf6abab09283676612edf5ce9f285b61d";
pub const WORKFLOW_PATH: &str = ".github/workflows/t045-pr174-official-symbols.yml";
pub const AUTHORITY_BYTES: usize = 16_384;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all="camelCase")]
pub struct OriginalRun {
    pub repository:String,pub event:String,pub attempt:u8,pub run_id:u64,pub job_id:u64,
    pub suite_id:u64,pub source_head:String,pub source_tree:String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all="camelCase")]
pub struct ExportFile {pub path:String,pub bytes:u64,pub sha256:String}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all="camelCase")]
pub struct ExportBinding {
    pub schema_version:u8,pub scope:String,pub run:OriginalRun,pub nonce:String,pub container_id:String,
    pub boot_id:String,pub daemon_id:String,pub image:String,pub image_id:String,
    pub root_authorization_sha256:String,pub calibration_sha256:String,pub workflow_sha256:String,
    pub files:Vec<ExportFile>,
}

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub struct CalibrationAction {
    pub schema_version:u8,pub scope:String,pub case:CalibrationCase,pub nonce:String,pub container_id:String,
    pub boot_id:String,pub daemon_id:String,pub init_pid:u32,pub init_starttime:u64,
    pub armed_ms:u64,pub deadline_ms:u64,pub observed_ms:u64,pub observed_unix_ms:u64,pub original_running:bool,
}
impl CalibrationAction {
    pub fn validate(&self)->Result<(),String> {
        if self.schema_version!=1||self.scope!="PR174_RUST_CALIBRATION_ACTION"||!self.case.forced()||!digest(&self.nonce)||!digest(&self.container_id)||self.boot_id.is_empty()||self.daemon_id.is_empty()||self.init_pid<=1||self.init_starttime==0||!self.original_running||self.observed_unix_ms==0||self.armed_ms.checked_add(300000)!=Some(self.deadline_ms)||self.observed_ms<self.armed_ms||self.observed_ms>=self.armed_ms.saturating_add(540000)||(self.case==CalibrationCase::Kill&&self.observed_ms>=self.deadline_ms)||(self.case!=CalibrationCase::Kill&&self.observed_ms<self.deadline_ms) {
            return Err("actual live original calibration action identity/deadline incomplete".into());
        }
        Ok(())
    }
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    if bytes.len() > AUTHORITY_BYTES { return Err("authority byte limit".into()); }
    serde_json::from_slice(bytes).map_err(|e| format!("strict authority JSON: {e}"))
}
pub fn digest(value: &str) -> bool { hex(value, 64) && value.bytes().any(|b| b != b'0') }
fn hex(value: &str, len: usize) -> bool {
    value.len() == len && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrantScope {
    #[serde(rename = "PR174_OFFICIAL_CAPTURE")]
    OfficialCapture,
    #[serde(rename = "PR174_RUST_CALIBRATION")]
    RustCalibration,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CalibrationCase { Normal, SameGroup, Setsid, Setpgid, Timeout, Kill, WorkflowStepCancel, StorageExhaustion, InodeExhaustion, BoundaryDenials }
impl CalibrationCase {
    pub const ALL: [Self; 10] = [Self::Normal,Self::SameGroup,Self::Setsid,Self::Setpgid,Self::Timeout,Self::Kill,Self::WorkflowStepCancel,Self::StorageExhaustion,Self::InodeExhaustion,Self::BoundaryDenials];
    pub fn as_str(self) -> &'static str {
        match self { Self::Normal=>"normal",Self::SameGroup=>"same-group",Self::Setsid=>"setsid",Self::Setpgid=>"setpgid",Self::Timeout=>"timeout",Self::Kill=>"kill",Self::WorkflowStepCancel=>"workflow-step-cancel",Self::StorageExhaustion=>"storage-exhaustion",Self::InodeExhaustion=>"inode-exhaustion",Self::BoundaryDenials=>"boundary-denials" }
    }
    pub fn forced(self) -> bool { matches!(self, Self::Timeout|Self::Kill|Self::WorkflowStepCancel) }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ToolSeal { pub role: String, pub path: String, pub sha256: String }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HostSeal { pub os_release_sha256: String, pub kernel_release_sha256: String, pub architecture: String }
impl HostSeal {
    pub fn validate(&self) -> Result<(), String> {
        if self.architecture != "x86_64" || !digest(&self.os_release_sha256) || !digest(&self.kernel_release_sha256) { return Err("host template seal".into()); }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CalibrationStep { pub name: String, pub number: u32 }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RootGrant {
    pub schema_version: u8, pub scope: GrantScope,
    pub repository: String, pub event: String, pub attempt: u8,
    pub run_id: u64, pub job_id: u64, pub suite_id: u64,
    pub source_head: String, pub source_tree: String,
    pub workflow_path: String, pub workflow_sha256: String, pub contract_sha256: String,
    pub host_binary_sha256: String, pub adapter_sha256: String, pub peer_sha256: String,
    pub lock_sha256: String, pub toolchain_sha256: String, pub containment_template_sha256: String,
    pub image: String, pub image_id: String, pub tools: Vec<ToolSeal>,
    pub required_calibration_sha256: Option<String>, pub calibration_case: Option<CalibrationCase>,
    pub calibration_step: Option<CalibrationStep>,
    pub host: HostSeal,
}
impl RootGrant {
    /// `expected` must originate independently from ROOT's admitted source/job grant.
    /// Passing the same untrusted document twice authenticates nothing.
    pub fn validate(&self, expected: &Self) -> Result<(), String> {
        self.validate_shape()?;
        if self != expected { return Err("ROOT independently admitted grant mismatch".into()); }
        Ok(())
    }
    pub fn validate_shape(&self) -> Result<(), String> {
        if self.schema_version != 1 || self.repository != "anilbeesetti/ktlsp" || self.event != "workflow_dispatch" || self.attempt != 1 || self.run_id == 0 || self.job_id == 0 || self.suite_id == 0 || !hex(&self.source_head,40) || !hex(&self.source_tree,40) || self.workflow_path != WORKFLOW_PATH || self.contract_sha256 != CONTRACT_SHA256 {
            return Err("ROOT grant original-run/source identity".into());
        }
        for d in [&self.workflow_sha256,&self.host_binary_sha256,&self.adapter_sha256,&self.peer_sha256,&self.lock_sha256,&self.toolchain_sha256,&self.containment_template_sha256] { if !digest(d) { return Err("ROOT grant source/tool seal".into()); } }
        if !self.image_id.strip_prefix("sha256:").is_some_and(digest) || !self.image.rsplit_once("@sha256:").is_some_and(|(name,d)| !name.is_empty() && !name.starts_with('-') && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"/._:-".contains(&b)) && digest(d)) { return Err("ROOT grant immutable image/config identity".into()); }
        let mut roles = BTreeSet::new();
        for tool in &self.tools {
            let path = Path::new(&tool.path);
            if !path.is_absolute() || path.components().any(|c| !matches!(c,Component::RootDir|Component::Normal(_))) || !tool.path.bytes().all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b)) || path.file_name().and_then(|s|s.to_str()) != Some(tool.role.as_str()) || !digest(&tool.sha256) || !roles.insert(tool.role.as_str()) { return Err("ROOT grant exact tool custody/seal".into()); }
        }
        if roles != BTreeSet::from(["docker","systemctl","sha256sum","gh","mount","findmnt","stat","curl"]) { return Err("ROOT grant tool roles".into()); }
        match self.scope {
            GrantScope::OfficialCapture if self.calibration_case.is_none() && self.required_calibration_sha256.as_deref().is_some_and(digest) => {},
            GrantScope::RustCalibration if self.calibration_case.is_some() && self.required_calibration_sha256.is_none() => {},
            _ => return Err("official/calibration authority scope separation".into()),
        }
        if (self.calibration_case == Some(CalibrationCase::WorkflowStepCancel)) != self.calibration_step.is_some()
            || self.calibration_step.as_ref().is_some_and(|step| step.number == 0 || step.name.is_empty() || step.name.len() > 256 || step.name.chars().any(char::is_control)) {
            return Err("finite original workflow cancellation-step identity".into());
        }
        self.host.validate()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CalibrationPlan { pub schema_version: u8, pub scope: GrantScope, pub case: CalibrationCase }
impl CalibrationPlan {
    pub fn validate(&self, grant: &RootGrant) -> Result<(), String> {
        if self.schema_version != 1 || self.scope != GrantScope::RustCalibration || grant.scope != GrantScope::RustCalibration || grant.calibration_case != Some(self.case) { return Err("calibration plan is not this finite ROOT grant".into()); }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CalibrationCaseReceipt {
    pub case: CalibrationCase, pub run_id: u64, pub job_id: u64, pub suite_id: u64, pub attempt: u8,
    pub nonce: String, pub container_id: String, pub observation_sha256: String,
    pub teardown_sha256: String, pub export_sha256: String,
    pub normal_exit: bool, pub closure_confirmed: bool, pub denials_confirmed: bool,
    pub workflow_cancellation_authenticated: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CalibrationReport {
    pub schema_version: u8, pub scope: GrantScope, pub source_head: String, pub source_tree: String,
    pub workflow_sha256: String, pub host_binary_sha256: String, pub peer_sha256: String,
    pub containment_template_sha256: String, pub image: String, pub image_id: String,
    pub host: HostSeal, pub boot_id: String, pub daemon_id: String,
    pub cases: Vec<CalibrationCaseReceipt>,
}
impl CalibrationReport {
    /// Checks protected report semantics only, never GitHub origin or actual raw closure.
    pub fn validate(&self, expected: &RootGrant) -> Result<(), String> {
        expected.validate_shape()?;
        if self.schema_version != 1 || self.scope != GrantScope::RustCalibration || self.source_head != expected.source_head || self.source_tree != expected.source_tree || self.workflow_sha256 != expected.workflow_sha256 || self.host_binary_sha256 != expected.host_binary_sha256 || self.peer_sha256 != expected.peer_sha256 || self.containment_template_sha256 != expected.containment_template_sha256 || self.image != expected.image || self.image_id != expected.image_id || self.host != expected.host || self.boot_id.is_empty() || self.boot_id.len() > 128 || self.daemon_id.is_empty() || self.daemon_id.len() > 128 { return Err("calibration source/image/host identity mismatch".into()); }
        let mut cases = BTreeSet::new(); let mut nonces = BTreeSet::new(); let mut containers = BTreeSet::new();
        for receipt in &self.cases {
            if receipt.run_id != expected.run_id || receipt.job_id != expected.job_id || receipt.suite_id != expected.suite_id || receipt.attempt != expected.attempt {
                return Err("calibration receipt is not the same independently admitted original job".into());
            }
            if !cases.insert(receipt.case) || !nonces.insert(&receipt.nonce) || !containers.insert(&receipt.container_id) || !digest(&receipt.nonce) || !digest(&receipt.container_id) || receipt.run_id == 0 || receipt.job_id == 0 || receipt.suite_id == 0 || receipt.attempt != 1 || !receipt.closure_confirmed || receipt.normal_exit == receipt.case.forced() || receipt.denials_confirmed != (receipt.case == CalibrationCase::BoundaryDenials) || receipt.workflow_cancellation_authenticated != (receipt.case == CalibrationCase::WorkflowStepCancel) || [&receipt.observation_sha256,&receipt.teardown_sha256,&receipt.export_sha256].iter().any(|s| !digest(s)) { return Err("calibration case missing actual complete closure/denial/cancellation evidence".into()); }
        }
        if cases != BTreeSet::from(CalibrationCase::ALL) { return Err("calibration requires every finite case exactly once".into()); }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_plan_and_scope_refusals_are_synthetic_not_calibration() {
        let p: CalibrationPlan = decode(br#"{"schemaVersion":1,"scope":"PR174_RUST_CALIBRATION","case":"normal"}"#).unwrap();
        assert_eq!(p.case, CalibrationCase::Normal);
        assert!(decode::<CalibrationPlan>(br#"{"schemaVersion":1,"schemaVersion":1,"scope":"PR174_RUST_CALIBRATION","case":"normal"}"#).is_err());
        assert!(decode::<CalibrationPlan>(br#"{"schemaVersion":1,"scope":"PR174_RUST_CALIBRATION","case":"normal","command":"sh"}"#).is_err());
        assert!(decode::<CalibrationPlan>(br#"{"schemaVersion":1,"scope":"PR174_RUST_CALIBRATION","case":"arbitrary"}"#).is_err());
        assert!(decode::<CalibrationPlan>(&vec![b' '; AUTHORITY_BYTES+1]).is_err());
        assert_eq!(CalibrationCase::ALL.into_iter().collect::<BTreeSet<_>>().len(),10);
        assert!(!digest(&"0".repeat(64)));
    }
    fn synthetic_grant() -> RootGrant {
        let seal="1".repeat(64);
        RootGrant { schema_version:1,scope:GrantScope::RustCalibration,repository:"anilbeesetti/ktlsp".into(),event:"workflow_dispatch".into(),attempt:1,run_id:11,job_id:12,suite_id:13,source_head:"2".repeat(40),source_tree:"3".repeat(40),workflow_path:WORKFLOW_PATH.into(),workflow_sha256:seal.clone(),contract_sha256:CONTRACT_SHA256.into(),host_binary_sha256:seal.clone(),adapter_sha256:seal.clone(),peer_sha256:seal.clone(),lock_sha256:seal.clone(),toolchain_sha256:seal.clone(),containment_template_sha256:seal.clone(),image:format!("synthetic/image@sha256:{seal}"),image_id:format!("sha256:{seal}"),tools:["docker","systemctl","sha256sum","gh","mount","findmnt","stat","curl"].into_iter().map(|role|ToolSeal{role:role.into(),path:format!("/usr/bin/{role}"),sha256:seal.clone()}).collect(),required_calibration_sha256:None,calibration_case:Some(CalibrationCase::Normal),calibration_step:None,host:HostSeal{os_release_sha256:seal.clone(),kernel_release_sha256:seal,architecture:"x86_64".into()} }
    }
    #[test]
    fn synthetic_expected_grant_scope_and_replay_refusals_are_not_authorization() {
        let grant=synthetic_grant();
        assert!(grant.validate(&grant).is_ok()); // Deliberately synthetic self-consistency, never authority.
        let mut independently_expected=grant.clone();
        independently_expected.source_head="4".repeat(40);
        assert!(grant.validate(&independently_expected).is_err());
        let plan=CalibrationPlan{schema_version:1,scope:GrantScope::RustCalibration,case:CalibrationCase::Normal};
        assert!(plan.validate(&grant).is_ok()); // Calibration has no circular prior-report dependency.
        let mut official=grant.clone();official.scope=GrantScope::OfficialCapture;official.calibration_case=None;
        assert!(official.validate_shape().is_err());
        official.required_calibration_sha256=Some("5".repeat(64));
        assert!(official.validate_shape().is_ok());
        assert!(plan.validate(&official).is_err());
        let mut cancel=grant;cancel.calibration_case=Some(CalibrationCase::WorkflowStepCancel);
        assert!(cancel.validate_shape().is_err());
        cancel.calibration_step=Some(CalibrationStep{name:"actual runner timeout step".into(),number:7});
        assert!(cancel.validate_shape().is_ok());
    }
    #[test]
    fn synthetic_host_profile_requires_exact_source_policy_and_independent_bindings() {
        let grant=synthetic_grant();let nonce="6".repeat(64);
        let measurement=ConfinementMeasurement{schema_version:1,scope:"PR174_HOST_CHILD_CONFINEMENT_PROFILE".into(),kind:"child-confinement-preflight".into(),nonce:nonce.clone(),boot_id:"synthetic-boot".into(),daemon_id:"synthetic-daemon".into(),profile:crate::recorder::confinement::profile(),landlock_abi:6,containment_template_sha256:grant.containment_template_sha256.clone(),source_head:grant.source_head.clone(),source_tree:grant.source_tree.clone(),host_binary_sha256:grant.host_binary_sha256.clone(),adapter_sha256:grant.adapter_sha256.clone(),peer_sha256:grant.peer_sha256.clone()};
        assert!(measurement.validate(&grant,&nonce,"synthetic-boot","synthetic-daemon").is_ok()); // Data consistency only.
        assert!(measurement.validate(&grant,&nonce,"different-boot","synthetic-daemon").is_err());
        let mut invalid=measurement.clone();invalid.landlock_abi=5;
        assert!(invalid.validate(&grant,&nonce,"synthetic-boot","synthetic-daemon").is_err());
        let mut invalid=measurement.clone();invalid.profile.scoped=0;
        assert!(invalid.validate(&grant,&nonce,"synthetic-boot","synthetic-daemon").is_err());
        let mut invalid=measurement;invalid.source_tree="7".repeat(40);
        assert!(invalid.validate(&grant,&nonce,"synthetic-boot","synthetic-daemon").is_err());
    }
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub struct ConfinementMeasurement {
    pub schema_version:u8,pub scope:String,pub kind:String,pub nonce:String,pub boot_id:String,pub daemon_id:String,
    pub profile:crate::recorder::confinement::Profile,pub landlock_abi:u32,pub containment_template_sha256:String,
    pub source_head:String,pub source_tree:String,pub host_binary_sha256:String,pub adapter_sha256:String,pub peer_sha256:String,
}
impl ConfinementMeasurement {
    /// Source policy + actual host ABI only. Child enforcement still requires the original
    /// protected recorder receipt and the real same-template Rust denial calibration.
    pub fn validate(&self,grant:&RootGrant,nonce:&str,boot:&str,daemon:&str)->Result<(),String> {
        grant.validate_shape()?;
        if self.schema_version!=1||self.scope!="PR174_HOST_CHILD_CONFINEMENT_PROFILE"||self.kind!="child-confinement-preflight"||self.nonce!=nonce||self.boot_id!=boot||self.daemon_id!=daemon||self.profile!=crate::recorder::confinement::profile()||self.landlock_abi<self.profile.minimum_abi||self.containment_template_sha256!=grant.containment_template_sha256||self.source_head!=grant.source_head||self.source_tree!=grant.source_tree||self.host_binary_sha256!=grant.host_binary_sha256||self.adapter_sha256!=grant.adapter_sha256||self.peer_sha256!=grant.peer_sha256 {
            return Err("original host ABI/source/profile measurement mismatch; not child enforcement proof".into());
        }
        Ok(())
    }
}

pub fn systemctl_usec(s: &str) -> Result<u64, String> {
        if s.is_empty() || s.len() > 128 {
            return Err("systemctl timespan length".into());
        }
        let mut total = 0_u64;
        let mut previous = u64::MAX;
        for (index, part) in s.split(' ').enumerate() {
            if index >= 6 {
                return Err("systemctl timespan unit count".into());
            }
            let (number, factor) = [
                ("us", 1_u64),
                ("µs", 1),
                ("ms", 1000),
                ("s", 1_000_000),
                ("min", 60_000_000),
                ("h", 3_600_000_000),
                ("d", 86_400_000_000),
            ]
            .into_iter()
            .find_map(|(unit, factor)| part.strip_suffix(unit).map(|n| (n, factor)))
            .ok_or("unsupported systemctl timespan unit")?;
            if factor >= previous {
                return Err("repeated/unordered systemctl timespan unit".into());
            }
            previous = factor;
            let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
            if whole.is_empty()
                || !whole.bytes().all(|b| b.is_ascii_digit())
                || fraction.len() > 6
                || !fraction.bytes().all(|b| b.is_ascii_digit())
                || (number.contains('.') && fraction.is_empty())
            {
                return Err("malformed systemctl timespan number".into());
            }
            let whole = whole.parse::<u64>().map_err(|_| "timespan overflow")?;
            let mut value = whole.checked_mul(factor).ok_or("timespan overflow")?;
            if !fraction.is_empty() {
                let digits = fraction.parse::<u64>().map_err(|_| "timespan fraction")?;
                let divisor = 10_u64.pow(fraction.len() as u32);
                let scaled = digits.checked_mul(factor).ok_or("timespan overflow")?;
                if scaled % divisor != 0 {
                    return Err("submicrosecond systemctl timespan".into());
                }
                value = value
                    .checked_add(scaled / divisor)
                    .ok_or("timespan overflow")?;
            }
            total = total.checked_add(value).ok_or("timespan overflow")?;
        }
        if total == 0 {
            return Err("zero systemctl timespan".into());
        }
        Ok(total)
    }

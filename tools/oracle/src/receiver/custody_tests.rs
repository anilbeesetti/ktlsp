//! Fabricated API/log records only. They cannot construct AuthenticatedArtifact.
use super::*;
use crate::receiver::{UploaderStep, ProtectedUploadReceipt};
use crate::authority::{RootGrant, ExportBinding, OriginalRun};
use serde_json::json;
fn fixture() -> (ReceiverAdmission,Files,Value,ProtectedUploadReceipt) {
    let mut g=json!({"schemaVersion":1,"scope":"PR174_RUST_CALIBRATION","repository":"anilbeesetti/ktlsp","event":"workflow_dispatch","attempt":1,"runId":11,"jobId":12,"suiteId":13,"sourceHead":"1".repeat(40),"sourceTree":"2".repeat(40),"workflowPath":crate::authority::WORKFLOW_PATH,"contractSha256":crate::authority::CONTRACT_SHA256,"image":format!("example@sha256:{}","a".repeat(64)),"imageId":format!("sha256:{}","b".repeat(64)),"tools":[],"requiredCalibrationSha256":null,"calibrationCase":"normal","calibrationStep":null,"host":{"osReleaseSha256":"c".repeat(64),"kernelReleaseSha256":"d".repeat(64),"architecture":"x86_64"}});
    for key in ["workflowSha256","hostBinarySha256","adapterSha256","peerSha256","lockSha256","toolchainSha256","containmentTemplateSha256"] {g[key]=json!("e".repeat(64));}
    g["tools"]=json!(["docker","systemctl","sha256sum","gh","mount","findmnt","stat","curl"].map(|role|json!({"role":role,"path":format!("/usr/bin/{role}"),"sha256":"f".repeat(64)})));
    let g: RootGrant=serde_json::from_value(g).unwrap();
    let admission=ReceiverAdmission{schema_version:1,scope:"PR174_RECEIVER_ADMISSION".into(),grant:g.clone(),uploader_step:UploaderStep{name:"ROOT protected post-upload receipt".into(),number:2}};
    let mut files=Files::new();
    files.insert("root-authorization.json".into(),serde_json::to_vec(&g).unwrap());
    let root_hash=hash(&files["root-authorization.json"]);
    let binding=ExportBinding{schema_version:1,scope:"PR174_PROTECTED_EXPORT".into(),run:OriginalRun{repository:g.repository.clone(),event:g.event.clone(),attempt:1,run_id:11,job_id:12,suite_id:13,source_head:g.source_head.clone(),source_tree:g.source_tree.clone()},nonce:"3".repeat(64),container_id:"4".repeat(64),boot_id:"synthetic-boot".into(),daemon_id:"synthetic-daemon".into(),image:g.image.clone(),image_id:g.image_id.clone(),root_authorization_sha256:root_hash.clone(),calibration_sha256:"5".repeat(64),workflow_sha256:g.workflow_sha256.clone(),files:Vec::new()};
    files.insert("export-binding.json".into(),serde_json::to_vec(&binding).unwrap());
    let job=json!({"steps":[{"number":1,"name":"Set up job","status":"completed","conclusion":"success","started_at":"2026-10-16T00:00:00Z","completed_at":"2026-10-16T00:00:01Z"},{"number":2,"name":admission.uploader_step.name,"status":"completed","conclusion":"success","started_at":"2026-10-16T00:00:10Z","completed_at":"2026-10-16T00:00:20Z"}]});
    let receipt=ProtectedUploadReceipt{schema_version:1,scope:"PR174_PROTECTED_UPLOAD".into(),repository:g.repository,run_id:11,job_id:12,suite_id:13,attempt:1,source_head:g.source_head,source_tree:g.source_tree,workflow_sha256:g.workflow_sha256,artifact_id:16,artifact_api_digest:format!("sha256:{}","6".repeat(64)),download_zip_sha256:None,export_binding_sha256:hash(&files["export-binding.json"]),nonce:binding.nonce,container_id:binding.container_id,root_grant_sha256:root_hash,uploader_step:admission.uploader_step.clone()};
    (admission,files,job,receipt)
}
fn logs(receipt: &Value) -> Vec<u8> {format!("2026-10-16T00:00:15.123Z {UPLOAD_PREFIX}{receipt}\n").into_bytes()}
#[test]
fn exact_original_step_receipt_is_required_not_matching_artifact_metadata() {
    let (a,f,j,r)=fixture();a.validate().unwrap();
    assert_eq!(step(&j,&a.uploader_step.name,2).unwrap().0,1);
    let r=serde_json::to_value(r).unwrap();let valid=logs(&r);
    let api=format!("sha256:{}","6".repeat(64));let zip="6".repeat(64);
    assert!(upload(&a,16,&api,&zip,&f,&j,&valid,&valid).is_ok());
    let mut cohort_job=j.clone();
    cohort_job["steps"].as_array_mut().unwrap().push(json!({"number":3,"name":"ROOT other calibration upload","status":"completed","conclusion":"success","started_at":"2026-10-16T00:00:30Z","completed_at":"2026-10-16T00:00:40Z"}));
    let mut other=r.clone();other["artifactId"]=json!(17);other["nonce"]=json!("7".repeat(64));other["containerId"]=json!("8".repeat(64));other["exportBindingSha256"]=json!("9".repeat(64));other["uploaderStep"]=json!({"name":"ROOT other calibration upload","number":3});
    let mut cohort=valid.clone();cohort.extend(format!("2026-10-16T00:00:35.123Z {UPLOAD_PREFIX}{other}\n").as_bytes());
    assert!(upload(&a,16,&api,&zip,&f,&cohort_job,&cohort,&valid).is_ok());
    let different_timestamp=String::from_utf8(valid.clone()).unwrap().replace("00:00:15.123","00:00:15.124");
    assert!(upload(&a,16,&api,&zip,&f,&j,&valid,different_timestamp.as_bytes()).is_err());
    for (field,value) in [("schemaVersion",json!(2)),("scope",json!("synthetic")),("runId",json!(99)),("jobId",json!(99)),("suiteId",json!(99)),("attempt",json!(2)),("sourceHead",json!("7".repeat(40))),("sourceTree",json!("8".repeat(40))),("workflowSha256",json!("9".repeat(64))),("artifactId",json!(17)),("artifactApiDigest",json!(format!("sha256:{}","7".repeat(64)))),("downloadZipSha256",json!("7".repeat(64))),("exportBindingSha256",json!("7".repeat(64))),("rootGrantSha256",json!("7".repeat(64))),("nonce",json!("7".repeat(64))),("containerId",json!("7".repeat(64)))] {
        let mut changed=r.clone();changed[field]=value;let log=logs(&changed);assert!(upload(&a,16,&api,&zip,&f,&j,&log,&log).is_err(),"{field}");
    }
    let mut duplicate=valid.clone();duplicate.extend(&valid);
    for bad in [Vec::new(),duplicate,valid[..valid.len()-1].to_vec(),String::from_utf8(valid.clone()).unwrap().replace("00:00:15","00:00:09").into_bytes()] {assert!(upload(&a,16,&api,&zip,&f,&j,&bad,&valid).is_err());}
    assert!(upload(&a,16,&api,&zip,&f,&j,&valid,b"missing exact step logs\n").is_err());
    let mut bad=j.clone();bad["steps"][1]["name"]=json!("untrusted other step");assert!(upload(&a,16,&api,&zip,&f,&bad,&valid,&valid).is_err());
    let mut bad=j;bad["steps"][1]["number"]=json!(1);assert!(upload(&a,16,&api,&zip,&f,&bad,&valid,&valid).is_err());
}
#[test]
fn actual_runner_timeout_requires_exact_step_not_exit130_or_whole_job_text() {
    let (a,_,j,_)=fixture();
    let line=format!("2026-10-16T00:00:15.123Z ##[error]The action '{}' has timed out after 1 minutes.\n",a.uploader_step.name).into_bytes();
    assert!(timeout(&j,&a.uploader_step.name,2,&line,&line).is_ok());
    for bad in [b"exit 130\n".as_slice(),b"The operation was canceled.\n",b"self-authored timeout\n"] {assert!(timeout(&j,&a.uploader_step.name,2,bad,&line).is_err());}
    assert!(timeout(&j,&a.uploader_step.name,2,&line,b"missing\n").is_err());
}

#[test]
fn original_timestamp_conversion_keeps_real_step_and_action_order() {
    assert_eq!(unix_ms("1970-01-01T00:00:00Z").unwrap(),0);
    assert_eq!(unix_ms("1970-01-01T00:00:00.123456Z").unwrap(),123);
    assert_eq!(unix_ms("2000-01-01T00:00:00Z").unwrap(),946684800000);
    assert!(unix_ms("2000-02-29T00:00:00Z").is_ok());
    for invalid in ["1969-12-31T23:59:59Z","2001-02-29T00:00:00Z","2026-13-01T00:00:00Z","2026-10-16T24:00:00Z","2026-10-16T00:00:00.Z"] {assert!(unix_ms(invalid).is_err(),"{invalid}");}
}

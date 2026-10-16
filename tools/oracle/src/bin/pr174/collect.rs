#[derive(Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
struct CalibrationCollection { cases:Vec<CalibrationLocation> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
struct CalibrationLocation { case:CalibrationCase, nonce:String }

fn read_host_observation(dir:&Path,case:CalibrationCase,cid:&str)->Result<Value> {
    let raw=read_file(&dir.join("receipts/observer.evidence"),OBSERVER_BYTES)?;
    let mut found=None;
    for line in raw.split(|b|*b==b'\n').filter(|line|!line.is_empty()) {
        let value:Value=parse(line)?;
        if value["kind"]=="calibration-host-observation" {
            if found.is_some() || value["case"]!=case.as_str() || value["containerId"]!=cid || value["scope"]!="PR174_RUST_CALIBRATION" {return Err("original calibration observation case/CID duplicated/mismatched".into());}found=Some(value);
        }
    }
    found.ok_or("real host calibration observation absent".into())
}
fn authenticate_step_cancellation(dir:&Path,a:&Admission,grant:&RootGrant,e:&mut Evidence)->Result<()> {
    let step=grant.calibration_step.as_ref().ok_or("ROOT cancellation step identity absent")?;
    let token=String::from_utf8(read_file(&dir.join("github.token"),4096)?).map_err(|_|"credential encoding")?;
    let token=token.trim();if token.is_empty()||token.bytes().any(|b|b.is_ascii_control()){return Err("original cancellation API credential".into());}
    let job:Value=parse(&gh(e,a,token,format!("repos/anilbeesetti/ktlsp/actions/jobs/{}",a.run.job_id),false)?)?;
    if job["id"]!=a.run.job_id||job["run_id"]!=a.run.run_id||job["head_sha"]!=a.run.source_head||job["run_attempt"]!=1{return Err("authenticated cancellation original job identity".into());}
    let steps=job["steps"].as_array().ok_or("authenticated original workflow steps unavailable")?;
    let matches=steps.iter().filter(|s|s["number"]==step.number&&s["name"]==step.name).collect::<Vec<_>>();
    if matches.len()!=1||matches[0]["status"]!="completed"||!matches[0]["conclusion"].as_str().is_some_and(|s|matches!(s,"success"|"failure"|"cancelled")){return Err("actual runner-enforced cancellation step outcome unavailable".into());}
    // Authenticated original job logs are required, not exit130, an arbitrary marker or a peer claim.
    // GitHub may withhold logs until job completion. Preserve UNKNOWN; never run official on that absence.
    let logs=gh(e,a,token,format!("repos/anilbeesetti/ktlsp/actions/jobs/{}/logs",a.run.job_id),true)?;
    let text=std::str::from_utf8(&logs).map_err(|_|"runner original job logs encoding")?;
    // actions/runner StepsRunner.RunStepAsync emits this exact named-step message only
    // for the step cancellation token expiring while the whole-job token is not cancelled.
    let marker=format!("##[error]The action '{}' has timed out after 1 minutes.",step.name);
    if !text.lines().any(|line|line.contains(&marker)) {
        return Err("runner-generated original step timeout/cancel cause cannot be authenticated; original export retained UNKNOWN".into());
    }
    // Retain exact API/log bytes in capped collector evidence; receiver re-retrieves and joins independently.
    e.append(&json!({"kind":"authenticated-workflow-step-cancellation","runId":a.run.run_id,"jobId":a.run.job_id,"attempt":1,"stepNumber":step.number,"stepName":step.name,"jobSha256":sha_bytes(&serde_json::to_vec(&job).map_err(|e|e.to_string())?),"logsSha256":sha_bytes(&logs),"wholeJobCancellationIsNotCalibration":true}),false)
}
fn collect_calibration()->Result<Value> {
    let expected:RootGrant=load(Path::new(EXPECTED_GRANT))?;expected.validate_shape()?;
    let locations:CalibrationCollection=load(&Path::new(STAGING).join("calibration-cases.json"))?;
    if locations.cases.len()!=CalibrationCase::ALL.len(){return Err("all ten actual calibration locations required".into());}
    let boot=String::from_utf8(read_file(Path::new("/proc/sys/kernel/random/boot_id"),128)?).map_err(|_|"boot encoding")?.trim().to_owned();
    let mut cases=BTreeSet::new();let mut nonces=BTreeSet::new();let mut receipts=Vec::new();let mut daemon_id=None;
    let mut evidence=Evidence::preflight();evidence.limit=64000;
    let mut cancellation_output=None;
    for location in locations.cases {
        if !cases.insert(location.case)||!nonces.insert(location.nonce.clone())||!digest(&location.nonce){return Err("calibration locations duplicated/nonfinite".into());}
        let dir=directory(&Path::new(ROOT).join(&location.nonce).to_string_lossy())?;
        let a:Admission=load(&dir.join("admission.json"))?;
        let grant:RootGrant=load(&dir.join("root-authorization.json"))?;grant.validate_shape()?;
        let originally_expected:RootGrant=load(&dir.join("expected-grant.json"))?;grant.validate(&originally_expected)?;
        if grant.scope!=GrantScope::RustCalibration||grant.calibration_case!=Some(location.case)||a.scope!="PR174_RUST_CALIBRATION_ADMITTED"||a.boot_id!=boot||grant.source_head!=expected.source_head||grant.source_tree!=expected.source_tree||grant.workflow_sha256!=expected.workflow_sha256||grant.host_binary_sha256!=expected.host_binary_sha256||grant.peer_sha256!=expected.peer_sha256||grant.containment_template_sha256!=expected.containment_template_sha256||grant.image!=expected.image||grant.image_id!=expected.image_id||grant.host!=expected.host||a.run.run_id!=expected.run_id||a.run.job_id!=expected.job_id||a.run.suite_id!=expected.suite_id{return Err("actual calibration must be same original source/run/job/host/template/image".into());}
        if daemon_id.as_ref().is_some_and(|id|id!=&a.daemon_id){return Err("calibration daemon changed between cases".into());}daemon_id=Some(a.daemon_id.clone());
        let b=binding(&dir)?;let teardown:Value=load(&dir.join("teardown-receipt.json"))?;
        if teardown["teardown"]!="CONTAINER_TEARDOWN_CONFIRMED"||teardown["containerId"]!=b.container_id||teardown["nonce"]!=a.nonce{return Err("actual exact-CID calibration closure incomplete".into());}
        let observation=read_host_observation(&dir,location.case,&b.container_id)?;
        if observation["normalExit"]!=!location.case.forced(){return Err("calibration lifecycle outcome mismatch".into());}
        let peer:PeerReport=load(&dir.join("export/capture/calibration-peer.json"))?;peer_shape(&peer,location.case)?;validate_peer_case(&peer)?;
        if !peer.observer_gate_observed || peer.outcome!=(if location.case.forced(){PeerOutcome::AwaitingHostAction}else{PeerOutcome::Completed}){return Err("calibration original peer mechanics incomplete".into());}
        if location.case==CalibrationCase::WorkflowStepCancel {
            cancellation_output=Some(dir.clone());
            let result=authenticate_step_cancellation(&dir,&a,&grant,&mut evidence);
            let bytes=serde_json::to_vec(&json!({"scope":"PR174_CALIBRATION_CANCELLATION_AUTHENTICATION","error":result.as_ref().err(),"evidenceHex":raw_hex(&evidence.pending),"officialCaptureSuccess":false})).map_err(|e|e.to_string())?;
            make_file(&dir.join("cancellation-authentication.json"),&bytes,0o400)?;
            result?;
        }
        let export=dir.join("export/export-binding.json");protected(&export,false)?;
        receipts.push(ktlsp_oracle_replay::authority::CalibrationCaseReceipt{case:location.case,run_id:a.run.run_id,job_id:a.run.job_id,suite_id:a.run.suite_id,attempt:a.run.attempt,nonce:a.nonce,container_id:b.container_id,observation_sha256:sha_path(&dir.join("receipts/observer.evidence"),a.preparation_deadline_ms)?,teardown_sha256:sha_path(&dir.join("teardown-receipt.json"),a.preparation_deadline_ms)?,export_sha256:sha_path(&export,a.preparation_deadline_ms)?,normal_exit:!location.case.forced(),closure_confirmed:true,denials_confirmed:location.case==CalibrationCase::BoundaryDenials,workflow_cancellation_authenticated:location.case==CalibrationCase::WorkflowStepCancel});
    }
    let report=CalibrationReport{schema_version:1,scope:GrantScope::RustCalibration,source_head:expected.source_head.clone(),source_tree:expected.source_tree.clone(),workflow_sha256:expected.workflow_sha256.clone(),host_binary_sha256:expected.host_binary_sha256.clone(),peer_sha256:expected.peer_sha256.clone(),containment_template_sha256:expected.containment_template_sha256.clone(),image:expected.image.clone(),image_id:expected.image_id.clone(),host:measured_host()?,boot_id:boot,daemon_id:daemon_id.ok_or("calibration daemon absent")?,cases:receipts};
    report.validate(&expected)?;
    let bytes=serde_json::to_vec(&report).map_err(|e|e.to_string())?;
    let hash=sha_bytes(&bytes);
    let target=Path::new(STAGING).join("collected-calibration.json");make_file(&target,&bytes,0o400)?;
    Ok(json!({"status":"ACTUAL_CALIBRATION_COLLECTED","path":target,"sha256":hash,"cancellationEvidenceDirectory":cancellation_output,"officialCaptureSuccess":false,"officialStart":"separate independently ROOT admitted grant and bootstrap required"}))
}

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
fn collect_calibration()->Result<Value> {
    let expected:RootGrant=load(Path::new(EXPECTED_GRANT))?;expected.validate_shape()?;
    // Collection authenticates retained evidence only; it extends no original START/lifecycle permission.
    let collection_end=mono_ms()?.checked_add(60000).ok_or("calibration collection deadline overflow")?;
    let locations:CalibrationCollection=load(&Path::new(STAGING).join("calibration-cases.json"))?;
    if locations.cases.len()!=CalibrationCase::ALL.len(){return Err("all ten actual calibration locations required".into());}
    let boot=String::from_utf8(read_file(Path::new("/proc/sys/kernel/random/boot_id"),128)?).map_err(|_|"boot encoding")?.trim().to_owned();
    let mut cases=BTreeSet::new();let mut nonces=BTreeSet::new();let mut receipts=Vec::new();let mut daemon_id=None;
    let mut evidence=Evidence::preflight();evidence.limit=64000;
    let mut cancellation_output=None;
    for location in locations.cases {
        if !cases.insert(location.case)||!nonces.insert(location.nonce.clone())||!digest(&location.nonce){return Err("calibration locations duplicated/nonfinite".into());}
        let dir=directory(&Path::new(ROOT).join(&location.nonce).to_string_lossy())?;
        let mut a:Admission=load(&dir.join("admission.json"))?;
        a.preparation_deadline_ms=collection_end;
        let grant:RootGrant=load(&dir.join("root-authorization.json"))?;grant.validate_shape()?;
        let originally_expected:RootGrant=load(&dir.join("expected-grant.json"))?;grant.validate(&originally_expected)?;
        if grant.scope!=GrantScope::RustCalibration||grant.calibration_case!=Some(location.case)||a.scope!="PR174_RUST_CALIBRATION_ADMITTED"||a.boot_id!=boot||grant.source_head!=expected.source_head||grant.source_tree!=expected.source_tree||grant.workflow_sha256!=expected.workflow_sha256||grant.host_binary_sha256!=expected.host_binary_sha256||grant.peer_sha256!=expected.peer_sha256||grant.containment_template_sha256!=expected.containment_template_sha256||grant.image!=expected.image||grant.image_id!=expected.image_id||grant.host!=expected.host||a.run.run_id!=expected.run_id||a.run.job_id!=expected.job_id||a.run.suite_id!=expected.suite_id{return Err("actual calibration must be same original source/run/job/host/template/image".into());}
        if daemon_id.as_ref().is_some_and(|id|id!=&a.daemon_id){return Err("calibration daemon changed between cases".into());}daemon_id=Some(a.daemon_id.clone());
        let b=binding(&dir)?;let teardown:Value=load(&dir.join("teardown-receipt.json"))?;
        if teardown["teardown"]!="CONTAINER_TEARDOWN_CONFIRMED"||teardown["containerId"]!=b.container_id||teardown["nonce"]!=a.nonce{return Err("actual exact-CID calibration closure incomplete".into());}
        require_actual_calibration_action(&dir,&a,&b,location.case)?;
        let observation=read_host_observation(&dir,location.case,&b.container_id)?;
        if observation["normalExit"]!=!location.case.forced(){return Err("calibration lifecycle outcome mismatch".into());}
        let peer:PeerReport=load(&dir.join("export/capture/calibration-peer.json"))?;peer_shape(&peer,location.case)?;validate_peer_case(&peer)?;
        if !peer.observer_gate_observed || peer.outcome!=(if location.case.forced(){PeerOutcome::AwaitingHostAction}else{PeerOutcome::Completed}){return Err("calibration original peer mechanics incomplete".into());}
        if location.case==CalibrationCase::WorkflowStepCancel {
            cancellation_output=Some(dir.clone());
            let result=workflow_authenticate_step_timeout(&dir,&a,&grant,&mut evidence);
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

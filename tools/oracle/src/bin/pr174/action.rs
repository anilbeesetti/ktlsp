fn command_receipt_ok(value:&Value)->bool {
    value["exitCode"]==0&&value["signal"].is_null()&&value["EOF"]==json!([true,true])&&value["timeout"]==false&&value["truncated"]==false&&value["reaped"]==true
}
fn unhex(value:&Value)->Result<Vec<u8>> {
    let text=value.as_str().ok_or("raw command hex missing")?;
    if text.len()>CLEANUP_BYTES*2||text.len()%2!=0||!text.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)){return Err("raw command hex invalid/bounded".into());}
    let nibble=|b:u8|if b.is_ascii_digit(){b-b'0'}else{b-b'a'+10};
    Ok(text.as_bytes().chunks_exact(2).map(|pair|(nibble(pair[0])<<4)|nibble(pair[1])).collect())
}
fn forced_cleanup_proof(raw:&[u8],tool:&Path,cid:&str)->Result<()> {
    let mut killed=false;let mut waited=false;let mut inspected=false;
    for line in raw.split(|b|*b==b'\n').filter(|line|!line.is_empty()) {
        let value:Value=parse(line)?;
        if value["kind"]!="command"||value["program"]!=json!(tool)||!command_receipt_ok(&value["result"]){continue;}
        let result=&value["result"];
        if value["args"]==json!(["--host=unix:///run/docker.sock","kill","--signal=KILL",cid]) {
            if killed||std::str::from_utf8(&unhex(&result["stdoutHex"])?).map_err(|_|"kill output encoding")?.trim()!=cid{return Err("actual exact-CID kill receipt duplicated/mismatched".into());}killed=true;
        }else if value["args"]==json!(["--host=unix:///run/docker.sock","wait",cid]) {
            if !killed||waited||std::str::from_utf8(&unhex(&result["stdoutHex"])?).map_err(|_|"wait output encoding")?.trim()!="137"{return Err("forced case missing actual ordered SIGKILL wait137".into());}waited=true;
        }else if waited&&value["args"]==json!(["--host=unix:///run/docker.sock","container","inspect",cid]) {
            let inspect=one_inspect(&unhex(&result["stdoutHex"])?)?;
            if !stopped(&inspect,cid)||inspect["State"]["ExitCode"]!=137||inspect["State"]["OOMKilled"]!=false||inspect["RestartCount"]!=0{return Err("forced calibration stopped-state/OOM/restart mismatch".into());}inspected=true;
        }
    }
    if !killed||!waited||!inspected{return Err("forced calibration requires actual kill/wait137/stopped-state sequence, not inferred lifecycle".into());}Ok(())
}
fn require_actual_calibration_action(dir:&Path,a:&Admission,b:&Binding,case:CalibrationCase)->Result<()> {
    if !case.forced(){return Ok(());}
    let action:ktlsp_oracle_replay::authority::CalibrationAction=load(&dir.join("calibration-action.json"))?;action.validate()?;
    let identity:ProcessIdentity=load(&dir.join("process-identity.json"))?;
    if action.case!=case||action.nonce!=a.nonce||action.container_id!=b.container_id||action.boot_id!=a.boot_id||action.daemon_id!=a.daemon_id||Some(action.armed_ms)!=b.armed_ms||action.init_pid!=identity.init_pid||action.init_starttime!=identity.init_start{return Err("actual forced calibration action not this original binding".into());}
    forced_cleanup_proof(&read_file(&dir.join("receipts/cleanup.evidence"),CLEANUP_BYTES)?,&a.tools.docker,&b.container_id)
}
fn await_calibration(dir:&Path)->Result<Value> {
    let a=admission(dir)?;root_authority(dir,&a)?;
    if calibration_case(dir,&a)?.is_none(){return Err("await route is finite Rust-calibration-only".into());}
    let end=mono_ms()?.checked_add(600000).ok_or("calibration await overflow")?.min(a.preparation_deadline_ms);
    loop {
        if dir.join("export/export-binding.json").try_exists().map_err(|e|e.to_string())? {
            let path=dir.join("export/export-binding.json");
            if fs::symlink_metadata(&path).map_err(|e|e.to_string())?.nlink()!=1{return Err("calibration export readiness single-link custody".into());}
            let binding:ktlsp_oracle_replay::authority::ExportBinding=load(&path)?;
            if binding.nonce!=a.nonce{return Err("calibration await export nonce mismatch".into());}
            return Ok(json!({"status":"CALIBRATION_EXPORT_AVAILABLE","directory":dir.join("export"),"officialCaptureSuccess":false}));
        }
        if mono_ms()?>=end{return Err("original calibration export not available within finite outer budget".into());}
        std::thread::yield_now();
    }
}

#[cfg(test)]
mod action_source_tests {
    use super::*;
    #[test]
    fn synthetic_forced_cleanup_cannot_infer_a_kill_from_absence() {
        assert!(forced_cleanup_proof(b"",Path::new("/usr/bin/docker"),&"1".repeat(64)).is_err());
        assert!(unhex(&json!("0g")).is_err());
        assert!(unhex(&json!("303137\n")).is_err());
        assert_eq!(unhex(&json!("3133370a")).unwrap(),b"137\n");
    }
    #[test]
    fn synthetic_forced_receipts_require_real_action_sequence_and_no_oom() {
        let cid="1".repeat(64);let tool=Path::new("/usr/bin/docker");
        let record=|argv:Value,stdout:Vec<u8>| {
            let output=Output{stdout,code:Some(0),eof:[true,true],reaped:true,..Output::default()};
            json!({"kind":"command","program":tool,"args":argv,"result":output.json()})
        };
        let killed=record(json!(["--host=unix:///run/docker.sock","kill","--signal=KILL",cid]),format!("{cid}\n").into_bytes());
        let waited=record(json!(["--host=unix:///run/docker.sock","wait",cid]),b"137\n".to_vec());
        let state=|oom|json!([{"Id":cid,"RestartCount":0,"State":{"Running":false,"Restarting":false,"Dead":false,"Pid":0,"ExitCode":137,"OOMKilled":oom,"StartedAt":"synthetic-start","FinishedAt":"synthetic-end"}}]);
        let inspected=|oom|record(json!(["--host=unix:///run/docker.sock","container","inspect",cid]),serde_json::to_vec(&state(oom)).unwrap());
        let join=|rows:&[Value]|{let mut bytes=Vec::new();for row in rows {serde_json::to_writer(&mut bytes,row).unwrap();bytes.push(b'\n');}bytes};
        assert!(forced_cleanup_proof(&join(&[killed.clone(),waited.clone(),inspected(false)]),tool,&cid).is_ok());
        assert!(forced_cleanup_proof(&join(&[waited.clone(),inspected(false)]),tool,&cid).is_err());
        assert!(forced_cleanup_proof(&join(&[killed,waited,inspected(true)]),tool,&cid).is_err());
        let action=|case,observed_ms|ktlsp_oracle_replay::authority::CalibrationAction{schema_version:1,scope:"PR174_RUST_CALIBRATION_ACTION".into(),case,nonce:"2".repeat(64),container_id:cid.clone(),boot_id:"synthetic-boot".into(),daemon_id:"synthetic-daemon".into(),init_pid:10,init_starttime:20,armed_ms:1000,deadline_ms:301000,observed_ms,observed_unix_ms:1,original_running:true};
        assert!(action(CalibrationCase::Timeout,300999).validate().is_err());
        assert!(action(CalibrationCase::Timeout,301000).validate().is_ok());
        assert!(action(CalibrationCase::Kill,1001).validate().is_ok());
        assert!(action(CalibrationCase::Kill,301000).validate().is_err());
    }
}

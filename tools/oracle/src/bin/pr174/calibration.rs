use ktlsp_oracle_replay::calibration_peer::{Report as PeerReport, Outcome as PeerOutcome};
use std::os::fd::{AsRawFd, FromRawFd};

fn seal_calibration(dir:&Path,a:&Admission,cid:&str,e:&mut Evidence)->Result<()> {
    let case=calibration_case(dir,a)?.ok_or("Rust calibration plan absent")?;
    side(e,&dir.join("preflight/identity.json"),&serde_json::to_vec(&json!({"schemaVersion":1,"scope":"PR174_RUST_CALIBRATION","case":case,"run":a.run,"containerId":cid,"nonce":a.nonce,"bootId":a.boot_id,"daemonId":a.daemon_id,"containmentTemplateSha256":containment_template_sha256()})).map_err(|e|e.to_string())?,0o444,false)?;
    fs::set_permissions(dir.join("preflight"),fs::Permissions::from_mode(0o555)).map_err(|e|e.to_string())
}
fn capture_file(dir:&Path,name:&str)->Result<File> {
    if name.is_empty() || !name.bytes().all(|b|b.is_ascii_alphanumeric()||b"._-".contains(&b)){return Err("fixed capture filename".into());}
    unsafe extern "C" {fn openat(fd:i32,name:*const std::ffi::c_char,flags:i32,...)->i32;}
    let mut parent=File::open(dir).map_err(|e|e.to_string())?;
    // O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC for every child-controlled directory, never follow a path component.
    for component in ["state","capture",name] {
        let component=std::ffi::CString::new(component).map_err(|_|"capture component")?;
        let flags=0x20000|0x80000|0x800|if component.as_bytes()==name.as_bytes(){0}else{0x10000};
        let fd=unsafe{openat(parent.as_raw_fd(),component.as_ptr(),flags)};
        if fd<0{return Err(std::io::Error::last_os_error().to_string());}
        parent=unsafe{File::from_raw_fd(fd)};
    }
    let metadata=parent.metadata().map_err(|e|e.to_string())?;
    if !metadata.is_file() || metadata.nlink()!=1{return Err("capture regular single-link file required".into());}
    Ok(parent)
}
fn read_peer(dir:&Path,name:&str)->Result<PeerReport> {
    let mut file=capture_file(dir,name)?;let mut bytes=Vec::new();Read::by_ref(&mut file).take(INPUT_BYTES as u64+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
    if bytes.len()>INPUT_BYTES{return Err("finite peer report bound".into());}parse(&bytes)
}
fn peer_shape(report:&PeerReport,case:CalibrationCase)->Result<()> {
    if report.schema_version!=1 || report.scope!="PR174_RUST_CALIBRATION_PEER" || report.case!=case || report.first_failure.is_some() || report.children.len()>1 || report.probes.len()>64 || report.elapsed_ms>300000 {return Err("finite peer observation identity/failure/bounds".into());}Ok(())
}
fn attest_peer_process(dir:&Path,identity:&ProcessIdentity,reported:&ktlsp_oracle_replay::calibration_peer::ProcessIdentity,case:CalibrationCase,child:bool,end:u64,e:&mut Evidence)->Result<()> {
    let sealed=fs::metadata(dir.join("inputs/tool/ktlsp-oracle-calibration-peer")).map_err(|e|e.to_string())?;
    let mut matched=0;
    for pid in members(&identity.cgroup,end,e.available()/4)?.pids {
        let p=Path::new("/proc").join(pid.to_string());
        let before=read_file(&p.join("stat"),4096)?;
        let status=read_file(&p.join("status"),INPUT_BYTES)?;
        let text=std::str::from_utf8(&status).map_err(|_|"peer status UTF8")?;
        let nspid=text.lines().find(|l|l.starts_with("NSpid:")).ok_or("peer NSpid unavailable")?.split_whitespace().skip(1).map(str::parse::<u32>).collect::<std::result::Result<Vec<_>,_>>().map_err(|_|"peer NSpid malformed")?;
        if nspid.last()!=Some(&reported.pid){continue;}
        let stamp=process_start(&before)?;let exe=exe_stamp(pid)?;
        let cmdline=read_file(&p.join("cmdline"),INPUT_BYTES)?;
        let expected=args(&[&format!("{INNER}/tool/ktlsp-oracle-calibration-peer"),if child{"--descendant"}else{"--case"},case.as_str()]);
        let limits=read_file(&p.join("limits"),INPUT_BYTES)?;
        let ns=fs::read_link(p.join("ns/pid")).map_err(|e|e.to_string())?;
        let cg=read_file(&p.join("cgroup"),INPUT_BYTES)?;
        if nspid.len()!=2 || nspid[0]!=pid || stamp.1!=reported.start_time_ticks || exe!=(sealed.dev(),sealed.ino()) || exe!=(reported.executable_device,reported.executable_inode) || !actual_argv(&cmdline,&expected) || ns.to_string_lossy()!=identity.pid_namespace || std::str::from_utf8(&cg).map_err(|_|"peer cgroup UTF8")?.trim()!=format!("0::{}",identity.cgroup) || !core_zero(text,std::str::from_utf8(&limits).map_err(|_|"peer limits UTF8")?) || process_start(&read_file(&p.join("stat"),4096)?)?!=stamp || exe_stamp(pid)?!=exe{return Err("original real Rust peer proc identity/security mismatch".into());}
        e.append(&json!({"kind":"calibration-original-process","case":case,"child":child,"hostPid":pid,"starttime":stamp.1,"statHex":raw_hex(&before),"statusHex":raw_hex(&status),"limitsHex":raw_hex(&limits),"cmdlineHex":raw_hex(&cmdline),"cgroupHex":raw_hex(&cg),"namespace":ns,"exeDevice":exe.0,"exeInode":exe.1,"sealedExecutableDevice":sealed.dev(),"sealedExecutableInode":sealed.ino()}),false)?;
        matched+=1;
    }
    if matched!=1{return Err("original peer identity missing/ambiguous".into());}Ok(())
}
fn observe_calibration(dir:&Path,a:&Admission,b:&Binding,e:&mut Evidence,case:CalibrationCase)->Result<()> {
    let started=mono_ms()?;let phase_end=started.checked_add(60000).ok_or("peer phase overflow")?;
    let session_end=b.armed_ms.ok_or("peer watchdog unarmed")?.checked_add(300000).ok_or("peer session overflow")?;
    let v=inspect(e,&a.tools.docker,&b.container_id,phase_end)?;config(&v,dir,a,&b.container_id,false)?;
    let pid=v["State"]["Pid"].as_u64().and_then(|p|u32::try_from(p).ok()).filter(|p|*p>1).ok_or("original peer init unavailable")?;
    let identity=original_init(pid)?;
    if identity.cgroup!=format!("/system.slice/docker-{}.scope",b.container_id){return Err("Rust peer original cgroup not exact CID".into());}
    side_json(e,&dir.join("process-identity.json"),&identity)?;attest_mounts(dir,Some(pid),e)?;actual_resources(&identity,e)?;
    let ready=loop {
        if mono_ms()?>=phase_end{return Err("real peer readiness observation missed".into());}
        match fs::symlink_metadata(dir.join("state/capture/calibration-ready.json")) {
            Ok(_)=>break read_peer(dir,"calibration-ready.json")?,
            Err(error) if error.kind()==ErrorKind::NotFound=>std::thread::yield_now(),
            Err(error)=>return Err(error.to_string()),
        }
    };
    peer_shape(&ready,case)?;
    if ready.outcome!=PeerOutcome::AwaitingObserver || ready.observer_gate_observed{return Err("peer readiness not original pre-gate observation".into());}
    let descendants=matches!(case,CalibrationCase::SameGroup|CalibrationCase::Setsid|CalibrationCase::Setpgid);
    if ready.children.len()!=usize::from(descendants){return Err("fixed descendant calibration inventory mismatch".into());}
    attest_peer_process(dir,&identity,&ready.original_process,case,false,phase_end,e)?;
    for child in &ready.children {attest_peer_process(dir,&identity,child,case,true,phase_end,e)?;}
    side(e,&dir.join("state/calibration-observer-ready"),b"observed\n",0o444,false)?;
    let mut final_report=None;
    loop {
        if mono_ms()?>=session_end {break;}
        if dir.join("cleanup-requested").try_exists().map_err(|e|e.to_string())? {return Err("calibration cleanup/cancellation latch; closure remains independent".into());}
        if final_report.is_none() && dir.join("state/capture/calibration-peer.json").try_exists().map_err(|e|e.to_string())? {
            let report=read_peer(dir,"calibration-peer.json")?;peer_shape(&report,case)?;
            if !report.observer_gate_observed || report.original_process!=ready.original_process || report.children!=ready.children{return Err("peer changed original identities/gate".into());}
            match (case.forced(),report.outcome) {(true,PeerOutcome::AwaitingHostAction)|(false,PeerOutcome::Completed)=>{},_=>return Err("peer outcome differs from finite calibration mode".into())}
            validate_peer_case(&report)?;
            // Retain self-report bytes as data, never relabel as independently measured host denial facts.
            e.append(&json!({"kind":"calibration-peer-observation","report":report,"trust":"sealed finite peer self-report; original proc/mount/resource/closure separately host measured"}),false)?;
            final_report=Some(report);
            if case==CalibrationCase::Kill {break;}
        }
        let stat_path=Path::new("/proc").join(pid.to_string()).join("stat");
        match fs::symlink_metadata(&stat_path) {Err(error)if error.kind()==ErrorKind::NotFound=>break,Err(error)=>return Err(error.to_string()),Ok(_)=>{if process_start(&read_file(&stat_path,4096)?)?!=(pid,identity.init_start){return Err("original calibration init reused".into());}}}
        std::thread::yield_now();
    }
    let report=final_report.ok_or("finite peer final observation absent")?;
    let forced=case.forced();
    if !forced {
        let state=inspect(e,&a.tools.docker,&b.container_id,session_end)?;
        if !stopped(&state,&b.container_id) || state["State"]["ExitCode"]!=0 || state["State"]["OOMKilled"]!=false || state["RestartCount"]!=0{return Err("calibration peer did not complete normally".into());}
    }
    e.append(&json!({"kind":"calibration-host-observation","scope":"PR174_RUST_CALIBRATION","case":case,"containerId":b.container_id,"nonce":b.nonce,"run":a.run,"bootId":a.boot_id,"daemonId":a.daemon_id,"containmentTemplateSha256":containment_template_sha256(),"peerReportSha256":sha_bytes(&serde_json::to_vec(&report).map_err(|e|e.to_string())?),"normalExit":!forced,"hostAction":if case==CalibrationCase::Kill{"exact-CID cleanup kill requested"}else if case==CalibrationCase::Timeout{"original300000ms deadline"}else if case==CalibrationCase::WorkflowStepCancel{"requires authenticated original job cancellation; watchdog/cleanup independently exports"}else{"normal original exit"},"workflowCancellationAuthenticated":false,"officialCaptureSuccess":false}),false)
}
fn validate_peer_case(report:&PeerReport)->Result<()> {
    report.validate_case(report.case)
}

#[cfg(test)]
mod calibration_source_tests {
    use super::*;
    #[test]
    fn synthetic_scope_template_is_deterministic_not_runtime_calibration() {
        assert!(digest(&containment_template_sha256()));
        assert_eq!(containment_template_sha256(),containment_template_sha256());
        assert_eq!(CalibrationCase::ALL.iter().filter(|case|case.forced()).count(),3);
        assert!(sha_bytes(b"one")!=sha_bytes(b"two"));
    }
}

use ktlsp_oracle_replay::calibration_peer::{Report as PeerReport, Outcome as PeerOutcome};
use std::os::fd::{AsRawFd, FromRawFd};

fn seal_calibration(dir:&Path,a:&Admission,cid:&str,e:&mut Evidence)->Result<()> {
    let case=calibration_case(dir,a)?.ok_or("Rust calibration plan absent")?;
    side(e,&dir.join("preflight/identity.json"),&serde_json::to_vec(&json!({"schemaVersion":1,"scope":"PR174_RUST_CALIBRATION","case":case,"run":a.run,"containerId":cid,"nonce":a.nonce,"bootId":a.boot_id,"daemonId":a.daemon_id,"containmentTemplateSha256":containment_template_sha256()?})).map_err(|e|e.to_string())?,0o444,false)?;
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
#[derive(Clone,Copy)]
struct AttestedPeer {pid:u32,parent:u32,group:u32,session:u32}
fn namespace_pair(status:&str,key:&str)->Result<(u32,u32)> {
    let mut values=status.lines().find(|line|line.starts_with(key)).ok_or("original peer namespace group/session unavailable")?.split_whitespace().skip(1);
    let first=values.next().ok_or("host namespace identity missing")?.parse().map_err(|_|"host namespace identity malformed")?;
    let second=values.next().ok_or("private namespace identity missing")?.parse().map_err(|_|"private namespace identity malformed")?;
    if values.next().is_some()||first==0||second==0{return Err("ambiguous namespace group/session identity".into());}
    Ok((first,second))
}
fn attest_peer_process(dir:&Path,identity:&ProcessIdentity,reported:&ktlsp_oracle_replay::calibration_peer::ProcessIdentity,case:CalibrationCase,child:bool,end:u64,e:&mut Evidence)->Result<AttestedPeer> {
    let sealed=fs::metadata(dir.join("inputs/tool/ktlsp-oracle-calibration-peer")).map_err(|e|e.to_string())?;
    let mut matched=None;
    for pid in members(&identity.cgroup,end,e.available()/4)?.pids {
        let p=Path::new("/proc").join(pid.to_string());
        let before=read_file(&p.join("stat"),4096)?;
        let status=read_file(&p.join("status"),INPUT_BYTES)?;
        let text=std::str::from_utf8(&status).map_err(|_|"peer status UTF8")?;
        let nspid=text.lines().find(|l|l.starts_with("NSpid:")).ok_or("peer NSpid unavailable")?.split_whitespace().skip(1).map(str::parse::<u32>).collect::<std::result::Result<Vec<_>,_>>().map_err(|_|"peer NSpid malformed")?;
        if nspid.last()!=Some(&reported.pid){continue;}
        let stamp=process_start(&before)?;let exe=exe_stamp(pid)?;
        let group=namespace_pair(text,"NSpgid:")?;
        let session=namespace_pair(text,"NSsid:")?;
        let parent=parent_pid(&before)?;
        if group.1!=reported.process_group||session.1!=reported.session||(!child&&parent!=identity.init_pid){return Err("actual peer parent/group/session differs from self-report".into());}
        let cmdline=read_file(&p.join("cmdline"),INPUT_BYTES)?;
        let expected=args(&[&format!("{INNER}/tool/ktlsp-oracle-calibration-peer"),if child{"--descendant"}else{"--case"},case.as_str()]);
        let limits=read_file(&p.join("limits"),INPUT_BYTES)?;
        let ns=fs::read_link(p.join("ns/pid")).map_err(|e|e.to_string())?;
        let cg=read_file(&p.join("cgroup"),INPUT_BYTES)?;
        let proc_custody=fs::metadata(&p).map_err(|e|e.to_string())?;
        if case==CalibrationCase::BoundaryDenials&&!child&&(proc_custody.uid()!=0||proc_custody.gid()!=0){return Err("boundary parent lacks measured nondumpable recorder proc custody".into());}
        if nspid.len()!=2 || nspid[0]!=pid || stamp.1!=reported.start_time_ticks || exe!=(sealed.dev(),sealed.ino()) || exe!=(reported.executable_device,reported.executable_inode) || !actual_argv(&cmdline,&expected) || ns.to_string_lossy()!=identity.pid_namespace || std::str::from_utf8(&cg).map_err(|_|"peer cgroup UTF8")?.trim()!=format!("0::{}",identity.cgroup) || !core_zero(text,std::str::from_utf8(&limits).map_err(|_|"peer limits UTF8")?) || process_start(&read_file(&p.join("stat"),4096)?)?!=stamp || exe_stamp(pid)?!=exe{return Err("original real Rust peer proc identity/security mismatch".into());}
        e.append(&json!({"kind":"calibration-original-process","case":case,"child":child,"hostPid":pid,"starttime":stamp.1,"statHex":raw_hex(&before),"statusHex":raw_hex(&status),"limitsHex":raw_hex(&limits),"cmdlineHex":raw_hex(&cmdline),"cgroupHex":raw_hex(&cg),"namespace":ns,"exeDevice":exe.0,"exeInode":exe.1,"procDirectoryUid":proc_custody.uid(),"procDirectoryGid":proc_custody.gid(),"sealedExecutableDevice":sealed.dev(),"sealedExecutableInode":sealed.ino()}),false)?;
        if matched.is_some(){return Err("original peer process identity ambiguous".into());}
        matched=Some(AttestedPeer{pid,parent,group:group.0,session:session.0});
    }
    matched.ok_or("original peer identity missing".into())
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
    let parent=attest_peer_process(dir,&identity,&ready.original_process,case,false,phase_end,e)?;
    for child in &ready.children {
        let actual=attest_peer_process(dir,&identity,child,case,true,phase_end,e)?;
        let relationship=actual.parent==parent.pid&&match case {
            CalibrationCase::SameGroup=>actual.group==parent.group&&actual.session==parent.session,
            CalibrationCase::Setsid=>actual.group==actual.pid&&actual.session==actual.pid&&actual.session!=parent.session,
            CalibrationCase::Setpgid=>actual.group==actual.pid&&actual.group!=parent.group&&actual.session==parent.session,
            _=>false,
        };
        if !relationship{return Err("original host /proc did not prove fixed descendant group/session case".into());}
    }
    side(e,&dir.join("state/calibration-observer-ready"),b"observed\n",0o444,false)?;
    let mut final_report=None;
    loop {
        if mono_ms()?>=session_end {break;}
        if dir.join("cleanup-requested").try_exists().map_err(|e|e.to_string())? {return Err("calibration cleanup/cancellation latch; closure remains independent".into());}
        if final_report.is_none() && dir.join("state/capture/calibration-peer.json").try_exists().map_err(|e|e.to_string())? {
            final_report=Some(consume_final_peer(dir,case,&ready,e)?);
            if case==CalibrationCase::Kill {break;}
        }
        let stat_path=Path::new("/proc").join(pid.to_string()).join("stat");
        let observed=match read_file(&stat_path,4096) {
            Ok(raw)=>Some(raw),
            Err(error)=>match fs::symlink_metadata(&stat_path) {
                Err(missing)if missing.kind()==ErrorKind::NotFound=>None,
                _=>return Err(error),
            },
        };
        match observed {
            None if case.forced()=>return Err("original forced-case init died before intended host action; calibration failed, closure independent".into()),
            None=>break,
            Some(raw)if process_start(&raw)?!=(pid,identity.init_start)=>return Err("original calibration init reused".into()),
            Some(_)=>{},
        }
        std::thread::yield_now();
    }
    // Publication can occur between the loop's existence check and normal init exit.
    let report=match final_report {Some(report)=>report,None=>consume_final_peer(dir,case,&ready,e)?};
    let forced=case.forced();
    if !forced {
        let state=inspect(e,&a.tools.docker,&b.container_id,session_end)?;
        if !stopped(&state,&b.container_id) || state["State"]["ExitCode"]!=0 || state["State"]["OOMKilled"]!=false || state["RestartCount"]!=0{return Err("calibration peer did not complete normally".into());}
    } else {
        record_calibration_action(dir,a,b,&identity,case,session_end,e)?;
    }
    e.append(&json!({"kind":"calibration-host-observation","scope":"PR174_RUST_CALIBRATION","case":case,"containerId":b.container_id,"nonce":b.nonce,"run":a.run,"bootId":a.boot_id,"daemonId":a.daemon_id,"containmentTemplateSha256":containment_template_sha256()?,"peerReportSha256":sha_bytes(&serde_json::to_vec(&report).map_err(|e|e.to_string())?),"normalExit":!forced,"hostAction":if case==CalibrationCase::Kill{"exact-CID cleanup kill requested"}else if case==CalibrationCase::Timeout{"original300000ms deadline"}else if case==CalibrationCase::WorkflowStepCancel{"requires authenticated original job cancellation; watchdog/cleanup independently exports"}else{"normal original exit"},"workflowCancellationAuthenticated":false,"officialCaptureSuccess":false}),false)
}
fn validate_peer_case(report:&PeerReport)->Result<()> {
    report.validate_case(report.case)
}

fn consume_final_peer(dir:&Path,case:CalibrationCase,ready:&PeerReport,e:&mut Evidence)->Result<PeerReport> {
    let report=read_peer(dir,"calibration-peer.json")?;peer_shape(&report,case)?;
    if report.original_process!=ready.original_process||report.children!=ready.children{return Err("peer changed original identities".into());}
    report.validate_case(case)?;
    e.append(&json!({"kind":"calibration-peer-observation","report":report,"trust":"sealed finite peer self-report; original proc/mount/resource/action/closure separately host measured"}),false)?;
    Ok(report)
}
fn record_calibration_action(dir:&Path,a:&Admission,b:&Binding,identity:&ProcessIdentity,case:CalibrationCase,deadline_ms:u64,e:&mut Evidence)->Result<()> {
    let end=b.armed_ms.ok_or("calibration action watchdog")?.checked_add(540000).ok_or("calibration action bound")?;
    let current=inspect(e,&a.tools.docker,&b.container_id,end)?;
    config(&current,dir,a,&b.container_id,false)?;
    let raw=read_file(&Path::new("/proc").join(identity.init_pid.to_string()).join("stat"),4096)?;
    if current["State"]["Running"]!=true||current["State"]["Pid"]!=identity.init_pid||current["State"]["Restarting"]!=false||current["State"]["Dead"]!=false||current["State"]["OOMKilled"]!=false||current["RestartCount"]!=0||process_start(&raw)?!=(identity.init_pid,identity.init_start){return Err("intended host action lost original live container/init; calibration fails independently of closure".into());}
    let action=ktlsp_oracle_replay::authority::CalibrationAction{schema_version:1,scope:"PR174_RUST_CALIBRATION_ACTION".into(),case,nonce:a.nonce.clone(),container_id:b.container_id.clone(),boot_id:a.boot_id.clone(),daemon_id:a.daemon_id.clone(),init_pid:identity.init_pid,init_starttime:identity.init_start,armed_ms:b.armed_ms.ok_or("calibration armed time")?,deadline_ms,observed_ms:mono_ms()?,observed_unix_ms:u64::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_|"host wall clock before epoch")?.as_millis()).map_err(|_|"host wall clock overflow")?,original_running:true};
    action.validate()?;
    side_json(e,&dir.join("calibration-action.json"),&action)?;
    e.append(&json!({"kind":"calibration-action","action":action,"originalStatHex":raw_hex(&raw),"actualContainerState":current["State"],"restartCount":current["RestartCount"]}),false)
}

#[cfg(test)]
mod calibration_source_tests {
    use super::*;
    #[test]
    fn synthetic_scope_template_is_deterministic_not_runtime_calibration() {
        assert!(digest(&containment_template_sha256().unwrap()));
        assert_eq!(containment_template_sha256(),containment_template_sha256());
        assert_eq!(CalibrationCase::ALL.iter().filter(|case|case.forced()).count(),3);
        assert!(sha_bytes(b"one")!=sha_bytes(b"two"));
    }
}

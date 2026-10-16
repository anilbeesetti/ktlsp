#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
struct StateMountIdentity { mount_id:u64,device:u64,inode:u64,row:String }
fn state_mount_identity(dir:&Path)->Result<StateMountIdentity> {
    let target=dir.join("state");let name=target.to_str().ok_or("state mount encoding")?;
    let raw=read_file(Path::new("/proc/self/mountinfo"),65536)?;
    let text=std::str::from_utf8(&raw).map_err(|_|"state mount table UTF8")?;
    let mut identity=None;
    for row in text.lines() {
        let fields=row.split_whitespace().collect::<Vec<_>>();
        if fields.len()<10{return Err("state mount malformed row".into());}
        if fields[4].strip_prefix(name).is_some_and(|tail|tail.starts_with('/')){return Err("unexpected nested host mount prevents state release".into());}
        if fields[4]==name {
            let split=fields.iter().position(|s|*s=="-").ok_or("state mount delimiter")?;
            if identity.is_some()||fields.get(split+1)!=Some(&"tmpfs")||!fields[5].split(',').any(|s|s=="rw")||fields[6..split].iter().any(|s|s.starts_with("shared:")||s.starts_with("master:")){return Err("state mount type/propagation/identity ambiguous".into());}
            let metadata=fs::symlink_metadata(&target).map_err(|e|e.to_string())?;
            if !metadata.is_dir()||metadata.uid()!=65532||metadata.gid()!=65532{return Err("state mount owned object mismatch".into());}
            identity=Some(StateMountIdentity{mount_id:fields[0].parse().map_err(|_|"state mount ID")?,device:metadata.dev(),inode:metadata.ino(),row:row.into()});
        }
    }
    identity.ok_or("state tmpfs mount disappeared".into())
}
fn capacity_before_preparation(expected:&RootGrant,dir:&Path,end:u64,e:&mut Evidence)->Result<()> {
    let tool=expected.tools.iter().find(|tool|tool.role=="stat").ok_or("ROOT stat role")?;
    let path=Path::new(&tool.path);protected(path,false)?;
    if sha_path(path,end)?!=tool.sha256{return Err("ROOT stat measurement seal".into());}
    let raw=e.checked(path,&["--file-system".into(),"--format=%S %b %f %a %c %d %T".into(),dir.to_string_lossy().into_owned()],end)?;
    let fields=std::str::from_utf8(&raw).map_err(|_|"preparation stat encoding")?.split_whitespace().collect::<Vec<_>>();
    if fields.len()!=7{return Err("preparation capacity fields".into());}
    let block=fields[0].parse::<u64>().map_err(|_|"preparation block size")?;
    let available=fields[3].parse::<u64>().map_err(|_|"preparation available blocks")?.checked_mul(block).ok_or("preparation capacity overflow")?;
    let inodes=fields[5].parse::<u64>().map_err(|_|"preparation free inodes")?;
    e.append(&json!({"kind":"actual-preparation-capacity","directory":dir,"rawHex":raw_hex(&raw),"availableBytes":available,"freeInodes":inodes,"requiredArchiveBytes":368488700u64,"requiredUnpackedMemberBytes":1199734503u64,"scope":"512MiB capture tmpfs does not cover preparation; no host quota claim"}),false)?;
    if available<1_568_223_203 || inodes<1200{return Err("actual preparation filesystem lacks sealed archive+distribution bytes/inodes; original failure retained".into());}Ok(())
}
fn release_calibration_preparation(dir:&Path)->Result<bool> {
    let a:Admission=load(&dir.join("admission.json"))?;
    let Some(case)=calibration_case(dir,&a)? else{return Ok(false);};
    // A cancellation case remains untouched until actual runner cancellation cause is authenticated.
    if case==CalibrationCase::WorkflowStepCancel{return Ok(false);}
    let teardown:Value=load(&dir.join("teardown-receipt.json"))?;
    if teardown["teardown"]!="CONTAINER_TEARDOWN_CONFIRMED"{return Ok(false);}
    let b=binding(dir)?;read_host_observation(dir,case,&b.container_id)?;
    protected(&dir.join("export/export-binding.json"),false)?;
    let peer:PeerReport=load(&dir.join("export/capture/calibration-peer.json"))?;peer.validate_case(case)?;
    require_actual_calibration_action(dir,&a,&b,case)?;
    let original:StateMountIdentity=load(&dir.join("state-mount.json"))?;
    let actual=state_mount_identity(dir)?;
    if (original.mount_id,original.device,original.inode,&original.row)!=(actual.mount_id,actual.device,actual.inode,&actual.row){return Err("recorded state mount changed; no unmount/disposal".into());}
    let pending=Path::new(ROOT).join("preparation-disposal-in-progress");
    make_file(&pending,format!("{}\n",a.nonce).as_bytes(),0o400)?;
    // Docker-owned private input binds disappeared with the proven removed container namespace.
    // Host mountinfo proves no unknown descendant remains: ordinary unmount only.
    ordinary_state_unmount(&dir.join("state"))?;
    let distribution=dir.join("inputs/distribution");protected(&distribution,true)?;
    fs::remove_dir_all(&distribution).map_err(|e|e.to_string())?;
    let archive=dir.join("inputs/archive.tar.gz");protected(&archive,false)?;
    fs::remove_file(archive).map_err(|e|e.to_string())?;
    make_file(&dir.join("preparation-release.json"),&serde_json::to_vec(&json!({"schemaVersion":1,"scope":"PR174_CALIBRATION_BULK_RELEASE","nonce":a.nonce,"mountId":original.mount_id,"ordinaryUnmount":true,"sealedStagingRetained":true,"authorityInventoryRawExportRetained":true,"containerClosureUnchanged":true})).map_err(|e|e.to_string())?,0o400)?;
    fs::remove_file(pending).map_err(|e|e.to_string())?;
    Ok(true)
}

fn ordinary_state_unmount(path:&Path)->Result<()> {
    #[cfg(target_os="linux")]
    {
        let target=std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).map_err(|_|"state unmount target")?;
        unsafe extern "C"{fn umount2(target:*const std::ffi::c_char,flags:i32)->i32;}
        if unsafe{umount2(target.as_ptr(),0)}!=0{return Err(std::io::Error::last_os_error().to_string());}
        Ok(())
    }
    #[cfg(not(target_os="linux"))]
    { let _=path;Err("ordinary state release is Linux-only".into()) }
}
fn release_calibration(dir:&Path)->Result<Value> {
    match release_calibration_preparation(dir) {
        Ok(released)=>Ok(json!({"status":if released{"OWNED_CALIBRATION_BULK_RELEASED"}else{"ORIGINAL_CASE_RETAINED"},"directory":dir,"containerClosureUnchanged":true,"officialCaptureSuccess":false})),
        Err(error)=>{
            let bytes=serde_json::to_vec(&json!({"scope":"PR174_PREPARATION_RELEASE_FAILURE","directory":dir,"error":error,"containerClosureUnchanged":true,"nextLaunchAllowed":false})).map_err(|e|e.to_string())?;
            if bytes.len()<=2048 {
                let _=make_file(&dir.join("preparation-release-failure.json"),&bytes,0o400);
                let _=make_file(&Path::new(ROOT).join("preparation-disposal-failed"),&bytes,0o400);
            }
            Err(error)
        }
    }
}

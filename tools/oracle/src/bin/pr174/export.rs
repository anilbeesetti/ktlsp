use ktlsp_oracle_replay::authority::{ExportBinding as ProtectedExportBinding,ExportFile as ProtectedExportFile};
fn export_copy(mut input:File,target:&Path,cap:u64,end:u64)->Result<(u64,String)> {
    let before=input.metadata().map_err(|e|e.to_string())?;
    if !before.is_file()||before.nlink()!=1||before.len()>cap{return Err("export input type/single-link/bounds".into());}
    let mut out=OpenOptions::new().write(true).create_new(true).mode(0o400).open(target).map_err(|e|e.to_string())?;
    let mut hash=Sha256::new();let mut buffer=[0u8;65536];let mut size=0u64;
    loop {if mono_ms()?>=end{return Err("protected export deadline".into());}let n=input.read(&mut buffer).map_err(|e|e.to_string())?;if n==0{break;}size+=n as u64;if size>cap||size>before.len(){return Err("export input grew".into());}hash.update(&buffer[..n]);out.write_all(&buffer[..n]).map_err(|e|e.to_string())?;}
    let after=input.metadata().map_err(|e|e.to_string())?;
    if size!=before.len()||before.dev()!=after.dev()||before.ino()!=after.ino()||before.mtime()!=after.mtime()||before.mtime_nsec()!=after.mtime_nsec()||before.ctime()!=after.ctime()||before.ctime_nsec()!=after.ctime_nsec(){return Err("export input changed".into());}
    out.sync_all().map_err(|e|e.to_string())?;Ok((size,format!("{:x}",hash.finalize())))
}
fn protected_export(dir:&Path)->Result<Value> {
    protected_export_until(dir,mono_ms()?.checked_add(10000).ok_or("export deadline overflow")?)
}
fn protected_export_until(dir:&Path,end:u64)->Result<Value> {
    // Export is never a success gate. Preserve original failed/incomplete files exactly and expose missing inventory.
    let a:Admission=load(&dir.join("admission.json"))?;
    let b=binding(dir)?;
    let teardown:Value=load(&dir.join("teardown-receipt.json"))?;
    if teardown["containerId"]!=b.container_id||teardown["nonce"]!=b.nonce{return Err("export teardown/binding original identity mismatch".into());}
    // No child-controlled file is copied while any original namespace/cgroup process might write it.
    let closed=teardown["teardown"]=="CONTAINER_TEARDOWN_CONFIRMED";
    if mono_ms()?>=end{return Err("protected export reserved service budget expired".into());}
    let output=dir.join("export");mkdir(&output,0o700)?;
    for name in ["capture","receipts","preflight"]{mkdir(&output.join(name),0o700)?;}
    let mut inventory=Vec::new();let mut total=0u64;
    for (path,cap) in [("admission.json",16384),("root-authorization.json",16384),("expected-grant.json",16384),("calibration.json",16384),("workflow.yml",16384),("binding.json",16384),("process-identity.json",16384),("state-mount.json",16384),("calibration-action.json",16384),("child-confinement-profile.json",16384),("teardown-receipt.json",16384),("armed.json",16384),("start-intent.json",16384),("start-result.json",16384),("cleanup-begun.json",16384),("bootstrap-receipt.json",131072),("preflight/identity.json",16384),("preflight/host-preflight.json",16384),("preflight/launch-authorization.json",16384),("receipts/observer.evidence",786432),("receipts/cleanup.evidence",262144)] {
        let source=dir.join(path);
        match fs::symlink_metadata(&source){Err(error)if error.kind()==ErrorKind::NotFound=>continue,Err(error)=>return Err(error.to_string()),Ok(_)=>{}}
        protected(&source,false)?;
        let input=OpenOptions::new().read(true).custom_flags(no_follow()|0x800).open(&source).map_err(|e|e.to_string())?;
        let (bytes,sha256)=export_copy(input,&output.join(path),cap,end)?;total+=bytes;
        inventory.push(ProtectedExportFile{path:path.into(),bytes,sha256});
    }
    let calibration=a.scope=="PR174_RUST_CALIBRATION_ADMITTED";
    let names:&[(&str,u64)]=if calibration {&[("calibration-ready.json",16384),("calibration-peer.json",16384)]}else{&[("manifest.json",8388608),("manifest.next",8388608),("metadata.json",8388608),("transcript.json",8388608),("child-confinement.json",16384),("client-intended.raw",8388608),("stdout.raw",8388608),("stderr.raw",65536)]};
    for (name,cap) in names {
        if !closed { break; } // Retain protected host facts even when mutable child output cannot safely be exported.
        match fs::symlink_metadata(dir.join("state/capture").join(name)){Err(error)if error.kind()==ErrorKind::NotFound=>continue,Err(error)=>return Err(error.to_string()),Ok(_)=>{}}
        let input=capture_file(dir,name)?;
        let path=format!("capture/{name}");let(bytes,sha256)=export_copy(input,&output.join(&path),*cap,end)?;total+=bytes;
        if total>67108864{return Err("finite protected export aggregate cap".into());}
        inventory.push(ProtectedExportFile{path,bytes,sha256});
    }
    inventory.sort_by(|a,b|a.path.cmp(&b.path));
    let binding=ProtectedExportBinding{schema_version:1,scope:"PR174_PROTECTED_EXPORT".into(),run:a.run,nonce:a.nonce,container_id:b.container_id,boot_id:a.boot_id,daemon_id:a.daemon_id,image:a.image,image_id:a.image_id,root_authorization_sha256:a.root_authorization_sha256,calibration_sha256:a.calibration_sha256,workflow_sha256:a.workflow_sha256,files:inventory};
    let bytes=serde_json::to_vec(&binding).map_err(|e|e.to_string())?;
    if bytes.len()>INPUT_BYTES{return Err("protected export binding cap".into());}
    make_file(&output.join("export-binding.json"),&bytes,0o400)?;
    Ok(json!({"status":"PROTECTED_EXPORT_AVAILABLE","directory":output,"bindingSha256":sha_bytes(&bytes),"originalClosureConfirmed":closed,"officialCaptureSuccess":false,"originalWorkflowArtifactCustody":"receiver must authenticate original run/job/attempt; artifact name alone not authority"}))
}

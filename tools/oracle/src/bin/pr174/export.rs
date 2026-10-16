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
    for name in ["capture","receipts","preflight","bootstrap"]{mkdir(&output.join(name),0o700)?;}
    let mut inventory=Vec::new();let mut total=0u64;
    for (path,cap) in [("admission.json",16384),("root-authorization.json",16384),("expected-grant.json",16384),("calibration.json",16384),("workflow.yml",16384),("binding.json",16384),("process-identity.json",16384),("state-mount.json",16384),("calibration-action.json",16384),("calibration-observer-ready.json",16384),("child-confinement-profile.json",16384),("teardown-receipt.json",16384),("armed.json",16384),("start-intent.json",16384),("start-result.json",16384),("cleanup-begun.json",16384),("bootstrap-receipt.json",4096),("bootstrap/observer.evidence",126976),("preflight/identity.json",16384),("preflight/host-preflight.json",16384),("preflight/launch-authorization.json",16384),("receipts/observer.evidence",786432),("receipts/cleanup.evidence",262144)] {
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
    publish_export_binding(&output,&bytes)?;
    Ok(json!({"status":"PROTECTED_EXPORT_AVAILABLE","directory":output,"bindingSha256":sha_bytes(&bytes),"originalClosureConfirmed":closed,"officialCaptureSuccess":false,"originalWorkflowArtifactCustody":"receiver must authenticate original run/job/attempt; artifact name alone not authority"}))
}

fn publish_export_binding(output:&Path,bytes:&[u8])->Result<()> {
    let next=output.join("export-binding.next");
    make_file(&next,bytes,0o400)?;
    publish_complete_no_replace(&next,&output.join("export-binding.json"))
}

#[cfg(test)]
mod export_source_tests {
    use super::*;

    #[test]
    fn synthetic_export_readiness_requires_complete_exclusive_publication() {
        let dir=std::env::temp_dir().join(format!("ktlsp-export-publication-synthetic-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&dir).unwrap();
        let next=dir.join("export-binding.next");
        let published=dir.join("export-binding.json");
        make_file(&next,b"{",0o400).unwrap();
        assert!(publish_export_binding(&dir,b"{\"complete\":true}").is_err());
        assert!(!published.exists());
        assert_eq!(fs::read(&next).unwrap(),b"{");
        fs::remove_file(&next).unwrap();
        publish_export_binding(&dir,b"{\"complete\":true}").unwrap();
        assert_eq!(fs::read(&published).unwrap(),b"{\"complete\":true}");
        assert_eq!(fs::metadata(&published).unwrap().nlink(),1);
        assert!(!next.exists());
        assert!(publish_export_binding(&dir,b"{\"replacement\":true}").is_err());
        assert_eq!(fs::read(&published).unwrap(),b"{\"complete\":true}");
        assert_eq!(fs::read(&next).unwrap(),b"{\"replacement\":true}");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn synthetic_atomic_publication_has_one_complete_single_link_winner() {
        let dir=std::env::temp_dir().join(format!("ktlsp-publication-race-synthetic-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&dir).unwrap();
        let published=dir.join("ready");
        let barrier=std::sync::Arc::new(std::sync::Barrier::new(3));
        let mut publishers=Vec::new();
        for (name,bytes) in [("a.next",b"{\"complete\":\"a\"}".as_slice()),("b.next",b"{\"complete\":\"b\"}".as_slice())] {
            let next=dir.join(name);
            make_file(&next,bytes,0o400).unwrap();
            let final_path=published.clone();
            let barrier=barrier.clone();
            publishers.push(std::thread::spawn(move || {barrier.wait();publish_complete_no_replace(&next,&final_path)}));
        }
        barrier.wait();
        let end=Instant::now()+Duration::from_secs(5);
        while publishers.iter().any(|thread|!thread.is_finished()) {
            match fs::read(&published) {
                Ok(bytes) => {
                    assert!(bytes==b"{\"complete\":\"a\"}"||bytes==b"{\"complete\":\"b\"}");
                    assert_eq!(fs::metadata(&published).unwrap().nlink(),1);
                }
                Err(error) if error.kind()==ErrorKind::NotFound => {}
                Err(error) => panic!("publication reader: {error}"),
            }
            assert!(Instant::now()<end,"bounded synthetic publication");
            std::thread::yield_now();
        }
        let winners=publishers.into_iter().map(|thread|thread.join().unwrap()).filter(|result|result.is_ok()).count();
        assert_eq!(winners,1);
        assert_eq!(fs::metadata(&published).unwrap().nlink(),1);
        assert_eq!([dir.join("a.next"),dir.join("b.next")].iter().filter(|path|path.exists()).count(),1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn synthetic_publication_syscall_and_path_errors_preserve_originals() {
        use std::os::unix::ffi::OsStrExt;
        let dir=std::env::temp_dir().join(format!("ktlsp-publication-errors-synthetic-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&dir).unwrap();
        let staged=dir.join("ready.next");
        make_file(&staged,b"complete",0o400).unwrap();
        let invalid=dir.join(std::ffi::OsStr::from_bytes(b"invalid\0destination"));
        assert!(publish_complete_no_replace(&staged,&invalid).is_err());
        assert_eq!(fs::read(&staged).unwrap(),b"complete");
        let victim=dir.join("victim");
        make_file(&victim,b"preserved",0o400).unwrap();
        let final_path=dir.join("ready");
        std::os::unix::fs::symlink(&victim,&final_path).unwrap();
        assert!(publish_complete_no_replace(&staged,&final_path).is_err());
        assert_eq!(fs::read_link(&final_path).unwrap(),victim);
        assert_eq!(fs::read(&victim).unwrap(),b"preserved");
        assert_eq!(fs::read(&staged).unwrap(),b"complete");
        assert!(publish_complete_no_replace(&dir.join("missing"),&dir.join("absent")).is_err());
        assert!(!dir.join("absent").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}

use ktlsp_oracle_replay::authority::{CalibrationCase, CalibrationPlan, CalibrationReport, GrantScope, HostSeal, RootGrant};
use sha2::{Digest, Sha256};

const EXPECTED_GRANT: &str = "/etc/ktlsp-pr174/expected-grant.json";
const STAGING: &str = "/etc/ktlsp-pr174/staging";

fn no_follow() -> i32 {
    #[cfg(target_os="linux")] { 0x20000 }
    #[cfg(not(target_os="linux"))] { 0x100 }
}
fn os_release() -> Result<Vec<u8>> {
    let path=fs::canonicalize("/etc/os-release").map_err(|e|e.to_string())?;
    protected(&path,false)?;
    read_file(&path,INPUT_BYTES)
}
fn sha_bytes(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn sha_path(path: &Path, end: u64) -> Result<String> {
    let mut file = OpenOptions::new().read(true).custom_flags(no_follow()).open(path).map_err(|e|e.to_string())?;
    let before = file.metadata().map_err(|e|e.to_string())?;
    if !before.is_file() || before.nlink() != 1 { return Err("sealed file regular/single-link custody".into()); }
    let mut hash = Sha256::new(); let mut buffer = [0u8; 65536]; let mut bytes = 0u64;
    loop {
        if mono_ms()? >= end { return Err("streamed hash preparation deadline".into()); }
        let n = file.read(&mut buffer).map_err(|e|e.to_string())?;
        if n == 0 { break; }
        bytes = bytes.checked_add(n as u64).ok_or("hash length overflow")?;
        if bytes > before.len() { return Err("sealed file grew during hashing".into()); }
        hash.update(&buffer[..n]);
    }
    let after = file.metadata().map_err(|e|e.to_string())?;
    let named = fs::symlink_metadata(path).map_err(|e|e.to_string())?;
    if before.dev()!=after.dev() || before.ino()!=after.ino() || before.len()!=bytes || before.mtime()!=after.mtime() || before.mtime_nsec()!=after.mtime_nsec() || before.ctime()!=after.ctime() || before.ctime_nsec()!=after.ctime_nsec() || named.dev()!=after.dev() || named.ino()!=after.ino() || !named.is_file() { return Err("sealed file changed while hashing".into()); }
    Ok(format!("{:x}",hash.finalize()))
}
fn measured_host() -> Result<HostSeal> {
    Ok(HostSeal { os_release_sha256: sha_bytes(&os_release()?), kernel_release_sha256: sha_bytes(&read_file(Path::new("/proc/sys/kernel/osrelease"),1024)?), architecture: std::env::consts::ARCH.into() })
}
fn root_authority(dir: &Path, a: &Admission) -> Result<RootGrant> {
    // The fixed protected expected grant is installed independently by ROOT's reviewed original workflow.
    // Host root/workflow are trusted; hashes do not cryptographically authenticate a hostile root.
    let expected: RootGrant = load(Path::new(EXPECTED_GRANT))?;
    let grant: RootGrant = load(&dir.join("root-authorization.json"))?;
    grant.validate(&expected)?;
    let claim=grant_claim(&expected)?;
    protected(&claim,false)?;
    if read_file(&claim,128)? != format!("{}\n",a.nonce).as_bytes(){return Err("ROOT finite launch grant already claimed by another nonce".into());}
    no_disposal_failure()?;
    let scope = if grant.scope == GrantScope::OfficialCapture { "PR174_HOST_EXECUTION_ADMITTED" } else { "PR174_RUST_CALIBRATION_ADMITTED" };
    if a.scope != scope || a.run.repository != grant.repository || a.run.event != grant.event || a.run.attempt != grant.attempt || a.run.run_id != grant.run_id || a.run.job_id != grant.job_id || a.run.suite_id != grant.suite_id || a.run.source_head != grant.source_head || a.run.source_tree != grant.source_tree || a.workflow_sha256 != grant.workflow_sha256 || a.host_binary_sha256 != grant.host_binary_sha256 || a.adapter_sha256 != grant.adapter_sha256 || a.image != grant.image || a.image_id != grant.image_id || measured_host()? != grant.host { return Err("Admission differs from independently ROOT admitted source/run/image/host".into()); }
    for (path,role) in [(&a.tools.docker,"docker"),(&a.tools.systemctl,"systemctl"),(&a.tools.sha256sum,"sha256sum"),(&a.tools.gh,"gh"),(&a.tools.mount,"mount"),(&a.tools.findmnt,"findmnt"),(&a.tools.stat,"stat"),(&a.tools.curl,"curl")] {
        let seal=grant.tools.iter().find(|t|t.role==role).ok_or("missing ROOT tool role")?;
        protected(path,false)?;
        if path.to_str()!=Some(seal.path.as_str()) || sha_path(path,a.preparation_deadline_ms)? != seal.sha256 { return Err("measured host tool differs from ROOT grant".into()); }
    }
    for (name,seal) in [("Cargo.lock",&grant.lock_sha256),("rust-toolchain.toml",&grant.toolchain_sha256)] {
        protected(&dir.join(name),false)?;
        if sha_path(&dir.join(name),a.preparation_deadline_ms)? != *seal { return Err("ROOT lock/toolchain source seal".into()); }
    }
    if containment_template_sha256() != grant.containment_template_sha256 { return Err("ROOT containment template source mismatch".into()); }
    match grant.scope {
        GrantScope::RustCalibration => {
            let plan: CalibrationPlan=load(&dir.join("calibration.json"))?; plan.validate(&grant)?;
            let peer=dir.join("inputs/tool/ktlsp-oracle-calibration-peer"); protected(&peer,false)?;
            if sha_path(&peer,a.preparation_deadline_ms)? != grant.peer_sha256 { return Err("Rust calibration peer source seal".into()); }
        },
        GrantScope::OfficialCapture => {
            if grant.required_calibration_sha256.as_deref()!=Some(a.calibration_sha256.as_str()) { return Err("ROOT required actual calibration hash".into()); }
            let report: CalibrationReport=load(&dir.join("calibration.json"))?; report.validate(&grant)?;
            if report.boot_id != a.boot_id || report.daemon_id != a.daemon_id { return Err("calibration is not this actual host boot/daemon".into()); }
        }
    }
    Ok(grant)
}
fn calibration_case(dir: &Path, a: &Admission) -> Result<Option<CalibrationCase>> {
    if a.scope=="PR174_RUST_CALIBRATION_ADMITTED" {
        let p: CalibrationPlan=load(&dir.join("calibration.json"))?;
        if p.schema_version!=1 || p.scope!=GrantScope::RustCalibration { return Err("finite calibration scope".into()); }
        Ok(Some(p.case))
    } else { Ok(None) }
}
fn entrypoint(a: &Admission) -> String {
    format!("{INNER}/tool/{}",if a.scope=="PR174_RUST_CALIBRATION_ADMITTED" {"ktlsp-oracle-calibration-peer"}else{"ktlsp-oracle-official-pr174"})
}
fn entry_arguments(dir: &Path,a: &Admission) -> Result<Vec<String>> {
    Ok(match calibration_case(dir,a)? {Some(case)=>args(&["--case",case.as_str()]),None=>adapter_argv()})
}
fn containment_template_sha256() -> String {
    // A finite source projection: all shared security/resources/mount constants. Run identity and paths are separately bound.
    sha_bytes(b"PR174-CONTAINMENT-v1\0linux/amd64;init;pid=private;ipc=private;cgroupns=private;uid=65532:65532;cap-drop=ALL;nnp;readonly;network=none;memory=4294967296;swap=0;pids=256;cpus=2;restart=no;pull=never;health=none;log=none;core=0:0;shm=67108864;state=536870912:16384;bind=state:rw,fixture:ro,distribution:ro,tool:ro,preflight:ro;rprivate;watchdog=530000:540000:600000;cleanup=60000;command=10000")
}
fn copy_sealed(source:&Path,target:&Path,cap:u64,mode:u32,end:u64)->Result<()> {
    protected(source,false)?;
    let mut input=OpenOptions::new().read(true).custom_flags(no_follow()).open(source).map_err(|e|e.to_string())?;
    let metadata=input.metadata().map_err(|e|e.to_string())?;
    if metadata.nlink()!=1 || metadata.len()>cap {return Err("bootstrap input type/size/custody".into());}
    let mut output=OpenOptions::new().write(true).create_new(true).mode(mode).open(target).map_err(|e|e.to_string())?;
    let mut buffer=[0u8;65536]; let mut size=0u64;
    loop { if mono_ms()?>=end{return Err("bootstrap copy deadline".into());} let n=input.read(&mut buffer).map_err(|e|e.to_string())?; if n==0{break;} size+=n as u64;if size>cap{return Err("bootstrap input grew".into());}output.write_all(&buffer[..n]).map_err(|e|e.to_string())?; }
    if size!=metadata.len(){return Err("bootstrap input changed".into());}
    output.sync_all().map_err(|e|e.to_string())?;
    if sha_path(source,end)?!=sha_path(target,end)?{return Err("bootstrap copy source changed".into());} Ok(())
}
fn grant_claim(grant:&RootGrant)->Result<PathBuf> {
    Ok(Path::new(ROOT).join("grant-claims").join(sha_bytes(&serde_json::to_vec(grant).map_err(|e|e.to_string())?)))
}
fn no_disposal_failure()->Result<()> {
    match fs::symlink_metadata(Path::new(ROOT).join("preparation-disposal-failed")) {
        Err(error) if error.kind()==ErrorKind::NotFound=>Ok(()),
        _=>Err("prior preparation disposal failed; no later launch admission".into()),
    }
}
fn bootstrap() -> Result<Value> {
    protected(Path::new(STAGING),true)?;
    let expected:RootGrant=load(Path::new(EXPECTED_GRANT))?; expected.validate_shape()?;
    let supplied:RootGrant=load(&Path::new(STAGING).join("root-authorization.json"))?; supplied.validate(&expected)?;
    protected(Path::new(ROOT),true)?;
    no_disposal_failure()?;
    let mut randomness=[0u8;32];File::open("/dev/urandom").and_then(|mut f|f.read_exact(&mut randomness)).map_err(|e|e.to_string())?;
    let nonce=raw_hex(&randomness); if !digest(&nonce){return Err("OS randomness unavailable".into());}
    let dir=Path::new(ROOT).join(&nonce);mkdir(&dir,0o700)?;
    let end=mono_ms()?.checked_add(1_800_000).ok_or("bootstrap deadline overflow")?;
    let mut evidence=Evidence::preflight();
    evidence.limit=64_000;
    let result=(|| {
        let claims=Path::new(ROOT).join("grant-claims");
        match fs::create_dir(&claims){Ok(())=>fs::set_permissions(&claims,fs::Permissions::from_mode(0o700)).map_err(|e|e.to_string())?,Err(error)if error.kind()==ErrorKind::AlreadyExists=>{},Err(error)=>return Err(error.to_string())}
        protected(&claims,true)?;
        make_file(&grant_claim(&expected)?,format!("{nonce}\n").as_bytes(),0o400)?;
        capacity_before_preparation(&expected,&dir,end,&mut evidence)?;
        for name in ["inputs","inputs/tool","inputs/fixture"] {mkdir(&dir.join(name),0o755)?;}
        copy_sealed(Path::new(EXPECTED_GRANT),&dir.join("expected-grant.json"),16384,0o400,end)?;
        for (name,cap,mode) in [("root-authorization.json",16384,0o400),("calibration.json",16384,0o400),("workflow.yml",16384,0o400),("github.token",4096,0o600),("Cargo.lock",1048576,0o400),("rust-toolchain.toml",16384,0o400),("inputs/archive.tar.gz",368488700,0o400),("inputs/archive-members.json",193661,0o400),("inputs/tool/ktlsp-oracle-official-pr174",134217728,0o555)] {
            copy_sealed(&Path::new(STAGING).join(name),&dir.join(name),cap,mode,end)?;
        }
        if expected.scope==GrantScope::RustCalibration {copy_sealed(&Path::new(STAGING).join("inputs/tool/ktlsp-oracle-calibration-peer"),&dir.join("inputs/tool/ktlsp-oracle-calibration-peer"),134217728,0o555,end)?;}
        for (name,size,_) in FIXTURES {copy_sealed(&Path::new(STAGING).join("inputs/fixture").join(name),&dir.join("inputs/fixture").join(name),size,0o444,end)?;}
        let tool=|role:&str|->Result<PathBuf>{Ok(PathBuf::from(&expected.tools.iter().find(|t|t.role==role).ok_or("ROOT tool role")?.path))};
        let tools=Tools{docker:tool("docker")?,systemctl:tool("systemctl")?,sha256sum:tool("sha256sum")?,gh:tool("gh")?,mount:tool("mount")?,findmnt:tool("findmnt")?,stat:tool("stat")?,curl:tool("curl")?};
        for seal in &expected.tools {let path=Path::new(&seal.path);protected(path,false)?;if sha_path(path,end)?!=seal.sha256{return Err("bootstrap ROOT host-tool measurement".into());}}
        let daemon_info:Value=parse(&evidence.checked(&tools.docker,&args(&["info","--format","{{json .}}"]),end)?)?;
        let daemon_id=daemon_info["ID"].as_str().filter(|s|!s.is_empty()).ok_or("actual daemon identity unavailable")?.to_owned();
        let mut admission=Admission{schema_version:1,scope:if expected.scope==GrantScope::OfficialCapture{"PR174_HOST_EXECUTION_ADMITTED"}else{"PR174_RUST_CALIBRATION_ADMITTED"}.into(),contract_sha256:CONTRACT.into(),root_authorization_sha256:sha_path(&dir.join("root-authorization.json"),end)?,calibration_sha256:sha_path(&dir.join("calibration.json"),end)?,nonce:nonce.clone(),run:OriginalRun{repository:expected.repository.clone(),event:expected.event.clone(),attempt:expected.attempt,run_id:expected.run_id,job_id:expected.job_id,suite_id:expected.suite_id,source_head:expected.source_head.clone(),source_tree:expected.source_tree.clone()},boot_id:String::from_utf8(read_file(Path::new("/proc/sys/kernel/random/boot_id"),128)?).map_err(|_|"boot ID encoding")?.trim().into(),daemon_id,preparation_deadline_ms:end,image:expected.image.clone(),image_id:expected.image_id.clone(),host_binary_sha256:expected.host_binary_sha256.clone(),adapter_sha256:expected.adapter_sha256.clone(),extraction_manifest_sha256:String::new(),workflow_sha256:expected.workflow_sha256.clone(),tools};
        root_authority(&dir,&admission)?;
        hash_file(&mut evidence,&admission,&std::env::current_exe().map_err(|e|e.to_string())?,&expected.host_binary_sha256)?;
        core_policy(&mut evidence)?;
        // Authentication is repeated by the outside-step supervisor immediately before CREATE.
        authenticate_original(&dir,&admission,&mut evidence)?;
        materializer::materialize(&dir,&admission,&mut evidence)?;
        admission.extraction_manifest_sha256=sha_path(&dir.join("inputs/extraction-manifest.json"),end)?;
        input_pins(&dir,&admission,&mut evidence)?;
        let bytes=serde_json::to_vec(&admission).map_err(|e|e.to_string())?;if bytes.len()>INPUT_BYTES{return Err("Admission size".into());}make_file(&dir.join("admission.json"),&bytes,0o400)?;
        install_supervisor(&dir,&admission,&mut evidence)?;
        Ok(json!({"status":"SUPERVISOR_STARTED","admissionDirectory":dir,"scope":admission.scope,"authority":"independent ROOT grant plus protected original workflow/OS-root custody; runtime receipts required"}))
    })();
    let receipt=json!({"schemaVersion":1,"scope":"PR174_BOOTSTRAP","nonce":nonce,"error":result.as_ref().err(),"officialCaptureSuccess":false,"evidenceHex":raw_hex(&evidence.pending)});
    // Bootstrap preparation evidence has its own reserved part of the ONE observer allowance.
    let bytes=serde_json::to_vec(&receipt).map_err(|e|e.to_string())?;
    if bytes.len()>131072{return Err("bootstrap original evidence cap".into());}
    let saved=make_file(&dir.join("bootstrap-receipt.json"),&bytes,0o400);
    result.and_then(|v|{saved?;Ok(v)})
}
fn install_supervisor(dir:&Path,a:&Admission,e:&mut Evidence)->Result<()> {
    let executable=std::env::current_exe().map_err(|e|e.to_string())?;protected(&executable,false)?;
    let exe=executable.to_str().ok_or("supervisor executable encoding")?;
    if !exe.bytes().all(|b|b.is_ascii_alphanumeric()||b"/._-".contains(&b)){return Err("supervisor executable service encoding".into());}
    let service=format!("{}.service",unit(&a.nonce,"supervisor"));
    let path=Path::new("/run/systemd/system").join(&service);protected(Path::new("/run/systemd/system"),true)?;
    let mode=if a.scope=="PR174_RUST_CALIBRATION_ADMITTED"{"calibrate"}else{"supervise"};
    let text=format!("[Unit]\nDescription=Protected PR174 original-job supervisor\nStartLimitIntervalSec=1800s\nStartLimitBurst=1\n[Service]\nType=exec\nUser=root\nGroup=root\nExecStart={exe} {mode} {}\nKillMode=control-group\nTimeoutStartSec=10s\nTimeoutStopSec=1s\nRuntimeMaxSec=1800s\nEnvironment=PATH=/usr/bin:/bin LANG=C.UTF-8 LC_ALL=C.UTF-8\nUnsetEnvironment=RUNNER_TRACKING_ID GH_TOKEN GITHUB_TOKEN JAVA_HOME JDK_HOME LD_LIBRARY_PATH\n",dir.display());
    make_file(&path,text.as_bytes(),0o600)?;
    e.checked(&a.tools.systemctl,&args(&["daemon-reload"]),a.preparation_deadline_ms)?;
    let raw=e.checked(&a.tools.systemctl,&args(&["show","--property=FragmentPath","--property=DropInPaths","--property=User","--property=Group","--property=KillMode","--property=ExecStart","--property=Environment","--property=StartLimitBurst",&service]),a.preparation_deadline_ms)?;
    let p=properties(&raw)?;
    if p.get("FragmentPath")!=Some(&path.to_string_lossy().into_owned()) || p.get("DropInPaths").map(String::as_str)!=Some("") || p.get("User").map(String::as_str)!=Some("root") || p.get("Group").map(String::as_str)!=Some("root") || p.get("KillMode").map(String::as_str)!=Some("control-group") || p.get("StartLimitBurst").map(String::as_str)!=Some("1") || !p.get("ExecStart").is_some_and(|value| supervisor_exec_start(value,exe,mode,dir)) || p.get("Environment").map(String::as_str)!=Some("PATH=/usr/bin:/bin LANG=C.UTF-8 LC_ALL=C.UTF-8") {return Err("effective outside-step supervisor unit mismatch".into());}
    // Exact on-disk bytes plus no drop-ins bind ExecStart; effective argv/env are retained for receiver.
    if read_file(&path,INPUT_BYTES)?!=text.as_bytes(){return Err("supervisor unit changed".into());}
    e.checked(&a.tools.systemctl,&args(&["start","--no-block",&service]),a.preparation_deadline_ms)?;
    Ok(())
}
fn supervisor_exec_start(value:&str,exe:&str,mode:&str,dir:&Path)->bool {
    let fields=value.trim().strip_prefix('{').and_then(|s|s.strip_suffix('}'));
    let Some(fields)=fields else{return false;};
    if fields.contains('{')||fields.contains('}'){return false;}
    let values=fields.split(';').map(str::trim).collect::<Vec<_>>();
    values.iter().filter(|value|**value==format!("path={exe}")).count()==1
        && values.iter().filter(|value|**value==format!("argv[]={exe} {mode} {}",dir.display())).count()==1
        && values.iter().filter(|value|value.starts_with("path=")).count()==1
        && values.iter().filter(|value|value.starts_with("argv[]=")).count()==1
        && values.contains(&"ignore_errors=no")
}

#[cfg(test)]
mod bootstrap_source_tests {
    use super::*;
    #[test]
    fn synthetic_supervisor_service_readback_rejects_second_commands() {
        let dir=Path::new("/run/ktlsp-pr174/test");
        let valid="{ path=/usr/local/bin/host ; argv[]=/usr/local/bin/host calibrate /run/ktlsp-pr174/test ; ignore_errors=no ; pid=0 ; code=(null) ; status=0/0 }";
        assert!(supervisor_exec_start(valid,"/usr/local/bin/host","calibrate",dir));
        assert!(!supervisor_exec_start(valid,"/usr/local/bin/host","supervise",dir));
        assert!(!supervisor_exec_start(&format!("{valid} {valid}"),"/usr/local/bin/host","calibrate",dir));
        assert!(!supervisor_exec_start("{ path=/bin/sh ; argv[]=/bin/sh -c evil ; ignore_errors=no }","/usr/local/bin/host","calibrate",dir));
    }
}

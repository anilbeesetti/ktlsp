//! Deterministic fabricated closure records, not host/runtime evidence.
use super::*;
fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b|format!("{b:02x}")).collect() }
fn save(f: &mut Files,name: &str,v: &Value) {f.insert(name.into(),serde_json::to_vec(v).unwrap());}
fn command(program: &str,args: Value,out: &[u8]) -> Value {
    json!({"kind":"command","program":program,"args":args,"result":{"exitCode":0,"signal":null,"EOF":[true,true],"timeout":false,"truncated":false,"reaped":true,"stdoutHex":hex(out),"stderrHex":""}})
}
fn docker(args: &[&str],out: &[u8]) -> Value {let mut a=vec!["--host=unix:///run/docker.sock"];a.extend_from_slice(args);command("/usr/bin/docker",json!(a),out)}
fn closure(stage: &str,cgroup: &str,device: u64,inode: u64) -> Value {
    json!({"kind":"original-cgroup-closure","stage":stage,"path":cgroup,"originalDevice":device,"originalInode":inode,"observedMs":3000,"attributableAbsent":true})
}
fn fixture() -> (Files,ExportBinding,Vec<Value>) {
    let b=ExportBinding{schema_version:1,scope:"PR174_PROTECTED_EXPORT".into(),run:OriginalRun{repository:"anilbeesetti/ktlsp".into(),event:"workflow_dispatch".into(),attempt:1,run_id:1,job_id:2,suite_id:3,source_head:"1".repeat(40),source_tree:"2".repeat(40)},nonce:"3".repeat(64),container_id:"4".repeat(64),boot_id:"synthetic-boot".into(),daemon_id:"synthetic-daemon".into(),image:"synthetic-image".into(),image_id:"synthetic-config".into(),root_authorization_sha256:"5".repeat(64),calibration_sha256:"6".repeat(64),workflow_sha256:"7".repeat(64),files:Vec::new()};
    let mut f=Files::new();
    let cg=format!("/system.slice/docker-{}.scope",b.container_id);
    let supervisor=format!("/system.slice/ktlsp-pr174-{}-supervisor.service",b.nonce);
    save(&mut f,"process-identity.json",&json!({"initPid":100,"initStart":200,"cgroup":cg,"cgroupDevice":1,"cgroupInode":2}));
    save(&mut f,"binding.json",&json!({"supervisorCgroup":supervisor,"supervisorCgroupDevice":3,"supervisorCgroupInode":4,"docker":"/usr/bin/docker","curl":"/usr/bin/curl","systemctl":"/usr/bin/systemctl"}));
    save(&mut f,"cleanup-begun.json",&json!({"nonce":b.nonce,"containerId":b.container_id,"bootId":b.boot_id,"firstMs":2000,"endMs":62000}));
    save(&mut f,"armed.json",&json!({"armedMs":1000,"finalDeadlineMs":601000}));
    save(&mut f,"start-intent.json",&json!({"nonce":b.nonce,"containerId":b.container_id,"bootId":b.boot_id}));
    save(&mut f,"start-result.json",&json!({"containerId":b.container_id,"complete":true}));
    save(&mut f,"teardown-receipt.json",&json!({"schemaVersion":1,"nonce":b.nonce,"containerId":b.container_id,"bootId":b.boot_id,"teardown":"CONTAINER_TEARDOWN_CONFIRMED","directPrearm":false,"endMs":62000}));
    let inspected=serde_json::to_vec(&json!([{"Id":b.container_id,"State":{"Running":false,"Restarting":false,"Dead":false,"Pid":0,"ExitCode":0,"OOMKilled":false},"RestartCount":0}])).unwrap();
    let mut rows=vec![command("/usr/bin/systemctl",json!(["show","--property=ActiveState",format!("ktlsp-pr174-{}-supervisor.service",b.nonce)]),b"ActiveState=inactive\n"),closure("supervisor-stopped",&supervisor,3,4),docker(&["info","--format","{{json .}}"],br#"{"ID":"synthetic-daemon"}"#),docker(&["container","inspect",&b.container_id],&inspected),docker(&["wait",&b.container_id],b"0\n")];
    for stage in ["pre-removal","post-removal"] {
        rows.push(json!({"kind":"original-init-closure","stage":stage,"hostPid":100,"originalStarttime":200,"statHex":"","originalGone":true,"observedMs":3000}));
        rows.push(closure(stage,&cg,1,2));
        if stage=="pre-removal" {
            rows.push(docker(&["rm",&b.container_id],format!("{}\n",b.container_id).as_bytes()));
            rows.push(docker(&["info","--format","{{json .}}"],br#"{"ID":"synthetic-daemon"}"#));
            rows.push(docker(&["container","ls","--all","--no-trunc","--format","{{json .ID}}"],b""));
            rows.push(command("/usr/bin/curl",json!(["--silent","--show-error","--max-time","10","--include","--unix-socket","/run/docker.sock","--noproxy","*",format!("http://localhost/v1.47/containers/{}/json",b.container_id)]),format!("HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\n\r\n{}\n",json!({"message":format!("No such container: {}",b.container_id)})).as_bytes()));
        }
    }
    persist(&mut f,&rows);
    (f,b,rows)
}
fn persist(f: &mut Files,rows: &[Value]) {let mut bytes=Vec::new();for r in rows{serde_json::to_writer(&mut bytes,r).unwrap();bytes.push(b'\n');}f.insert("receipts/cleanup.evidence".into(),bytes);}
#[test]
fn original_cid_closure_requires_every_raw_join_not_terminal_boolean() {
    let (files,b,rows)=fixture();assert!(teardown(&files,&b,true).is_ok());
    for index in 0..rows.len() {
        let mut f=files.clone();let mut r=rows.clone();r.remove(index);persist(&mut f,&r);
        assert!(teardown(&f,&b,true).is_err(),"removed original proof {index}");
    }
    for (pointer,value) in [("/containerId",json!("f".repeat(64))),("/teardown",json!("UNKNOWN")),("/endMs",json!(62001)),("/directPrearm",json!(true))] {
        let mut f=files.clone();let mut receipt=json_bytes(file(&f,"teardown-receipt.json").unwrap()).unwrap();*receipt.pointer_mut(pointer).unwrap()=value;save(&mut f,"teardown-receipt.json",&receipt);assert!(teardown(&f,&b,true).is_err(),"{pointer}");
    }
    let mut f=files.clone();let bytes=f.get_mut("receipts/cleanup.evidence").unwrap();bytes.pop();assert!(teardown(&f,&b,true).is_err());
    let mut f=files;let mut r=rows;r[3]["result"]["truncated"]=json!(true);persist(&mut f,&r);assert!(teardown(&f,&b,true).is_err());
}

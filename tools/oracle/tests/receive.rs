//! All fixtures here are fabricated synthetic mechanics, NEVER official evidence.
use ktlsp_oracle_replay::{Evidence, Limits, replay, recorder::{CaptureLimits, record}, receiver::{self, Files, retrieval::read_zip, verdict::{self, ExportBinding, ExportFile, OriginalRun}}};
use ktlsp_oracle_replay::authority::{RootGrant, GrantScope, CalibrationCase, HostSeal, ToolSeal};
use ktlsp_protocol::write_frame;
use serde_json::{Value,json};

fn put(files: &mut Files, name: &str, v: &Value) { files.insert(name.into(),serde_json::to_vec(v).unwrap()); }
fn get(files: &Files, name: &str) -> Value { serde_json::from_slice(&files[name]).unwrap() }
fn fixture(result: Value) -> Files {
    let clients: Vec<Value> = serde_json::from_slice(include_bytes!("../fixtures/official-pr174-symbols/script.json")).unwrap();
    let mut records = Vec::new();
    for client in clients {
        records.push(json!({"direction":"client","message":client}));
        if let Some(id) = records.last().unwrap()["message"]["id"].as_u64() {
            let result = match id {
                1 => json!({"serverInfo":{"version":"ILS-263.6379.0"},"capabilities":{"positionEncoding":"utf-16","documentSymbolProvider":true}}),
                2 => result.clone(), _ => Value::Null,
            };
            records.push(json!({"direction":"server","message":{"jsonrpc":"2.0","id":id,"result":result}}));
        }
    }
    from_records(records)
}
fn from_records(records: Vec<Value>) -> Files {
    let mut files = Files::new();
    let mut raw = [Vec::new(),Vec::new()];
    let mut frames = [Vec::new(),Vec::new()];
    let mut chronology = Vec::new();
    for record in &records {
        let dir = usize::from(record["direction"] == "server");
        let start = raw[dir].len();
        write_frame(&mut raw[dir],&serde_json::to_vec(&record["message"]).unwrap()).unwrap();
        let end = raw[dir].len();
        chronology.push(json!({"direction":record["direction"],"frameIndex":frames[dir].len()}));
        frames[dir].push(if dir == 0 { json!({"start":start,"end":end,"intendedLength":end-start,"persistedBytes":end-start,"acceptedBytes":end-start}) } else {json!({"start":start,"end":end,"message":record["message"]})});
    }
    let transcript = serde_json::to_vec(&records).unwrap();
    let manifest = json!({"schemaVersion":1,"complete":true,"failureReason":null,"scriptCompleted":true,
        "limits":{"phaseMs":60000,"sessionMs":300000,"cleanupMs":5000,"streamBytes":8388608,"stderrBytes":65536,"messages":1024},"elapsedMs":1,
        "export":{"complete":true,"records":records.len(),"bytes":transcript.len(),"failureReason":null},
        "chronology":chronology,"clientFrames":frames[0],"stdoutFrames":frames[1],
        "stdout":{"bytes":raw[1].len(),"observedBytes":raw[1].len(),"unpersistedBytes":0,"storageFailed":false,"eof":true,"truncated":false,"undecodedStart":raw[1].len(),"undecodedEnd":raw[1].len()},
        "stderr":{"bytes":0,"observedBytes":0,"unpersistedBytes":0,"storageFailed":false,"eof":true,"truncated":false},
        "exit":{"observed":true,"code":0,"signal":null},
        "cleanup":{"forcedTermination":false,"before":{"childExitObserved":true,"stdoutEof":true,"stderrEof":true},"error":null}});
    put(&mut files,"capture/manifest.json",&manifest);
    put(&mut files,"capture/metadata.json",&json!({"release":"kotlin-lsp/v263.6379.0","sha256":receiver::ARCHIVE_SHA256,"returncode":0}));
    files.insert("capture/transcript.json".into(),transcript);
    files.insert("capture/client-intended.raw".into(),raw[0].clone());
    files.insert("capture/stdout.raw".into(),raw[1].clone());
    files.insert("capture/stderr.raw".into(),Vec::new());
    files
}
fn document_symbol() -> Value {
    json!({"name":"Host","kind":5,"range":{"start":{"line":0,"character":0},"end":{"line":8,"character":0}},"selectionRange":{"start":{"line":0,"character":6},"end":{"line":0,"character":10}}})
}
#[test]
fn synthetic_symbols_and_null_are_data_consistency_not_evidence() {
    let files = fixture(json!([document_symbol()]));
    let report = receiver::transport(&files,true).unwrap();
    assert!(report.useful_symbols);
    assert!(!report.peer_consumption_claimed);
    for result in [json!([]),Value::Null] {
        assert!(!receiver::transport(&fixture(result),true).unwrap().useful_symbols);
    }
    assert!(receiver::transport(&fixture(json!([{"name":"Host","kind":5,"location":{"uri":"file:///tmp/ktlsp-t045-pr174-host/fixture/declarations.kt","range":{"start":{"line":0,"character":0},"end":{"line":0,"character":10}}}}])),true).unwrap().useful_symbols);
}
#[test]
fn adverse_shape_utf16_caps_offsets_eof_and_original_exit_refuse() {
    for name in ["", " \t", "\n\r", "\u{2003}"] {
        let mut invalid = document_symbol();
        invalid["name"] = json!(name);
        assert!(receiver::transport(&fixture(json!([invalid])),true).is_err(),"name {name:?}");
    }
    for symbol in [
        json!({"name":"x","kind":0}),
        json!({"name":"x","kind":1,"range":null,"selectionRange":null}),
        json!({"name":"x","kind":1,"range":{"start":{"line":0,"character":2},"end":{"line":0,"character":1}},"selectionRange":null}),
        json!({"name":"x","kind":1,"location":{"uri":"file:///wrong","range":{"start":{"line":0,"character":0},"end":{"line":0,"character":1}}}}),
    ] { assert!(receiver::transport(&fixture(json!([symbol])),true).is_err()); }
    let text = include_str!("../fixtures/official-pr174-symbols/declarations.kt");
    let pos = ktlsp_text::offset_to_position(text,text.find('🦀').unwrap()).unwrap();
    let bad = json!({"name":"x","kind":13,"range":{"start":{"line":pos.line,"character":pos.character},"end":{"line":pos.line,"character":pos.character+2}},"selectionRange":{"start":{"line":pos.line,"character":pos.character+1},"end":{"line":pos.line,"character":pos.character+2}}});
    assert!(receiver::transport(&fixture(json!([bad])),true).is_err());
    for (pointer,value) in [
        ("/clientFrames/0/acceptedBytes",json!(0)), ("/clientFrames/0/start",json!(1)),
        ("/stdoutFrames/0/end",json!(usize::MAX)), ("/chronology/0/frameIndex",json!(1)),
        ("/stdout/eof",json!(false)), ("/stderr/truncated",json!(true)),
        ("/stdout/unpersistedBytes",json!(1)), ("/exit/code",json!(1)),
        ("/cleanup/forcedTermination",json!(true)), ("/cleanup/before/stdoutEof",json!(false)),
        ("/limits/messages",json!(1)), ("/limits/sessionMs",json!(300001)),
        ("/export/complete",json!(false)), ("/export/bytes",json!(0)),
    ] {
        let mut f = fixture(Value::Null);
        let mut m = get(&f,"capture/manifest.json");
        *m.pointer_mut(pointer).unwrap() = value;
        put(&mut f,"capture/manifest.json",&m);
        assert!(receiver::transport(&f,true).is_err(),"{pointer}");
    }
    let mut f = fixture(Value::Null);
    f.get_mut("capture/stdout.raw").unwrap().extend_from_slice(b"junk");
    assert!(receiver::transport(&f,true).is_err());
    assert!(receiver::json_bytes(br#"{"x":{"a":1,"a":2}}"#).is_err());
}
#[test]
fn replay_refusal_is_not_reinterpreted_by_receiver() {
    let mut records = get(&fixture(Value::Null),"capture/transcript.json").as_array().unwrap().clone();
    records.swap(0,1);
    let f = from_records(records);
    assert!(Evidence::parse(&f["capture/transcript.json"],&f["capture/metadata.json"]).is_err());
    assert!(receiver::transport(&f,true).is_err());
}
fn grant() -> RootGrant {
    RootGrant {schema_version:1,scope:GrantScope::RustCalibration,repository:"anilbeesetti/ktlsp".into(),event:"workflow_dispatch".into(),attempt:1,run_id:1,job_id:2,suite_id:3,source_head:"1".repeat(40),source_tree:"2".repeat(40),workflow_path:ktlsp_oracle_replay::authority::WORKFLOW_PATH.into(),workflow_sha256:receiver::hash(b"synthetic workflow"),contract_sha256:ktlsp_oracle_replay::authority::CONTRACT_SHA256.into(),host_binary_sha256:"3".repeat(64),adapter_sha256:"4".repeat(64),peer_sha256:"5".repeat(64),lock_sha256:"6".repeat(64),toolchain_sha256:"7".repeat(64),containment_template_sha256:"8".repeat(64),image:format!("example@sha256:{}","9".repeat(64)),image_id:format!("sha256:{}","a".repeat(64)),tools:["docker","systemctl","sha256sum","gh","mount","findmnt","stat","curl"].into_iter().map(|role|ToolSeal{role:role.into(),path:format!("/usr/bin/{role}"),sha256:"b".repeat(64)}).collect(),required_calibration_sha256:None,calibration_case:Some(CalibrationCase::Normal),calibration_step:None,host:HostSeal{os_release_sha256:"c".repeat(64),kernel_release_sha256:"d".repeat(64),architecture:"x86_64".into()}}
}
fn sealed() -> (RootGrant,Files) {
    let g = grant(); let mut f = Files::new();
    let run = OriginalRun{repository:g.repository.clone(),event:g.event.clone(),attempt:g.attempt,run_id:g.run_id,job_id:g.job_id,suite_id:g.suite_id,source_head:g.source_head.clone(),source_tree:g.source_tree.clone()};
    let root = serde_json::to_vec(&g).unwrap(); let root_hash = receiver::hash(&root);
    f.insert("root-authorization.json".into(),root); f.insert("calibration.json".into(),b"synthetic plan".to_vec()); f.insert("workflow.yml".into(),b"synthetic workflow".to_vec());
    let mut a = json!({"schemaVersion":1,"scope":"PR174_RUST_CALIBRATION_ADMITTED","run":run,"nonce":"e".repeat(64),"containerId":"f".repeat(64),"bootId":"synthetic-boot","daemonId":"synthetic-daemon","image":g.image,"imageId":g.image_id,"workflowSha256":g.workflow_sha256,"rootAuthorizationSha256":root_hash,"calibrationSha256":receiver::hash(b"synthetic plan"),"contractSha256":g.contract_sha256,"hostBinarySha256":g.host_binary_sha256,"adapterSha256":g.adapter_sha256});
    a["tools"] = json!({});
    for tool in &g.tools {a["tools"][&tool.role] = json!(tool.path);a[&tool.role] = json!(tool.path);}
    put(&mut f,"admission.json",&a); put(&mut f,"binding.json",&a);
    let b = ExportBinding{schema_version:1,scope:"PR174_PROTECTED_EXPORT".into(),run,nonce:a["nonce"].take().as_str().unwrap().into(),container_id:"f".repeat(64),boot_id:"synthetic-boot".into(),daemon_id:"synthetic-daemon".into(),image:g.image.clone(),image_id:g.image_id.clone(),root_authorization_sha256:root_hash,calibration_sha256:receiver::hash(b"synthetic plan"),workflow_sha256:g.workflow_sha256.clone(),files:f.iter().map(|(path,bytes)|ExportFile{path:path.clone(),bytes:bytes.len() as u64,sha256:receiver::hash(bytes)}).collect()};
    put(&mut f,"export-binding.json",&serde_json::to_value(b).unwrap());
    (g,f)
}
#[test]
fn wrong_run_job_head_nonce_cid_and_independent_hashes_refuse() {
    let (g,f) = sealed(); assert!(verdict::provenance(&f,&g).is_ok());
    for (pointer,value) in [("/run/runId",json!(2)),("/run/jobId",json!(3)),("/run/sourceHead",json!("9".repeat(40))),("/nonce",json!("a".repeat(64))),("/containerId",json!("1".repeat(64))),("/imageId",json!("sha256:wrong")),("/files/0/sha256",json!("2".repeat(64)))] {
        let mut bad = f.clone(); let mut binding = get(&bad,"export-binding.json"); *binding.pointer_mut(pointer).unwrap() = value; put(&mut bad,"export-binding.json",&binding);
        assert!(verdict::provenance(&bad,&g).is_err(),"{pointer}");
    }
    let mut bad = f.clone(); bad.insert("extra".into(),Vec::new()); assert!(verdict::provenance(&bad,&g).is_err());
    let mut bad = f; bad.get_mut("root-authorization.json").unwrap().push(b' '); assert!(verdict::provenance(&bad,&g).is_err());
}
#[test]
fn malformed_tar_zip_and_unsafe_paths_refuse_without_extraction() {
    for bytes in [Vec::new(),vec![0;1024],b"ustar".to_vec(),b"PK\x03\x04".to_vec()] { assert!(read_zip(&bytes).is_err()); }
    for name in ["../escape","/absolute","a/../../b","a\\b","a//b","./a","a\0b","-x"] {
        assert!(!receiver::retrieval::safe_name(name));
    }
}
#[cfg(any(target_os="linux",target_os="macos"))]
#[test]
fn actual_adapter_export_receiver_evidence_replay_smoke_is_nonofficial() {
    let path = std::env::temp_dir().join(format!("ktlsp-receiver-smoke-{}",std::process::id()));
    struct Cleanup(std::path::PathBuf); impl Drop for Cleanup {fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
    let cleanup = Cleanup(path);
    let script = vec![json!({"jsonrpc":"2.0","id":"😀","method":"shutdown"}),json!({"jsonrpc":"2.0","method":"exit"})];
    let command = vec![env!("CARGO_BIN_EXE_record-test-peer").into(),"notices".into()];
    let manifest = record(&script,&command,&cleanup.0,CaptureLimits{session_ms:20000,..CaptureLimits::default()}).unwrap();
    assert_eq!(manifest["complete"],true,"{manifest}");
    let mut f = Files::new();
    for name in ["manifest.json","metadata.json","transcript.json","client-intended.raw","stdout.raw","stderr.raw"] { f.insert(format!("capture/{name}"),std::fs::read(cleanup.0.join(name)).unwrap()); }
    assert!(!receiver::transport(&f,false).unwrap().useful_symbols);
    assert!(receiver::transport(&f,true).is_err());
    let received = std::process::Command::new(env!("CARGO_BIN_EXE_ktlsp-oracle-receive")).arg("--synthetic").arg(&cleanup.0).output().unwrap();
    assert!(received.status.success(),"{}",String::from_utf8_lossy(&received.stderr));
    assert_eq!(serde_json::from_slice::<Value>(&received.stdout).unwrap()["officialSuccess"],false);
    let evidence = Evidence::load(&cleanup.0).unwrap();
    assert_eq!(replay(&evidence,&command,Limits::default()).unwrap()["outcome"],"match");
}

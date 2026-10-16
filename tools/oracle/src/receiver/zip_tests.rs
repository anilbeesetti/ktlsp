//! Fabricated archives only; never original capture evidence.
use super::*;
use std::io::Write;
fn zip(name: &str, content: &[u8], deflate: bool, descriptor: bool) -> Vec<u8> {
    let payload = if deflate {
        let mut e = flate2::write::DeflateEncoder::new(Vec::new(),flate2::Compression::default());
        e.write_all(content).unwrap(); e.finish().unwrap()
    } else {content.to_vec()};
    let method = if deflate {8u16} else {0};
    let flags = if descriptor {8u16} else {0};
    let crc = crc32(content);
    let mut b = Vec::new();
    b.extend_from_slice(b"PK\x03\x04"); b.extend(20u16.to_le_bytes()); b.extend(flags.to_le_bytes()); b.extend(method.to_le_bytes()); b.extend([0;4]);
    for n in [if descriptor {0}else{crc},if descriptor {0}else{payload.len() as u32},if descriptor {0}else{content.len() as u32}] {b.extend(n.to_le_bytes());}
    b.extend((name.len() as u16).to_le_bytes()); b.extend(0u16.to_le_bytes()); b.extend(name.as_bytes()); b.extend(&payload);
    if descriptor {b.extend_from_slice(b"PK\x07\x08");for n in [crc,payload.len() as u32,content.len() as u32] {b.extend(n.to_le_bytes());}}
    let central = b.len();
    b.extend_from_slice(b"PK\x01\x02");b.extend(0x314u16.to_le_bytes());b.extend(20u16.to_le_bytes());b.extend(flags.to_le_bytes());b.extend(method.to_le_bytes());b.extend([0;4]);
    for n in [crc,payload.len() as u32,content.len() as u32] {b.extend(n.to_le_bytes());}
    for n in [name.len() as u16,0,0,0,0] {b.extend(n.to_le_bytes());}
    b.extend((0o100600u32<<16).to_le_bytes());b.extend(0u32.to_le_bytes());b.extend(name.as_bytes());
    let size = b.len()-central;
    b.extend_from_slice(b"PK\x05\x06"); for n in [0u16,0,1,1] {b.extend(n.to_le_bytes());}
    b.extend((size as u32).to_le_bytes());b.extend((central as u32).to_le_bytes());b.extend(0u16.to_le_bytes());b
}
#[test]
fn stored_deflate_and_descriptors_are_native_bounded_reads() {
    for deflate in [false,true] {for descriptor in [false,true] {
        let archive = zip("capture/stdout.raw",b"synthetic NON EVIDENCE",deflate,descriptor);
        assert_eq!(read_zip(&archive).unwrap()["capture/stdout.raw"],b"synthetic NON EVIDENCE");
    }}
    assert_eq!(crc32(b"123456789"),0xcbf43926);
}
#[test]
fn paths_links_crc_bounds_flags_local_mismatch_and_truncation_refuse() {
    for name in ["../outside","/absolute","a//b","a/./b","a/../b","a\\b","-option"] {assert!(read_zip(&zip(name,b"x",false,false)).is_err(),"{name}");}
    let good = zip("capture/stdout.raw",b"xyz",false,false);
    let end = good.len()-22;
    let central = u32le(&good,end+16).unwrap();
    for (offset,bytes) in [(central+38,(0o120777u32<<16).to_le_bytes().to_vec()),(central+24,((FILE_LIMIT+1) as u32).to_le_bytes().to_vec()),(central+8,1u16.to_le_bytes().to_vec()),(central+16,0u32.to_le_bytes().to_vec()),(14,0u32.to_le_bytes().to_vec()),(30,b"X".to_vec())] {
        let mut bad = good.clone();bad[offset..offset+bytes.len()].copy_from_slice(&bytes);assert!(read_zip(&bad).is_err(),"offset {offset}");
    }
    for len in 0..good.len() {assert!(read_zip(&good[..len]).is_err(),"length {len}");}
    let mut bad = zip("capture/stdout.raw",&vec![b'x';4096],true,false);
    let central = u32le(&bad,bad.len()-6).unwrap();
    bad[central+24..central+28].copy_from_slice(&1u32.to_le_bytes());bad[22..26].copy_from_slice(&1u32.to_le_bytes());
    assert!(read_zip(&bad).is_err());
}

#[test]
fn authenticated_api_join_requires_original_job_attempt_head_tree_workflow_and_artifact() {
    use serde_json::json;
    let mut grant = json!({"schemaVersion":1,"scope":"PR174_RUST_CALIBRATION","repository":"anilbeesetti/ktlsp","event":"workflow_dispatch","attempt":1,"runId":11,"jobId":12,"suiteId":13,"sourceHead":"1".repeat(40),"sourceTree":"2".repeat(40),"workflowPath":crate::authority::WORKFLOW_PATH,"contractSha256":crate::authority::CONTRACT_SHA256,"image":format!("example@sha256:{}","a".repeat(64)),"imageId":format!("sha256:{}","b".repeat(64)),"tools":[],"requiredCalibrationSha256":null,"calibrationCase":"normal","calibrationStep":null,"host":{"osReleaseSha256":"c".repeat(64),"kernelReleaseSha256":"d".repeat(64),"architecture":"x86_64"}});
    for key in ["workflowSha256","hostBinarySha256","adapterSha256","peerSha256","lockSha256","toolchainSha256","containmentTemplateSha256"] {grant[key] = json!("e".repeat(64));}
    let g: RootGrant = serde_json::from_value(grant).unwrap();
    let run = json!({"id":11,"run_attempt":1,"head_sha":g.source_head,"check_suite_id":13,"event":"workflow_dispatch","repository":{"id":15,"full_name":g.repository},"head_repository":{"id":15,"full_name":g.repository},"status":"completed","conclusion":"success","path":g.workflow_path,"workflow_id":14});
    let job = json!({"id":12,"run_id":11,"run_attempt":1,"head_sha":g.source_head,"status":"completed","conclusion":"success"});
    let jobs = json!({"total_count":1,"jobs":[job]});
    let artifact = json!({"id":16,"expired":false,"size_in_bytes":1024,"workflow_run":{"id":11,"head_sha":g.source_head,"repository_id":15}});
    let workflow = json!({"id":14,"path":g.workflow_path});
    let commit = json!({"sha":g.source_head,"tree":{"sha":g.source_tree}});
    assert!(validate_facts(&g,16,&run,&run,&job,&jobs,&artifact,&workflow,&commit).is_ok());
    for (field,value) in [("id",json!(99)),("run_attempt",json!(2)),("head_sha",json!("f".repeat(40))),("check_suite_id",json!(99)),("workflow_id",json!(99)),("conclusion",json!("cancelled"))] {
        let mut bad = run.clone();bad[field]=value;
        assert!(validate_facts(&g,16,&bad,&run,&job,&jobs,&artifact,&workflow,&commit).is_err(),"{field}");
    }
    let mut bad = job.clone();bad["id"]=json!(99);
    assert!(validate_facts(&g,16,&run,&run,&bad,&jobs,&artifact,&workflow,&commit).is_err());
    let mut bad = jobs.clone();bad["total_count"]=json!(101);
    assert!(validate_facts(&g,16,&run,&run,&job,&bad,&artifact,&workflow,&commit).is_err());
    let mut bad = artifact.clone();bad["workflow_run"]["head_sha"]=json!("f".repeat(40));
    assert!(validate_facts(&g,16,&run,&run,&job,&jobs,&bad,&workflow,&commit).is_err());
}

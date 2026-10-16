//! Fixed GitHub API endpoints and non-extracting ZIP reads; no server-provided commands.
use super::{BUNDLE_LIMIT, FILE_COUNT, FILE_LIMIT, Files, Result, hash, json_bytes, require};
use crate::authority::RootGrant;
use serde_json::Value;
use std::{fs, io::Read, path::Path, process::{Command, Stdio}, time::{Duration, Instant}};
#[cfg(unix)]
use std::os::{fd::OwnedFd, unix::{fs::MetadataExt, net::UnixStream}};

pub struct AuthenticatedArtifact {
    pub(super) files: Files,
    pub(super) artifact_id: u64,
    pub(super) archive_sha256: String,
    pub(super) timeout_completed_unix_ms: Option<u64>,
    pub(super) admission_sha256: String,
}

#[cfg(unix)]
fn trusted_tool(path: &str) -> Result<()> {
    for p in Path::new(path).ancestors() {
        let m = fs::symlink_metadata(p).map_err(|e| e.to_string())?;
        require(!m.file_type().is_symlink() && m.uid() == 0 && m.mode() & 0o022 == 0, "tool custody")?;
    }
    require(fs::metadata(path).map_err(|e| e.to_string())?.is_file(), "tool not a file")
}
#[cfg(unix)]
fn command(program: &str, args: &[&str], cap: usize) -> Result<Vec<u8>> {
    trusted_tool(program)?;
    let (mut output, child_output) = UnixStream::pair().map_err(|e| e.to_string())?;
    output.set_nonblocking(true).map_err(|e| e.to_string())?;
    let mut c = Command::new(program);
    c.args(args).env_clear().env("PATH","/usr/bin:/bin").env("LC_ALL","C");
    if program == "/usr/bin/gh" {
        let token = std::env::var_os("GH_TOKEN").ok_or("GH_TOKEN required; never read artifact-provided credentials")?;
        c.env("GH_TOKEN", token).env("GH_HOST","github.com").env("GH_PROMPT_DISABLED","1");
    }
    let mut child = c.stdin(Stdio::null()).stdout(Stdio::from(OwnedFd::from(child_output))).stderr(Stdio::null()).spawn().map_err(|e| e.to_string())?;
    drop(c);
    let end = Instant::now()+Duration::from_secs(30);
    let mut bytes = Vec::new();
    let mut eof = false;
    loop {
        let mut buffer = [0u8;8192];
        match output.read(&mut buffer) {
            Ok(0) => eof = true,
            Ok(n) if n <= cap.saturating_sub(bytes.len()) => bytes.extend_from_slice(&buffer[..n]),
            Ok(_) => { let _ = child.kill(); let _ = child.wait(); return Err("retrieval output limit".into()); }
            Err(e) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::Interrupted) => (),
            Err(e) => { let _ = child.kill(); let _ = child.wait(); return Err(e.to_string()); }
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? && eof {
            require(status.success(),"fixed retrieval command failed")?; return Ok(bytes);
        }
        if Instant::now() >= end { let _ = child.kill(); let _ = child.wait(); return Err("retrieval deadline/EOF missing".into()); }
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[cfg(not(unix))]
fn command(_: &str, _: &[&str], _: usize) -> Result<Vec<u8>> { Err("trusted fixed retrieval requires Unix".into()) }
fn api(endpoint: &str, cap: usize) -> Result<Vec<u8>> {
    command("/usr/bin/gh", &["api","--hostname","github.com","--method","GET","-H","Accept: application/vnd.github+json","-H","X-GitHub-Api-Version: 2026-03-10",endpoint],cap)
}
fn api_json(endpoint: &str) -> Result<Value> { json_bytes(&api(endpoint,FILE_LIMIT)?) }

/// The job API and attempt-specific job membership are independently authenticated;
/// artifact names and uploader-supplied job/attempt metadata are never authority.
pub fn retrieve(admission: &super::ReceiverAdmission, artifact_id: u64) -> Result<AuthenticatedArtifact> {
    admission.validate()?;
    let expected = &admission.grant;
    require(artifact_id > 0, "artifact ID required")?;
    let repo = "repos/anilbeesetti/ktlsp";
    let run = api_json(&format!("{repo}/actions/runs/{}",expected.run_id))?;
    let attempt = api_json(&format!("{repo}/actions/runs/{}/attempts/{}",expected.run_id,expected.attempt))?;
    let job = api_json(&format!("{repo}/actions/jobs/{}",expected.job_id))?;
    let jobs = api_json(&format!("{repo}/actions/runs/{}/attempts/{}/jobs?per_page=100",expected.run_id,expected.attempt))?;
    let artifact = api_json(&format!("{repo}/actions/artifacts/{artifact_id}"))?;
    let workflow = api_json(&format!("{repo}/actions/workflows/{}",run["workflow_id"].as_u64().ok_or("run missing workflow ID")?))?;
    let commit = api_json(&format!("{repo}/git/commits/{}",expected.source_head))?;
    validate_facts(expected,artifact_id,&run,&attempt,&job,&jobs,&artifact,&workflow,&commit)?;
    let workflow_bytes = command("/usr/bin/gh", &["api","--hostname","github.com","--method","GET","-H","Accept: application/vnd.github.raw+json",&format!("{repo}/contents/{}?ref={}",expected.workflow_path,expected.source_head)],FILE_LIMIT)?;
    require(hash(&workflow_bytes) == expected.workflow_sha256,"authenticated workflow bytes mismatch")?;
    let zip = api(&format!("{repo}/actions/artifacts/{artifact_id}/zip"),BUNDLE_LIMIT)?;
    let digest = hash(&zip);
    require(artifact["digest"] == format!("sha256:{digest}"), "artifact digest missing/mismatch")?;
    require(artifact["size_in_bytes"].as_u64() == Some(zip.len() as u64),"artifact original archive size mismatch")?;
    let files = read_zip(&zip)?;
    let logs = api(&format!("{repo}/actions/jobs/{}/logs",expected.job_id),FILE_LIMIT)?;
    let (position,_) = super::custody::step(&job,&admission.uploader_step.name,admission.uploader_step.number)?;
    let step_logs = api(&format!("{repo}/actions/jobs/{}/steps/{position}/logs",expected.job_id),FILE_LIMIT)?;
    super::custody::upload(admission,artifact_id,artifact["digest"].as_str().ok_or("artifact digest missing")?,&digest,&files,&job,&logs,&step_logs)?;
    let timeout_completed_unix_ms = if let Some(step) = &expected.calibration_step {
        let (position,_) = super::custody::step(&job,&step.name,step.number)?;
        let step_logs = api(&format!("{repo}/actions/jobs/{}/steps/{position}/logs",expected.job_id),FILE_LIMIT)?;
        Some(super::custody::timeout(&job,&step.name,step.number,&logs,&step_logs)?)
    } else {None};
    Ok(AuthenticatedArtifact {files,artifact_id,archive_sha256:digest,timeout_completed_unix_ms,admission_sha256:hash(&serde_json::to_vec(admission).map_err(|e|e.to_string())?)})
}
#[allow(clippy::too_many_arguments)]
fn validate_facts(g: &RootGrant, artifact_id: u64, run: &Value, attempt: &Value, job: &Value, jobs: &Value, artifact: &Value, workflow: &Value, commit: &Value) -> Result<()> {
    for r in [run,attempt] {
        require(r["id"] == g.run_id && r["run_attempt"] == g.attempt && r["head_sha"] == g.source_head && r["check_suite_id"] == g.suite_id && r["event"] == g.event && r["repository"]["full_name"] == g.repository && r["status"] == "completed" && r["conclusion"] == "success" && r["path"] == g.workflow_path, "original run/attempt/head/suite/workflow mismatch")?;
        require(r["repository"]["id"].as_u64().is_some_and(|n|n>0) && r["head_repository"]["id"] == r["repository"]["id"] && r["head_repository"]["full_name"] == g.repository && r["workflow_id"].as_u64().is_some_and(|n|n>0),"missing/foreign original repository/workflow identity")?;
    }
    require(job["id"] == g.job_id && job["run_id"] == g.run_id && job["run_attempt"] == g.attempt && job["head_sha"] == g.source_head && job["status"] == "completed" && job["conclusion"] == "success", "original job mismatch")?;
    let list = jobs["jobs"].as_array().ok_or("missing attempt jobs")?;
    require(jobs["total_count"].as_u64() == Some(list.len() as u64) && list.len() <= 100 && list.iter().filter(|j| j["id"] == g.job_id).count() == 1 && list.iter().any(|j| j == job), "incomplete/unmatched original attempt jobs")?;
    require(artifact["id"] == artifact_id && artifact["expired"] == false && artifact["size_in_bytes"].as_u64().is_some_and(|n| n > 0 && n <= BUNDLE_LIMIT as u64) && artifact["workflow_run"]["id"] == g.run_id && artifact["workflow_run"]["head_sha"] == g.source_head && artifact["workflow_run"]["repository_id"] == run["repository"]["id"], "original artifact binding mismatch")?;
    require(workflow["id"] == run["workflow_id"] && workflow["path"] == g.workflow_path && commit["sha"] == g.source_head && commit["tree"]["sha"] == g.source_tree, "authenticated workflow/tree mismatch")
}

fn u16le(b: &[u8], n: usize) -> Result<usize> { Ok(u16::from_le_bytes(b.get(n..n.checked_add(2).ok_or("ZIP offset overflow")?).ok_or("truncated ZIP")?.try_into().map_err(|_|"ZIP integer")?) as usize) }
fn u32le(b: &[u8], n: usize) -> Result<usize> { Ok(u32::from_le_bytes(b.get(n..n.checked_add(4).ok_or("ZIP offset overflow")?).ok_or("truncated ZIP")?.try_into().map_err(|_|"ZIP integer")?) as usize) }
pub fn safe_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 160 && !name.starts_with('/') && name.split('/').all(|s| !s.is_empty() && !s.starts_with('-') && s != "." && s != "..") && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b))
}
/// Only ZIP is accepted. Tar, ZIP64, encrypted, linked, overlapping or oversized input refuses.
/// Native bounded stored/deflate decoding retains bytes only; no filesystem extraction.
pub fn read_zip(zip: &[u8]) -> Result<Files> {
    require(zip.len() >= 22 && zip.len() <= BUNDLE_LIMIT,"ZIP archive byte limit")?;
    let eocd = zip.len()-22;
    require(zip.get(eocd..eocd+4) == Some(b"PK\x05\x06") && u16le(zip,eocd+4)? == 0 && u16le(zip,eocd+6)? == 0 && u16le(zip,eocd+8)? == u16le(zip,eocd+10)? && u16le(zip,eocd+20)? == 0,"ZIP footer/multidisk/comment refusal")?;
    let count = u16le(zip,eocd+10)?;
    let central_size = u32le(zip,eocd+12)?;
    let mut pos = u32le(zip,eocd+16)?;
    let central = pos;
    require(count > 0 && count <= FILE_COUNT && pos.checked_add(central_size) == Some(eocd),"ZIP count/central bounds")?;
    let mut entries = BTreeMap::new();
    let mut spans = Vec::new();
    let mut total = 0usize;
    for _ in 0..count {
        require(zip.get(pos..pos+4) == Some(b"PK\x01\x02"),"ZIP central signature")?;
        let flags = u16le(zip,pos+8)?;
        let method = u16le(zip,pos+10)?;
        let compressed = u32le(zip,pos+20)?;
        let bytes = u32le(zip,pos+24)?;
        let name_len = u16le(zip,pos+28)?;
        let extra = u16le(zip,pos+30)?;
        let comment = u16le(zip,pos+32)?;
        let mode = u32le(zip,pos+38)? >> 16;
        let local = u32le(zip,pos+42)?;
        require(local < central,"ZIP local offset outside bounded payload")?;
        let next = pos.checked_add(46+name_len+extra+comment).ok_or("ZIP offset overflow")?;
        let name = std::str::from_utf8(zip.get(pos+46..pos+46+name_len).ok_or("ZIP name bounds")?).map_err(|_|"ZIP name UTF8")?;
        require(u16le(zip,pos+6)? <= 20 && u32le(zip,pos+38)? & 0x10 == 0,"ZIP64/version/directory refusal")?;
        zip_extra(zip.get(pos+46+name_len..pos+46+name_len+extra).ok_or("ZIP extra bounds")?)?;
        require(next <= eocd && safe_name(name) && flags & !0x808 == 0 && [0,8].contains(&method) && (mode & 0o170000 == 0 || mode & 0o170000 == 0o100000) && u16le(zip,pos+34)? == 0 && bytes <= FILE_LIMIT && compressed <= BUNDLE_LIMIT,"ZIP unsafe path/type/flags/size")?;
        total = total.checked_add(bytes).ok_or("ZIP total overflow")?;
        require(total <= BUNDLE_LIMIT && !entries.contains_key(name),"ZIP aggregate/duplicate path")?;
        require(zip.get(local..local+4) == Some(b"PK\x03\x04") && u16le(zip,local+6)? == flags && u16le(zip,local+8)? == method && u16le(zip,local+26)? == name_len,"ZIP local header mismatch")?;
        require(zip.get(local+30..local+30+name_len) == Some(name.as_bytes()),"ZIP local name mismatch")?;
        require(u16le(zip,local+4)? <= 20,"ZIP local version refusal")?;
        let local_extra = u16le(zip,local+28)?;
        zip_extra(zip.get(local+30+name_len..local+30+name_len+local_extra).ok_or("ZIP local extra bounds")?)?;
        let data = local.checked_add(30+name_len+u16le(zip,local+28)?).ok_or("ZIP local overflow")?;
        let end = data.checked_add(compressed).ok_or("ZIP data overflow")?;
        require(end <= central,"ZIP payload bounds")?;
        let crc = u32le(zip,pos+16)? as u32;
        let mut section_end = end;
        if flags & 8 == 0 {
            require(u32le(zip,local+18)? == compressed && u32le(zip,local+22)? == bytes && u32le(zip,local+14)? == crc as usize,"ZIP local size/CRC mismatch")?;
        } else {
            let descriptor = if zip.get(end..end+4) == Some(b"PK\x07\x08") {end+4} else {end};
            require(u32le(zip,descriptor)? == crc as usize && u32le(zip,descriptor+4)? == compressed && u32le(zip,descriptor+8)? == bytes,"ZIP descriptor mismatch")?;
            section_end = descriptor+12;
            require(section_end <= central,"ZIP descriptor bounds")?;
        }
        entries.insert(name.to_owned(),(bytes,method,crc,data,end));
        spans.push((local,section_end));
        pos = next;
    }
    spans.sort_unstable();
    require(pos == eocd && spans.first().is_some_and(|s|s.0 == 0) && spans.last().is_some_and(|s|s.1 == central) && spans.windows(2).all(|s| s[0].1 == s[1].0),"ZIP overlapping/hidden sections")?;
    let mut files = Files::new();
    for (name,(size,method,crc,start,end)) in entries {
        let payload = &zip[start..end];
        let bytes = if method == 0 {
            require(payload.len() == size,"stored ZIP size mismatch")?;
            payload.to_vec()
        } else {
            let mut decoder = flate2::read::DeflateDecoder::new(payload);
            let mut decoded = Vec::new();
            (&mut decoder).take(size as u64+1).read_to_end(&mut decoded).map_err(|e|e.to_string())?;
            require(decoded.len() == size && decoder.total_in() == payload.len() as u64,"ZIP deflate size/tail mismatch")?;
            decoded
        };
        require(crc32(&bytes) == crc,"ZIP CRC mismatch")?;
        files.insert(name,bytes);
    }
    Ok(files)
}

use std::collections::BTreeMap;

fn crc32(bytes: &[u8]) -> u32 {
    const TABLE: [u32;256] = {
        let mut table = [0;256]; let mut i = 0;
        while i < 256 {
            let mut x = i as u32; let mut bit = 0;
            while bit < 8 { x = (x >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(x & 1)); bit += 1; }
            table[i] = x; i += 1;
        }
        table
    };
    !bytes.iter().fold(!0u32, |crc,b| (crc >> 8) ^ TABLE[((crc ^ *b as u32) & 255) as usize])
}


fn zip_extra(extra: &[u8]) -> Result<()> {
    let mut pos = 0;
    while pos < extra.len() {
        let kind = u16le(extra,pos)?;
        let bytes = u16le(extra,pos+2)?;
        require(kind != 1,"ZIP64 extra refusal")?;
        pos = pos.checked_add(4+bytes).ok_or("ZIP extra overflow")?;
        require(pos <= extra.len(),"ZIP extra field truncated")?;
    }
    Ok(())
}
#[cfg(test)]
#[path = "zip_tests.rs"]
mod zip_tests;

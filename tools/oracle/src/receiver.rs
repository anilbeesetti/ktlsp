//! Independent optional receiver. Synthetic mechanics are never official evidence.
use crate::Evidence;
use ktlsp_protocol::read_frame;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Cursor};

#[path = "receiver/retrieval.rs"]
pub mod retrieval;
#[path = "receiver/verdict.rs"]
pub mod verdict;
#[path = "receiver/custody.rs"]
mod custody;

pub type Result<T, E = String> = std::result::Result<T, E>;
pub type Files = BTreeMap<String, Vec<u8>>;
pub const FILE_LIMIT: usize = 8 * 1024 * 1024;
pub const BUNDLE_LIMIT: usize = 32 * 1024 * 1024;
pub const FILE_COUNT: usize = 48;
pub const ARCHIVE_SHA256: &str = "ab8ca4455dc2fc5fe1a24db2bccc46c104254d2c465155c4251ee65df8f3f7cc";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UploaderStep { pub name: String, pub number: u32 }
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReceiverAdmission {
    pub schema_version: u8,
    pub scope: String,
    pub grant: crate::authority::RootGrant,
    pub uploader_step: UploaderStep,
}
impl ReceiverAdmission {
    pub fn validate(&self) -> Result<()> {
        self.grant.validate_shape()?;
        require(self.schema_version == 1 && self.scope == "PR174_RECEIVER_ADMISSION" && self.uploader_step.number > 0 && !self.uploader_step.name.trim().is_empty() && self.uploader_step.name.len() <= 256 && !self.uploader_step.name.chars().any(char::is_control),"independent ROOT receiver/uploader-step admission malformed")
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProtectedUploadReceipt {
    pub schema_version: u8, pub scope: String, pub repository: String,
    pub run_id: u64, pub job_id: u64, pub suite_id: u64, pub attempt: u8,
    pub source_head: String, pub source_tree: String, pub workflow_sha256: String,
    pub artifact_id: u64, pub artifact_api_digest: String,
    pub download_zip_sha256: Option<String>, pub export_binding_sha256: String,
    pub nonce: String, pub container_id: String, pub root_grant_sha256: String,
    pub uploader_step: UploaderStep,
}
pub const UPLOAD_PREFIX: &str = "KTLSP_PR174_PROTECTED_UPLOAD ";

/// Validate original step-timeout data and return the API completion time in Unix ms.
/// The caller must independently authenticate original job and exact step-log retrieval;
/// passing matching caller-authored JSON/logs is not GitHub provenance or ROOT authority.
/// A genuine continued timeout has outcome=failure but final/API conclusion=success.
pub fn validate_step_timeout(
    grant: &crate::authority::RootGrant,
    job: &Value,
    job_logs: &[u8],
    step_logs: &[u8],
) -> Result<u64> {
    grant.validate_shape()?;
    require(grant.scope == crate::authority::GrantScope::RustCalibration && grant.calibration_case == Some(crate::authority::CalibrationCase::WorkflowStepCancel),"timeout helper requires the independently admitted finite cancellation calibration grant")?;
    require(job["id"] == grant.job_id && job["run_id"] == grant.run_id && job["run_attempt"] == grant.attempt && job["head_sha"] == grant.source_head,"timeout data is not the original admitted job/attempt/head")?;
    require((job["status"] == "in_progress" && job.get("conclusion") == Some(&Value::Null)) || (job["status"] == "completed" && job["conclusion"] == "success"),"whole-job cancellation/failure or absent job state is not step-timeout calibration")?;
    let step = grant.calibration_step.as_ref().ok_or("independent original cancellation-step identity missing")?;
    custody::timeout(job,&step.name,step.number,job_logs,step_logs)
}

pub fn hash(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
pub fn require(ok: bool, reason: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(reason.into()) }
}
pub fn file<'a>(files: &'a Files, name: &str) -> Result<&'a [u8]> {
    files.get(name).map(Vec::as_slice).ok_or_else(|| format!("missing {name}"))
}

// serde_json::Value normally silently discards duplicate keys; receipts must not.
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Unique;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("unique-key JSON") }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> std::result::Result<Unique,E> { Ok(Unique(v.into())) }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Unique,E> { Ok(Unique(v.into())) }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Unique,E> { Ok(Unique(v.into())) }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Unique,E> { Ok(Unique(json!(v))) }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<Unique,E> { Ok(Unique(v.into())) }
            fn visit_none<E: serde::de::Error>(self) -> std::result::Result<Unique,E> { Ok(Unique(Value::Null)) }
            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Unique,E> { Ok(Unique(Value::Null)) }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut a: A) -> std::result::Result<Unique,A::Error> {
                let mut v = Vec::new();
                while let Some(Unique(x)) = a.next_element()? { v.push(x); }
                Ok(Unique(Value::Array(v)))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut a: A) -> std::result::Result<Unique,A::Error> {
                let mut v = serde_json::Map::new();
                while let Some((k, Unique(x))) = a.next_entry::<String,Unique>()? {
                    if v.insert(k,x).is_some() { return Err(serde::de::Error::custom("duplicate JSON key")); }
                }
                Ok(Unique(Value::Object(v)))
            }
        }
        d.deserialize_any(Visitor)
    }
}
pub fn json_bytes(bytes: &[u8]) -> Result<Value> {
    require(bytes.len() <= FILE_LIMIT, "JSON byte limit")?;
    serde_json::from_slice::<Unique>(bytes).map(|v| v.0).map_err(|e| e.to_string())
}
fn number(v: &Value) -> Result<usize> {
    v.as_u64().and_then(|n| usize::try_from(n).ok()).ok_or_else(|| "missing/invalid unsigned count".into())
}
fn array(v: &Value) -> Result<&Vec<Value>> { v.as_array().ok_or_else(|| "missing array".into()) }

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transport {
    pub frames: usize,
    pub useful_symbols: bool,
    // OS acceptance proves a write, not that the original peer consumed it.
    pub peer_consumption_claimed: bool,
}

/// Verify exported chronology against complete retained raw bytes, then reuse replay validation.
pub fn transport(files: &Files, official: bool) -> Result<Transport> {
    require(files.len() <= FILE_COUNT && files.values().try_fold(0usize, |n,b| n.checked_add(b.len())).is_some_and(|n| n <= BUNDLE_LIMIT), "bundle work limit")?;
    require(files.values().all(|b| b.len() <= FILE_LIMIT), "file byte limit")?;
    let manifest = json_bytes(file(files,"capture/manifest.json")?)?;
    let transcript_bytes = file(files,"capture/transcript.json")?;
    let metadata_bytes = file(files,"capture/metadata.json")?;
    let metadata = json_bytes(metadata_bytes)?;
    let records = json_bytes(transcript_bytes)?;
    Evidence::parse(transcript_bytes,metadata_bytes)?;
    require(manifest["schemaVersion"] == 1 && manifest["complete"] == true && manifest["scriptCompleted"] == true && manifest.get("failureReason") == Some(&Value::Null), "incomplete capture")?;
    require(manifest["export"]["complete"] == true && manifest["export"].get("failureReason") == Some(&Value::Null) && number(&manifest["export"]["bytes"])? == transcript_bytes.len(), "incomplete export")?;
    let records = array(&records)?;
    let chronology = array(&manifest["chronology"])?;
    require(!records.is_empty() && records.len() <= 1024 && records.len() == chronology.len() && number(&manifest["export"]["records"])? == records.len(), "chronology count")?;
    let mut directional = [Vec::new(),Vec::new()];
    for (reference,record) in chronology.iter().zip(records) {
        let direction = match reference["direction"].as_str() { Some("client") => 0, Some("server") => 1, _ => return Err("unknown chronology direction".into()) };
        require(reference["direction"] == record["direction"] && number(&reference["frameIndex"])? == directional[direction].len(), "chronology direction/index mismatch")?;
        directional[direction].push(&record["message"]);
    }
    for (direction,(raw_name,frame_name)) in [("capture/client-intended.raw","clientFrames"),("capture/stdout.raw","stdoutFrames")].iter().enumerate() {
        let raw = file(files,raw_name)?;
        let frames = array(&manifest[frame_name])?;
        require(frames.len() == directional[direction].len(), "unreferenced frame")?;
        let mut end = 0;
        for (frame,message) in frames.iter().zip(&directional[direction]) {
            let start = number(&frame["start"])?;
            let next = number(&frame["end"])?;
            require(start == end && next > start && next <= raw.len(), "raw offset gap/overlap/bounds")?;
            let mut cursor = Cursor::new(&raw[start..next]);
            let payload = read_frame(&mut cursor).map_err(|e| e.to_string())?.ok_or("missing raw frame")?;
            require(cursor.position() == (next-start) as u64 && json_bytes(&payload)? == **message, "raw frame/message mismatch")?;
            if direction == 0 {
                require(number(&frame["intendedLength"])? == next-start && number(&frame["persistedBytes"])? == next-start && number(&frame["acceptedBytes"])? == next-start, "partial intended/persisted/OS-accepted client frame")?;
            } else { require(frame["message"] == **message, "decoded stdout message mismatch")?; }
            end = next;
        }
        require(end == raw.len(), "unreferenced raw tail")?;
    }
    for (stream,name,cap) in [("stdout","capture/stdout.raw",FILE_LIMIT),("stderr","capture/stderr.raw",64*1024)] {
        let bytes = file(files,name)?;
        let s = &manifest[stream];
        require(bytes.len() <= cap && number(&s["bytes"])? == bytes.len() && number(&s["observedBytes"])? == bytes.len() && s["unpersistedBytes"] == 0 && s["storageFailed"] == false && s["eof"] == true && s["truncated"] == false, "stream incomplete/truncated/unpersisted")?;
    }
    require(manifest["stdout"]["undecodedStart"] == manifest["stdout"]["undecodedEnd"] && number(&manifest["stdout"]["undecodedEnd"])? == file(files,"capture/stdout.raw")?.len(), "undecoded stdout tail")?;
    require(manifest["exit"]["observed"] == true && manifest["exit"]["code"] == 0 && manifest["exit"].get("signal") == Some(&Value::Null) && metadata["returncode"] == 0, "non-normal original adapter child exit")?;
    require(manifest["cleanup"]["forcedTermination"] == false && manifest["cleanup"].get("error") == Some(&Value::Null) && ["childExitObserved","stdoutEof","stderrEof"].iter().all(|k| manifest["cleanup"]["before"][k] == true), "forced/incomplete adapter cleanup")?;
    let limits = &manifest["limits"];
    require(number(&limits["phaseMs"])? <= 60_000 && number(&limits["sessionMs"])? <= 300_000 && number(&limits["cleanupMs"])? <= 60_000 && number(&limits["streamBytes"])? <= FILE_LIMIT && number(&limits["stderrBytes"])? <= 64*1024 && number(&limits["messages"])? <= 1024 && number(&manifest["elapsedMs"])? <= number(&limits["sessionMs"])? + number(&limits["cleanupMs"])?, "capture limit mismatch")?;
    require(["phaseMs","sessionMs","cleanupMs","streamBytes","stderrBytes","messages"].iter().all(|k| limits[k].as_u64().is_some_and(|n|n>0)) && records.len() <= number(&limits["messages"])? && file(files,"capture/stdout.raw")?.len() <= number(&limits["streamBytes"])? && file(files,"capture/client-intended.raw")?.len() <= number(&limits["streamBytes"])? && file(files,"capture/stderr.raw")?.len() <= number(&limits["stderrBytes"])?, "capture exceeds declared bounds")?;
    let useful_symbols = if official {
        let frozen: Vec<Value> = serde_json::from_slice(include_bytes!("../fixtures/official-pr174-symbols/script.json")).map_err(|e|e.to_string())?;
        require(directional[0].iter().copied().eq(frozen.iter()), "frozen client script/UTF16 inputs mismatch")?;
        require(limits == &json!({"phaseMs":60000,"sessionMs":300000,"cleanupMs":5000,"streamBytes":8388608,"stderrBytes":65536,"messages":1024}), "official limits mismatch")?;
        require(metadata["release"] == "kotlin-lsp/v263.6379.0" && metadata["sha256"] == ARCHIVE_SHA256 && metadata["identity"] == manifest["identity"] && metadata["authorization"] == manifest["authorization"], "official metadata mismatch")?;
        symbols(records)?
    } else { false };
    Ok(Transport { frames: records.len(), useful_symbols, peer_consumption_claimed: false })
}

fn valid_range(v: &Value, text: &str) -> Result<ktlsp_text::Range> {
    let range: ktlsp_text::Range = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
    require(range.start <= range.end, "reversed symbol range")?;
    ktlsp_text::position_to_offset(text,range.start).map_err(|e| e.to_string())?;
    ktlsp_text::position_to_offset(text,range.end).map_err(|e| e.to_string())?;
    Ok(range)
}
fn symbol(v: &Value, text: &str, uri: &str, parent: Option<ktlsp_text::Range>, budget: &mut usize, depth: usize, flat: bool) -> Result<()> {
    require(*budget > 0 && depth <= 32, "symbol node/depth limit")?;
    *budget -= 1;
    require(v["name"].as_str().is_some_and(|s| !s.trim().is_empty()) && v["kind"].as_u64().is_some_and(|k| (1..=26).contains(&k)), "invalid symbol name/kind")?;
    if let Some(tags) = v.get("tags") { require(array(tags)?.iter().all(|t| t == 1), "invalid symbol tag")?; }
    if let Some(deprecated) = v.get("deprecated") { require(deprecated.is_boolean(), "invalid deprecated flag")?; }
    if flat {
        require(v["location"]["uri"] == uri && v.get("range").is_none() && v.get("children").is_none() && v.get("selectionRange").is_none(), "invalid SymbolInformation shape/location")?;
        if let Some(name) = v.get("containerName") { require(name.is_string(), "invalid containerName")?; }
        valid_range(&v["location"]["range"],text)?;
    } else {
        require(v.get("location").is_none(), "mixed symbol shapes")?;
        if let Some(detail) = v.get("detail") { require(detail.is_string(), "invalid symbol detail")?; }
        let range = valid_range(&v["range"],text)?;
        let selection = valid_range(&v["selectionRange"],text)?;
        require(range.start <= selection.start && selection.end <= range.end && parent.is_none_or(|p| p.start <= range.start && range.end <= p.end), "symbol/selection/parent containment")?;
        if let Some(children) = v.get("children") {
            for child in array(children)? { symbol(child,text,uri,Some(range),budget,depth+1,false)?; }
        }
    }
    Ok(())
}
fn symbols(records: &[Value]) -> Result<bool> {
    let mut documents = BTreeMap::new();
    let mut requests = BTreeMap::new();
    let mut initialized = false;
    let mut useful = false;
    let mut answers = [false;4];
    let mut budget = 4096;
    for record in records {
        let m = &record["message"];
        if record["direction"] == "client" {
            match m["method"].as_str() {
                Some("textDocument/didOpen") => {
                    let d = &m["params"]["textDocument"];
                    let uri = d["uri"].as_str().ok_or("missing document URI")?;
                    let text = d["text"].as_str().ok_or("missing document text")?;
                    require(documents.insert(uri,text).is_none(), "duplicate document open")?;
                }
                Some("textDocument/documentSymbol") => {
                    let id = m["id"].as_u64().ok_or("symbol ID must be integer")?;
                    require((2..=3).contains(&id), "unexpected symbol request ID")?;
                    let uri = m["params"]["textDocument"]["uri"].as_str().ok_or("missing symbol URI")?;
                    let text = documents.get(uri).ok_or("symbol request before document open")?;
                    requests.insert(id,(uri,*text));
                }
                Some("textDocument/didChange"|"textDocument/didClose") => return Err("frozen capture cannot mutate/close symbol document".into()),
                Some("initialize") => require(m["id"] == 1, "initialize ID mismatch")?,
                Some("shutdown") => require(m["id"] == 4, "shutdown ID mismatch")?,
                _ => (),
            }
        } else if m.get("method").is_none() {
            let id = m["id"].as_u64().ok_or("official response ID must be integer")?;
            require((1..=4).contains(&id) && m.get("error").is_none() && m.get("result").is_some(), "official error/missing response result")?;
            answers[id as usize-1] = true;
            if id == 1 {
                let r = &m["result"];
                require(r["serverInfo"]["version"] == "ILS-263.6379.0" && (r["capabilities"]["documentSymbolProvider"] == true || r["capabilities"]["documentSymbolProvider"].is_object()) && r["capabilities"].get("positionEncoding").is_none_or(|v| v == "utf-16"), "version/symbol capability/UTF16 mismatch")?;
                initialized = true;
            } else if id == 4 { require(m["result"].is_null(), "shutdown must return genuine null")?; }
            else {
                require(initialized, "symbols before initialization")?;
                let (uri,text) = requests.get(&id).ok_or("unbound symbol response")?;
                if !m["result"].is_null() {
                    let result = array(&m["result"])?;
                    let flat = result.first().is_some_and(|v| v.get("location").is_some());
                    for s in result { symbol(s,text,uri,None,&mut budget,0,flat)?; }
                    useful |= !result.is_empty();
                }
            }
        }
    }
    require(answers == [true;4], "missing frozen response IDs1/2/3/4")?;
    Ok(useful)
}

//! Original authenticated job AND exact step logs bind protected post-upload custody.
use super::{Files, ProtectedUploadReceipt, ReceiverAdmission, Result, UPLOAD_PREFIX, file, hash, json_bytes, require};
use serde_json::Value;

pub(super) fn step<'a>(job: &'a Value,name: &str,number: u32) -> Result<(usize,&'a Value)> {
    let steps = job["steps"].as_array().ok_or("original job steps missing")?;
    require(steps.len() <= 256 && steps.windows(2).all(|p|p[0]["number"].as_u64().zip(p[1]["number"].as_u64()).is_some_and(|(a,b)|a<b)),"original steps incomplete/duplicate/unsorted")?;
    let mut matches = steps.iter().enumerate().filter(|(_,s)|s["number"] == number);
    let (index,step) = matches.next().ok_or("original admitted step missing")?;
    require(matches.next().is_none(),"original admitted step ambiguous")?;
    require(step["name"] == name && step["status"] == "completed" && step["conclusion"] == "success","original admitted step name/status/conclusion mismatch")?;
    let start = utc_second(step["started_at"].as_str().ok_or("step start missing")?)?;
    let end = utc_second(step["completed_at"].as_str().ok_or("step end missing")?)?;
    require(start <= end,"original step time range reversed")?;
    Ok((index,step))
}
fn utc_second(s: &str) -> Result<&str> {
    let b=s.as_bytes();
    require(b.len()>=20 && b.last()==Some(&b'Z') && b.get(4)==Some(&b'-') && b.get(7)==Some(&b'-') && b.get(10)==Some(&b'T') && b.get(13)==Some(&b':') && b.get(16)==Some(&b':') && b[..19].iter().enumerate().all(|(i,b)|[4,7,10,13,16].contains(&i)||b.is_ascii_digit()) && (b.len()==20 || (b.get(19)==Some(&b'.') && b.len()<=30 && b[20..b.len()-1].iter().all(u8::is_ascii_digit))),"invalid original UTC timestamp")?;
    Ok(&s[..19])
}
fn messages<'a>(logs: &'a [u8],step: &Value,prefix: &str,artifact_id: Option<u64>) -> Result<(&'a str,&'a str)> {
    require(!logs.is_empty() && logs.len() <= super::FILE_LIMIT && logs.ends_with(b"\n"),"original logs absent/truncated/oversized")?;
    let logs=std::str::from_utf8(logs).map_err(|_|"original log UTF8")?;
    let start=utc_second(step["started_at"].as_str().ok_or("step start missing")?)?;
    let end=utc_second(step["completed_at"].as_str().ok_or("step end missing")?)?;
    let mut found = None;
    for line in logs.lines() {
        let Some((timestamp,message))=line.split_once(' ') else {continue;};
        if let Some(body)=message.strip_prefix(prefix) {
            require(body.len()<=16384,"protected receipt byte limit")?;
            if let Some(id)=artifact_id {
                let receipt: ProtectedUploadReceipt=serde_json::from_value(json_bytes(body.as_bytes())?).map_err(|e|e.to_string())?;
                if receipt.artifact_id!=id {continue;}
            }
            let time=utc_second(timestamp)?;
            require(start<=time && time<=end,"protected receipt outside independently admitted original step time range")?;
            require(found.is_none(),"duplicate original protected log receipt")?;
            found = Some((timestamp,body));
        }
    }
    found.ok_or_else(||"original protected log receipt missing".into())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn upload(admission: &ReceiverAdmission,artifact_id: u64,api_digest: &str,zip_sha256: &str,files: &Files,job: &Value,job_logs: &[u8],step_logs: &[u8]) -> Result<()> {
    let (_,step)=step(job,&admission.uploader_step.name,admission.uploader_step.number)?;
    let original=messages(job_logs,step,UPLOAD_PREFIX,Some(artifact_id))?;
    let selective=messages(step_logs,step,UPLOAD_PREFIX,Some(artifact_id))?;
    require(original==selective,"original job and exact uploader-step custody receipts differ")?;
    let receipt: ProtectedUploadReceipt=serde_json::from_value(json_bytes(original.1.as_bytes())?).map_err(|e|e.to_string())?;
    let g=&admission.grant;
    let binding: crate::authority::ExportBinding=serde_json::from_value(json_bytes(file(files,"export-binding.json")?)?).map_err(|e|e.to_string())?;
    require(receipt.schema_version==1 && receipt.scope=="PR174_PROTECTED_UPLOAD" && receipt.repository==g.repository && receipt.run_id==g.run_id && receipt.job_id==g.job_id && receipt.suite_id==g.suite_id && receipt.attempt==g.attempt && receipt.source_head==g.source_head && receipt.source_tree==g.source_tree && receipt.workflow_sha256==g.workflow_sha256 && receipt.uploader_step==admission.uploader_step,"original protected upload source/job/step mismatch")?;
    require(receipt.artifact_id==artifact_id && receipt.artifact_api_digest==api_digest && receipt.download_zip_sha256.as_deref().is_none_or(|s|s==zip_sha256) && api_digest==format!("sha256:{zip_sha256}"),"original protected upload artifact/API/observed download digest mismatch")?;
    require(receipt.export_binding_sha256==hash(file(files,"export-binding.json")?) && receipt.root_grant_sha256==hash(file(files,"root-authorization.json")?) && receipt.root_grant_sha256==binding.root_authorization_sha256 && receipt.nonce==binding.nonce && receipt.container_id==binding.container_id,"original protected upload binding/ROOT/nonce/CID mismatch")
}
pub(super) fn timeout(job: &Value,name: &str,number: u32,job_logs: &[u8],step_logs: &[u8]) -> Result<u64> {
    let (_,step)=step(job,name,number)?;
    let prefix=format!("##[error]The action '{name}' has timed out after 1 minutes.");
    let original=messages(job_logs,step,&prefix,None)?;
    let selective=messages(step_logs,step,&prefix,None)?;
    require(original.1.is_empty() && selective==original,"exact original runner one-minute step timeout missing")?;
    unix_ms(step["completed_at"].as_str().ok_or("original timeout-step completion missing")?)
}


fn unix_ms(timestamp: &str) -> Result<u64> {
    let s = utc_second(timestamp)?;
    let part = |start,end| s[start..end].parse::<u64>().map_err(|_|"invalid UTC field".to_string());
    let year=part(0,4)?;let month=part(5,7)?;let day=part(8,10)?;
    let hour=part(11,13)?;let minute=part(14,16)?;let second=part(17,19)?;
    let leap=year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let months=[31,if leap{29}else{28},31,30,31,30,31,31,30,31,30,31];
    require((1970..=9999).contains(&year) && (1..=12).contains(&month) && hour<24 && minute<60 && second<60,"invalid original UTC date/time")?;
    require(day>0 && day<=months[month as usize-1],"invalid original UTC calendar day")?;
    let days=(year-1970)*365+((year-1)/4-1969/4)-((year-1)/100-1969/100)+((year-1)/400-1969/400)+months[..month as usize-1].iter().sum::<u64>()+day-1;
    let mut fraction=0;
    if timestamp.len()>20 {
        let digits=&timestamp.as_bytes()[20..timestamp.len()-1];
        require(!digits.is_empty(),"empty UTC fraction")?;
        for i in 0..3 {fraction=fraction*10+u64::from(digits.get(i).copied().unwrap_or(b'0')-b'0');}
    }
    Ok((((days*24+hour)*60+minute)*60+second)*1000+fraction)
}
#[cfg(test)]
#[path = "custody_tests.rs"]
mod custody_tests;

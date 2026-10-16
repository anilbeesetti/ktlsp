fn authenticate_original(dir: &Path, a: &Admission, e: &mut Evidence) -> Result<()> {
    let raw=read_file(&dir.join("github.token"),4096)?;
    let token=std::str::from_utf8(&raw).map_err(|_|"credential encoding")?.trim();
    if token.is_empty() || token.bytes().any(|b|b.is_ascii_control()){return Err("credential input unavailable".into());}
    let prefix="repos/anilbeesetti/ktlsp";
    let run:Value=parse(&gh(e,a,token,format!("{prefix}/actions/runs/{}",a.run.run_id),false)?)?;
    let job:Value=parse(&gh(e,a,token,format!("{prefix}/actions/jobs/{}",a.run.job_id),false)?)?;
    let commit:Value=parse(&gh(e,a,token,format!("{prefix}/git/commits/{}",a.run.source_head),false)?)?;
    api_identity(&run,&job,&commit,&a.run)?;
    let attempt:Value=parse(&gh(e,a,token,format!("{prefix}/actions/runs/{}/attempts/1/jobs?per_page=100",a.run.run_id),false)?)?;
    let jobs=attempt["jobs"].as_array().ok_or("attempt-specific original jobs unavailable")?;
    if attempt["total_count"].as_u64()!=Some(jobs.len() as u64) || jobs.iter().filter(|j|j["id"]==a.run.job_id && j["run_id"]==a.run.run_id && j["head_sha"]==a.run.source_head).count()!=1 || job["run_attempt"]!=1 || job["labels"].as_array().is_some_and(|labels|labels.contains(&json!("self-hosted"))) {return Err("original attempt/job authenticated join incomplete".into());}
    let suite:Value=parse(&gh(e,a,token,format!("{prefix}/check-suites/{}",a.run.suite_id),false)?)?;
    if suite["id"]!=a.run.suite_id || suite["head_sha"]!=a.run.source_head || suite["app"]["slug"]!="github-actions" {return Err("original check suite authenticated join".into());}
    let workflow=gh(e,a,token,format!("{prefix}/contents/.github/workflows/t045-pr174-official-symbols.yml?ref={}",a.run.source_head),true)?;
    if workflow!=read_file(&dir.join("workflow.yml"),INPUT_BYTES)? || sha_bytes(&workflow)!=a.workflow_sha256 {return Err("original API workflow byte seal mismatch".into());}
    Ok(())
}

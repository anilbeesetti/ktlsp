// Original-job producer. GitHub owner approval, not matching caller hashes, creates authority.
use ktlsp_oracle_replay::authority::{CalibrationStep, ToolSeal};
use ktlsp_oracle_replay::receiver::{ProtectedUploadReceipt, UploaderStep, UPLOAD_PREFIX};

const WF_ROOT: &str = "/etc/ktlsp-pr174";
const WF_SOURCE: &str = "/opt/ktlsp-pr174/source";
const WF_BIN: &str = "/opt/ktlsp-pr174/bin";
const WF_IMAGE: &str = "ubuntu@sha256:f610ab94648195aa356059f5b41d6085c9d4d903c072430cdd1af7bdb646106b";
const WF_JOB: &str = "PR174 original protected cohort";
const WF_CANCEL: &str = "PR174 runner timeout calibration";
const WF_INVENTORY: &str = "4fbbe3b1f6a8d8cc98c559c86f629487ed91f5e3daf3cee6f84499d93d7fae26";
const WF_ARCHIVE_URL: &str = "https://download.jetbrains.com/language-server/kotlin-server/263.6379.0/kotlin-server-263.6379.0.tar.gz";

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WorkflowRequest {
    source_sha: String, checkout_tree: String, run_id: u64,
    source_comment: u64, calibration_comment: u64, official_comment: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WorkflowSourceAdmission {
    schema_version: u8, scope: String, run: OriginalRun, workflow_sha256: String,
    contract_sha256: String, image: String, containment_template_sha256: String,
    measured_future_facts: Vec<String>, calibration_comment: u64, official_comment: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WorkflowProposal {
    schema_version: u8, scope: String, source_admission_sha256: String,
    grant: RootGrant, cancellation_step: CalibrationStep,
    boot_id: String, daemon_id: String, image_inspect: Value, daemon_info: Value,
    os_release_hex: String, kernel_release_hex: String, landlock_abi: u32,
    archive_sha256: String, inventory_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WorkflowApproval {
    schema_version: u8, scope: String, run: OriginalRun,
    proposal_sha256: String, proposal_artifact_id: u64, proposal_artifact_api_digest: String,
    #[serde(deserialize_with = "wf_nullable")]
    calibration_sha256: Option<String>,
    #[serde(deserialize_with = "wf_nullable")]
    calibration_artifact_id: Option<u64>,
    #[serde(deserialize_with = "wf_nullable")]
    calibration_artifact_api_digest: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WorkflowArtifact {
    artifact_id: u64, api_digest: String, proposal_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WorkflowCase {
    case: Option<CalibrationCase>, nonce: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WorkflowCalibrationArtifact {
    artifact_id: u64, api_digest: String, calibration_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WorkflowCancellationReady {
    schema_version: u8, scope: String, case: CalibrationCase, nonce: String, container_id: String,
    boot_id: String, daemon_id: String, init_pid: u32, init_starttime: u64,
    supervisor_pid: u32, supervisor_starttime: u64, armed_ms: u64, observed_ms: u64,
}
fn wf_nullable<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}

fn wf_require(ok: bool, reason: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(reason.into()) }
}
fn wf_json<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|e| e.to_string())
}
fn wf_positive(value: &str) -> Result<u64> {
    let number: u64 = value.parse().map_err(|_| "positive numeric original identity required")?;
    wf_require(number > 0 && number.to_string() == value, "canonical positive numeric identity")?;
    Ok(number)
}
fn wf_env(name: &str) -> Result<String> {
    std::env::var(name).map_err(|_| format!("required workflow context {name} unavailable"))
}
fn wf_request_env() -> Result<WorkflowRequest> {
    let source_sha = wf_env("KTLSP_SOURCE_SHA")?;
    wf_require(hex(&source_sha, 40) && wf_env("GITHUB_SHA")? == source_sha
        && wf_env("KTLSP_CHECKOUT_SHA")? == source_sha
        && wf_env("GITHUB_REF")? == "refs/heads/main"
        && wf_env("GITHUB_EVENT_NAME")? == "workflow_dispatch"
        && wf_env("GITHUB_RUN_ATTEMPT")? == "1"
        && wf_env("GITHUB_REPOSITORY")? == "anilbeesetti/ktlsp", "manual original main checkout/context mismatch")?;
    let request = WorkflowRequest {
        source_sha, checkout_tree: wf_env("KTLSP_CHECKOUT_TREE")?,
        run_id: wf_positive(&wf_env("KTLSP_RUN_ID")?)?,
        source_comment: wf_positive(&wf_env("KTLSP_SOURCE_COMMENT")?)?,
        calibration_comment: wf_positive(&wf_env("KTLSP_CALIBRATION_COMMENT")?)?,
        official_comment: wf_positive(&wf_env("KTLSP_OFFICIAL_COMMENT")?)?,
    };
    wf_require(hex(&request.checkout_tree, 40)
        && request.source_comment != request.calibration_comment
        && request.source_comment != request.official_comment
        && request.calibration_comment != request.official_comment, "independent distinct approval comments/source tree required")?;
    Ok(request)
}
fn wf_token_env() -> Result<String> {
    let token = wf_env("GH_TOKEN")?;
    wf_require(!token.is_empty() && token.len() <= 4096 && !token.bytes().any(|b| b.is_ascii_control()), "bounded API credential unavailable")?;
    Ok(token)
}
fn wf_token() -> Result<String> {
    let bytes = read_file(&Path::new(WF_ROOT).join("github.token"), 4096)?;
    let token = String::from_utf8(bytes).map_err(|_| "API credential encoding")?;
    wf_require(!token.is_empty() && !token.bytes().any(|b| b.is_ascii_control()), "API credential unavailable")?;
    Ok(token)
}
// Authentication never records command environment, tokens, API response bodies or child text in job logs.
fn wf_api(token: &str, endpoint: &str, raw: bool, cap: usize) -> Result<Vec<u8>> {
    wf_api_before(token, endpoint, raw, cap, Instant::now() + Duration::from_secs(30))
}
fn wf_gh_command(token: &str) -> Result<Command> {
    protected(Path::new("/usr/bin/gh"), false)?;
    let mut command = Command::new("/usr/bin/gh");
    command.env_clear().env("PATH", "/usr/bin:/bin").env("LANG", "C.UTF-8")
        .env("GH_TOKEN", token).env("GH_HOST", "github.com").env("GH_PROMPT_DISABLED", "1");
    Ok(command)
}
fn wf_api_before(token: &str, endpoint: &str, raw: bool, cap: usize, end: Instant) -> Result<Vec<u8>> {
    let mut command = wf_gh_command(token)?;
    command.args(["api", "--hostname", "github.com", "--method", "GET", "--header",
        if raw { "Accept: application/vnd.github.raw+json" } else { "Accept: application/vnd.github+json" },
        "--header", "X-GitHub-Api-Version: 2026-03-10", endpoint]);
    let output = bounded_command(command, cap, end.min(Instant::now() + Duration::from_secs(30)))?;
    wf_require(output.ok(), "authenticated GitHub API unavailable/incomplete; UNKNOWN, no START")?;
    Ok(output.stdout)
}
fn wf_api_json(token: &str, endpoint: &str) -> Result<Value> {
    parse(&wf_api(token, endpoint, false, 262144)?)
}
fn wf_owner_comment(token: &str, id: u64, end: Instant) -> Result<Vec<u8>> {
    let repo: Value = parse(&wf_api_before(token, "repos/anilbeesetti/ktlsp", false, 262144, end)?)?;
    let comment: Value = parse(&wf_api_before(token, &format!("repos/anilbeesetti/ktlsp/issues/comments/{id}"), false, 262144, end)?)?;
    wf_require(repo["full_name"] == "anilbeesetti/ktlsp" && repo["owner"]["login"] == "anilbeesetti"
        && repo["owner"]["id"].as_u64().is_some_and(|n| n > 0)
        && comment["id"] == id && comment["user"]["id"] == repo["owner"]["id"]
        && comment["user"]["login"] == repo["owner"]["login"]
        && comment["user"]["type"] == "User" && comment["author_association"] == "OWNER",
        "approval must be an independently authenticated repository-owner comment")?;
    let node_id = comment["node_id"].as_str().filter(|id| !id.is_empty() && id.len() <= 256).ok_or("original comment node identity unavailable")?;
    const QUERY: &str = "query($id: ID!) { node(id: $id) { __typename ... on IssueComment { fullDatabaseId body lastEditedAt author { login ... on User { databaseId } } editor { login ... on User { databaseId } } } } }";
    let mut command = wf_gh_command(token)?;
    command.args(["api", "--hostname", "github.com", "--method", "POST", "graphql",
        "-f", &format!("query={QUERY}"), "-f", &format!("id={node_id}")]);
    let output = bounded_command(command, 262144, end.min(Instant::now() + Duration::from_secs(30)))?;
    wf_require(output.ok(), "current approval editor API unavailable; UNKNOWN, no START")?;
    let graph: Value = parse(&output.stdout)?;
    let body = wf_comment_body(&repo["owner"], &comment, &graph)?.as_bytes();
    Ok(body.to_vec())
}
fn wf_comment_body<'a>(owner: &Value, comment: &Value, graph: &'a Value) -> Result<&'a str> {
    let owner_id = owner["id"].as_u64().filter(|id| *id > 0).ok_or("repository owner numeric identity unavailable")?;
    let node = &graph["data"]["node"];
    wf_require(!graph.get("errors").is_some_and(|errors| !errors.is_null())
        && node["__typename"] == "IssueComment"
        && node["fullDatabaseId"].as_u64().or_else(|| node["fullDatabaseId"].as_str().and_then(|id| wf_positive(id).ok()))
            .is_some_and(|id| comment["id"] == id)
        && node["author"]["databaseId"] == owner_id && node["author"]["login"] == owner["login"],
        "current approval body/author identity unavailable")?;
    let untouched = node.get("lastEditedAt").is_some_and(Value::is_null)
        && node.get("editor").is_some_and(Value::is_null);
    let owner_edited = node["lastEditedAt"].as_str().is_some_and(|time| !time.is_empty())
        && node["editor"]["databaseId"] == owner_id && node["editor"]["login"] == owner["login"];
    wf_require(untouched || owner_edited, "current approval body was not authored/edited by the independent repository owner")?;
    let body = node["body"].as_str().ok_or("current owner approval body unavailable")?;
    wf_require(!body.is_empty() && body.len() <= INPUT_BYTES && comment["body"] == body,
        "strict bounded approval body changed between authenticated author/editor snapshots")?;
    Ok(body)
}
fn wf_original(token: &str, request: &WorkflowRequest) -> Result<(OriginalRun, Value)> {
    let run = wf_api_json(token, &format!("repos/anilbeesetti/ktlsp/actions/runs/{}", request.run_id))?;
    let jobs = wf_api_json(token, &format!("repos/anilbeesetti/ktlsp/actions/runs/{}/attempts/1/jobs?per_page=100", request.run_id))?;
    let list = jobs["jobs"].as_array().ok_or("original job API unavailable")?;
    wf_require(jobs["total_count"] == 1 && list.len() == 1 && list[0]["name"] == WF_JOB,
        "exactly one original protected job required")?;
    let id = list[0]["id"].as_u64().filter(|n| *n > 0).ok_or("original positive job ID unavailable")?;
    let job = wf_api_json(token, &format!("repos/anilbeesetti/ktlsp/actions/jobs/{id}"))?;
    let commit = wf_api_json(token, &format!("repos/anilbeesetti/ktlsp/git/commits/{}", request.source_sha))?;
    let original = OriginalRun { repository: "anilbeesetti/ktlsp".into(), event: "workflow_dispatch".into(), attempt: 1,
        run_id: request.run_id, job_id: id,
        suite_id: run["check_suite_id"].as_u64().filter(|n| *n > 0).ok_or("original positive suite ID unavailable")?,
        source_head: request.source_sha.clone(), source_tree: request.checkout_tree.clone() };
    api_identity(&run, &job, &commit, &original)?;
    wf_require(run["head_branch"] == "main" && run["head_repository"]["id"] == run["repository"]["id"]
        && run["repository"]["id"].as_u64().is_some_and(|n| n > 0)
        && job["run_attempt"] == 1 && job["name"] == WF_JOB
        && !job["labels"].as_array().is_some_and(|v| v.contains(&json!("self-hosted"))), "original standard main-only job mismatch")?;
    let suite = wf_api_json(token, &format!("repos/anilbeesetti/ktlsp/check-suites/{}", original.suite_id))?;
    wf_require(suite["id"] == original.suite_id && suite["head_sha"] == original.source_head
        && suite["app"]["slug"] == "github-actions", "original suite source mismatch")?;
    Ok((original, job))
}
fn wf_source_approval(token: &str, request: &WorkflowRequest) -> Result<(WorkflowSourceAdmission, Vec<u8>, Vec<u8>)> {
    let (original, _) = wf_original(token, request)?;
    let workflow = wf_api(token, &format!("repos/anilbeesetti/ktlsp/contents/.github/workflows/t045-pr174-official-symbols.yml?ref={}", original.source_head), true, INPUT_BYTES)?;
    let bytes = wf_owner_comment(token, request.source_comment, Instant::now() + Duration::from_secs(60))?;
    let admission: WorkflowSourceAdmission = parse(&bytes)?;
    wf_validate_source(&admission, &original, request, &workflow)?;
    Ok((admission, bytes, workflow))
}
fn wf_validate_source(admission: &WorkflowSourceAdmission, original: &OriginalRun, request: &WorkflowRequest, workflow: &[u8]) -> Result<()> {
    wf_require(admission.schema_version == 1 && admission.scope == "PR174_SOURCE_ADMISSION"
        && admission.run == *original && admission.workflow_sha256 == sha_bytes(workflow)
        && admission.contract_sha256 == CONTRACT && admission.image == WF_IMAGE
        && admission.containment_template_sha256 == containment_template_sha256()?
        && admission.calibration_comment == request.calibration_comment && admission.official_comment == request.official_comment
        && admission.measured_future_facts.len() == 6
        && ["boot", "daemon", "host", "image-config", "native-binaries", "tools"].iter()
            .all(|fact| admission.measured_future_facts.iter().any(|value| value.as_str() == *fact)),
        "independent source admission does not authorize these measured preparation facts")
}
fn workflow_source(argv: &[String]) -> Result<Value> {
    wf_require(argv.len() == 1 && argv[0] == "workflow-source", "workflow-source takes no arguments")?;
    let request = wf_request_env()?;
    let token = wf_token_env()?;
    let (_, _, workflow) = wf_source_approval(&token, &request)?;
    let local = read_file(Path::new(".github/workflows/t045-pr174-official-symbols.yml"), INPUT_BYTES)?;
    wf_require(local == workflow, "authenticated source workflow differs from actual checkout bytes")?;
    Ok(json!({"status":"INDEPENDENT_SOURCE_ADMITTED","sourceHead":request.source_sha,"runtimeAuthority":"NONE; measured proposal and separate finite approvals required"}))
}
fn wf_file<T: Serialize>(path: &Path, value: &T) -> Result<()> { make_file(path, &wf_json(value)?, 0o400) }
fn wf_load_request() -> Result<WorkflowRequest> { load(&Path::new(WF_ROOT).join("request.json")) }
fn wf_proposal() -> Result<WorkflowProposal> {
    protected(&Path::new(WF_ROOT).join("proposal/proposal.json"), false)?;
    parse(&read_file(&Path::new(WF_ROOT).join("proposal/proposal.json"), 262144)?)
}
fn wf_run_tool(path: &Path, argv: &[String], cap: usize, end: u64) -> Result<Vec<u8>> {
    protected(path, false)?;
    let mut command = Command::new(path);
    command.env_clear().env("PATH", "/usr/bin:/bin").env("LANG", "C.UTF-8").env("LC_ALL", "C.UTF-8").args(argv);
    let output = bounded_command(command, cap, Instant::now() + Duration::from_millis(end.checked_sub(mono_ms()?).filter(|n| *n > 0).ok_or("preparation deadline")?))?;
    wf_require(output.ok(), "bounded preparation measurement failed; retained UNKNOWN")?;
    Ok(output.stdout)
}
fn wf_docker(argv: &[&str], cap: usize, end: u64) -> Result<Vec<u8>> {
    let mut command = Command::new("/usr/bin/docker");
    protected(Path::new("/usr/bin/docker"), false)?;
    command.env_clear().env("PATH", "/usr/bin:/bin").env("LANG", "C.UTF-8").env("LC_ALL", "C.UTF-8")
        .arg("--host=unix:///run/docker.sock").args(argv);
    let output = bounded_command(command, cap, Instant::now() + Duration::from_millis(end.checked_sub(mono_ms()?).filter(|n| *n > 0).ok_or("image measurement deadline")?))?;
    wf_require(output.ok(), "actual pinned image/daemon measurement unavailable")?;
    Ok(output.stdout)
}
fn workflow_publish_calibration_ready(dir: &Path, a: &Admission, b: &Binding, identity: &ProcessIdentity, case: CalibrationCase, e: &mut Evidence) -> Result<()> {
    if case != CalibrationCase::WorkflowStepCancel {return Ok(());}
    let end = mono_ms()?.checked_add(10000).ok_or("readiness measurement deadline")?;
    let actual = inspect(e, &a.tools.docker, &b.container_id, end)?;
    wf_require(actual["State"]["Running"] == true && actual["State"]["Pid"] == identity.init_pid
        && actual["State"]["OOMKilled"] == false && actual["RestartCount"] == 0
        && process_start(&read_file(&Path::new("/proc").join(identity.init_pid.to_string()).join("stat"),4096)?)? == (identity.init_pid, identity.init_start),
        "actual original cancellation peer lost before protected readiness publication")?;
    let (supervisor_pid, supervisor_starttime) = process_start(&read_file(Path::new("/proc/self/stat"),4096)?)?;
    wf_require(std::str::from_utf8(&read_file(Path::new("/proc/self/cgroup"),4096)?).map_err(|_| "supervisor cgroup encoding")?.trim()
        == format!("0::{}",b.supervisor_cgroup), "readiness publisher is not the original outside-step supervisor")?;
    let ready = WorkflowCancellationReady {schema_version:1,scope:"PR174_ORIGINAL_CANCELLATION_PEER_READY".into(),
        case,nonce:a.nonce.clone(),container_id:b.container_id.clone(),boot_id:a.boot_id.clone(),daemon_id:a.daemon_id.clone(),
        init_pid:identity.init_pid,init_starttime:identity.init_start,supervisor_pid,supervisor_starttime,
        armed_ms:b.armed_ms.ok_or("unarmed original cancellation case")?,observed_ms:mono_ms()?};
    wf_publish_readiness_file(e,dir,&ready)
}
fn wf_publish_readiness_file(e: &mut Evidence, dir: &Path, ready: &WorkflowCancellationReady) -> Result<()> {
    let final_path = dir.join("calibration-observer-ready.json");
    let next = dir.join("calibration-observer-ready.next");
    side_json(e,&next,ready)?;
    publish_complete_no_replace(&next,&final_path)
}
fn wf_verify_cancellation_ready(dir: &Path) -> Result<()> {
    let ready_path=dir.join("calibration-observer-ready.json");
    let ready:WorkflowCancellationReady=load(&ready_path)?;
    wf_require(fs::symlink_metadata(&ready_path).map_err(|error|error.to_string())?.nlink()==1,
        "protected readiness receipt must retain single-link custody")?;
    let a:Admission=load(&dir.join("admission.json"))?;let b=binding(dir)?;
    let identity:ProcessIdentity=load(&dir.join("process-identity.json"))?;
    let now=mono_ms()?;let end=now.checked_add(10000).ok_or("cancellation readiness deadline")?;
    wf_require(ready.schema_version==1&&ready.scope=="PR174_ORIGINAL_CANCELLATION_PEER_READY"
        && ready.case==CalibrationCase::WorkflowStepCancel&&calibration_case(dir,&a)?==Some(ready.case)
        && ready.nonce==a.nonce&&ready.nonce==b.nonce&&ready.container_id==b.container_id
        && ready.boot_id==a.boot_id&&ready.daemon_id==a.daemon_id
        && ready.init_pid==identity.init_pid&&ready.init_starttime==identity.init_start
        && Some(ready.armed_ms)==b.armed_ms&&ready.observed_ms>=ready.armed_ms&&ready.observed_ms<=now
        && ready.armed_ms.checked_add(300000).is_some_and(|deadline| now.checked_add(90000).is_some_and(|reserve|reserve<deadline)),
        "protected cancellation readiness lacks original identities/one-minute active-peer reserve")?;
    let mut evidence=Evidence::preflight();
    daemon(&mut evidence,&a.tools.docker,end,&a.daemon_id)?;
    let actual=inspect(&mut evidence,&a.tools.docker,&b.container_id,end)?;
    wf_require(actual["State"]["Running"]==true&&actual["State"]["Pid"]==ready.init_pid
        && actual["State"]["OOMKilled"]==false&&actual["RestartCount"]==0
        && process_start(&read_file(&Path::new("/proc").join(ready.init_pid.to_string()).join("stat"),4096)?)?==(ready.init_pid,ready.init_starttime),
        "original admitted cancellation peer is not actually active")?;
    let state=properties(&evidence.checked(&a.tools.systemctl,&args(&["show","--property=ActiveState","--property=MainPID","--property=ControlGroup",
        &format!("{}.service",unit(&a.nonce,"supervisor"))]),end)?)?;
    wf_require(state.get("ActiveState").map(String::as_str)==Some("active")
        && state.get("MainPID")==Some(&ready.supervisor_pid.to_string())
        && state.get("ControlGroup")==Some(&b.supervisor_cgroup)
        && process_start(&read_file(&Path::new("/proc").join(ready.supervisor_pid.to_string()).join("stat"),4096)?)?==(ready.supervisor_pid,ready.supervisor_starttime),
        "original outside-step supervisor is not actually active")?;
    Ok(())
}
fn workflow_authenticate_step_timeout(dir: &Path, a: &Admission, grant: &RootGrant, e: &mut Evidence) -> Result<()> {
    let token = String::from_utf8(read_file(&dir.join("github.token"), 4096)?).map_err(|_| "credential encoding")?;
    wf_require(!token.is_empty() && !token.bytes().any(|b| b.is_ascii_control()), "original cancellation credential unavailable")?;
    let end = Instant::now() + Duration::from_millis(a.preparation_deadline_ms.checked_sub(mono_ms()?).filter(|remaining| *remaining > 0).ok_or("original collection deadline")?);
    // Keep the existing 64KiB collector budget; raw authenticated inputs share it instead of
    // spending it twice on hex command envelopes. Only authenticated facts/hashes are persisted.
    let budget = e.available().min(64000);
    let job_bytes = wf_api_before(&token, &format!("repos/anilbeesetti/ktlsp/actions/jobs/{}", a.run.job_id), false, budget, end)?;
    let job: Value = parse(&job_bytes)?;
    let step = grant.calibration_step.as_ref().ok_or("independent cancellation step absent")?;
    let steps = job["steps"].as_array().ok_or("authenticated original steps unavailable")?;
    let position = steps.iter().position(|candidate| candidate["number"] == step.number && candidate["name"] == step.name)
        .ok_or("actual original cancellation-step identity absent")?;
    let logs = wf_api_before(&token, &format!("repos/anilbeesetti/ktlsp/actions/jobs/{}/logs", a.run.job_id), true,
        budget.saturating_sub(job_bytes.len()), end)?;
    let step_logs = wf_api_before(&token, &format!("repos/anilbeesetti/ktlsp/actions/jobs/{}/steps/{position}/logs", a.run.job_id), true,
        budget.saturating_sub(job_bytes.len()).saturating_sub(logs.len()), end)?;
    let completed = ktlsp_oracle_replay::receiver::validate_step_timeout(grant, &job, &logs, &step_logs)?;
    let action: ktlsp_oracle_replay::authority::CalibrationAction = load(&dir.join("calibration-action.json"))?;
    action.validate()?;
    wf_require(action.case == CalibrationCase::WorkflowStepCancel && action.nonce == a.nonce
        && action.boot_id == a.boot_id && action.daemon_id == a.daemon_id
        && action.observed_unix_ms >= completed, "actual outside-step live-original action did not survive authenticated runner STEP completion")?;
    e.append(&json!({"kind":"authenticated-workflow-step-cancellation","runId":a.run.run_id,"jobId":a.run.job_id,
        "attempt":1,"stepNumber":step.number,"stepName":step.name,"completedUnixMs":completed,
        "jobSha256":sha_bytes(&job_bytes),"logsSha256":sha_bytes(&logs),"stepLogsSha256":sha_bytes(&step_logs),
        "outsideStepActionUnixMs":action.observed_unix_ms,"wholeJobCompletionClaimed":false}),false)
}
fn wf_prepare() -> Result<Value> {
    let request = wf_request_env()?;
    let token = wf_token_env()?;
    let (source, source_bytes, workflow) = wf_source_approval(&token, &request)?;
    protected(Path::new(WF_SOURCE), true)?; protected(Path::new(WF_BIN), true)?;
    wf_require(!Path::new(WF_ROOT).try_exists().map_err(|e| e.to_string())? && !Path::new(ROOT).try_exists().map_err(|e| e.to_string())?, "exclusive original preparation; no retry/reuse")?;
    mkdir(Path::new(WF_ROOT), 0o755)?; mkdir(Path::new(ROOT), 0o700)?;
    for name in ["proposal", "grants", "cases", "receipts", "uploads"] { mkdir(&Path::new(WF_ROOT).join(name), if matches!(name, "proposal" | "uploads") {0o755} else {0o700})?; }
    mkdir(Path::new(STAGING), 0o700)?;
    wf_file(&Path::new(WF_ROOT).join("request.json"), &request)?;
    make_file(&Path::new(WF_ROOT).join("github.token"), token.as_bytes(), 0o600)?;
    make_file(&Path::new(WF_ROOT).join("source-admission.json"), &source_bytes, 0o400)?;
    let end = mono_ms()?.checked_add(1_800_000).ok_or("preparation deadline overflow")?;
    wf_require(workflow == read_file(&Path::new(WF_SOURCE).join(".github/workflows/t045-pr174-official-symbols.yml"), INPUT_BYTES)?, "protected workflow source bytes mismatch")?;
    let mut tools = Vec::new();
    for role in ["docker", "systemctl", "sha256sum", "gh", "mount", "findmnt", "stat", "curl"] {
        let path = PathBuf::from(format!("/usr/bin/{role}")); protected(&path, false)?;
        tools.push(ToolSeal { role: role.into(), path: path.to_string_lossy().into_owned(), sha256: sha_path(&path, end)? });
    }
    let host = measured_host()?; host.validate()?;
    let mut evidence = Evidence::preflight(); core_policy(&mut evidence)?;
    let abi = confinement_abi(&child_confinement::profile())?;
    let daemon_info: Value = parse(&wf_docker(&["info", "--format", "{{json .}}"], 131072, end)?)?;
    let daemon_id = daemon_info["ID"].as_str().filter(|s| !s.is_empty()).ok_or("actual daemon identity unavailable")?.to_owned();
    daemon(&mut evidence, Path::new("/usr/bin/docker"), end, &daemon_id)?;
    wf_docker(&["pull", "--platform=linux/amd64", WF_IMAGE], 65536, end)?;
    let image_inspect: Value = parse(&wf_docker(&["image", "inspect", WF_IMAGE], 131072, end)?)?;
    let images = image_inspect.as_array().ok_or("actual image inspect shape")?;
    wf_require(images.len() == 1 && images[0]["Architecture"] == "amd64" && images[0]["Os"] == "linux"
        && images[0]["RepoDigests"].as_array().is_some_and(|v| v.contains(&json!(WF_IMAGE))), "actual immutable amd64 image identity mismatch")?;
    let image_id = images[0]["Id"].as_str().filter(|s| s.strip_prefix("sha256:").is_some_and(digest)).ok_or("actual image config digest unavailable")?.to_owned();
    let (_, job) = wf_original(&token, &request)?;
    let step = wf_named_step(&job, WF_CANCEL, false)?;
    let source_path = Path::new(WF_SOURCE);
    for (remote, local) in [("tools/oracle/Cargo.lock", "tools/oracle/Cargo.lock"), ("rust-toolchain.toml", "rust-toolchain.toml")] {
        let actual = wf_api(&token, &format!("repos/anilbeesetti/ktlsp/contents/{remote}?ref={}", request.source_sha), true, 1048576)?;
        wf_require(actual == read_file(&source_path.join(local), 1048576)?, "protected lock/toolchain differs from authenticated source")?;
    }
    let grant = RootGrant { schema_version: 1, scope: GrantScope::RustCalibration,
        repository: source.run.repository.clone(), event: source.run.event.clone(), attempt: 1,
        run_id: source.run.run_id, job_id: source.run.job_id, suite_id: source.run.suite_id,
        source_head: source.run.source_head.clone(), source_tree: source.run.source_tree.clone(),
        workflow_path: ".github/workflows/t045-pr174-official-symbols.yml".into(), workflow_sha256: sha_bytes(&workflow), contract_sha256: CONTRACT.into(),
        host_binary_sha256: sha_path(&Path::new(WF_BIN).join("ktlsp-oracle-pr174-host"), end)?,
        adapter_sha256: sha_path(&Path::new(WF_BIN).join("ktlsp-oracle-official-pr174"), end)?,
        peer_sha256: sha_path(&Path::new(WF_BIN).join("ktlsp-oracle-calibration-peer"), end)?,
        lock_sha256: sha_path(&source_path.join("tools/oracle/Cargo.lock"), end)?, toolchain_sha256: sha_path(&source_path.join("rust-toolchain.toml"), end)?,
        containment_template_sha256: containment_template_sha256()?, image: WF_IMAGE.into(), image_id, tools,
        required_calibration_sha256: None, calibration_case: Some(CalibrationCase::Normal), calibration_step: None, host };
    grant.validate_shape()?;
    wf_require(sha_path(&std::env::current_exe().map_err(|e| e.to_string())?, end)? == grant.host_binary_sha256, "creator must be actual sealed host binary")?;
    for name in ["inputs", "inputs/tool", "inputs/fixture"] { mkdir(&Path::new(STAGING).join(name), 0o755)?; }
    for (name, size, hash) in FIXTURES {
        let src = source_path.join("tools/oracle/fixtures/official-pr174-symbols").join(name);
        wf_require(sha_path(&src, end)? == hash && fs::metadata(&src).map_err(|e| e.to_string())?.len() == size, "unchanged fixture pin")?;
        copy_sealed(&src, &Path::new(STAGING).join("inputs/fixture").join(name), size, 0o444, end)?;
    }
    let inventory = source_path.join("tools/oracle/fixtures/official-pr174-symbols/archive-members.json");
    wf_require(fs::metadata(&inventory).map_err(|e| e.to_string())?.len() == 193661 && sha_path(&inventory, end)? == WF_INVENTORY, "unchanged publisher archive inventory pin")?;
    copy_sealed(&inventory, &Path::new(STAGING).join("inputs/archive-members.json"), 193661, 0o400, end)?;
    for (src, target, cap) in [(source_path.join("tools/oracle/Cargo.lock"), "Cargo.lock", 1048576), (source_path.join("rust-toolchain.toml"), "rust-toolchain.toml", 16384)] { copy_sealed(&src, &Path::new(STAGING).join(target), cap, 0o400, end)?; }
    for name in ["ktlsp-oracle-official-pr174", "ktlsp-oracle-calibration-peer"] { copy_sealed(&Path::new(WF_BIN).join(name), &Path::new(STAGING).join("inputs/tool").join(name), 134217728, 0o555, end)?; }
    make_file(&Path::new(STAGING).join("github.token"), token.as_bytes(), 0o600)?;
    make_file(&Path::new(STAGING).join("workflow.yml"), &workflow, 0o400)?;
    let archive = Path::new(STAGING).join("inputs/archive.tar.gz");
    make_file(&archive, b"", 0o600)?;
    wf_run_tool(Path::new("/usr/bin/curl"), &args(&["--fail", "--silent", "--show-error", "--location", "--proto", "=https", "--proto-redir", "=https", "--max-redirs", "5", "--connect-timeout", "30", "--max-time", "900", "--max-filesize", "368488700", "--output", archive.to_str().ok_or("archive path")?, WF_ARCHIVE_URL]), 4096, end)?;
    wf_require(fs::metadata(&archive).map_err(|e| e.to_string())?.len() == 368488700 && sha_path(&archive, end)? == ARCHIVE, "actual publisher archive bytes/hash mismatch; no START")?;
    fs::set_permissions(&archive, fs::Permissions::from_mode(0o400)).map_err(|e| e.to_string())?;
    let proposal = WorkflowProposal { schema_version: 1, scope: "PR174_MEASURED_GRANT_PROPOSAL".into(), source_admission_sha256: sha_bytes(&source_bytes), grant,
        cancellation_step: CalibrationStep { name: step.name, number: step.number },
        boot_id: String::from_utf8(read_file(Path::new("/proc/sys/kernel/random/boot_id"), 128)?).map_err(|_| "boot ID encoding")?.trim().into(),
        daemon_id, image_inspect, daemon_info: json!({
            "rawSha256":sha_bytes(&wf_json(&daemon_info)?),
            "ID":daemon_info["ID"],"ServerVersion":daemon_info["ServerVersion"],
            "OSType":daemon_info["OSType"],"Architecture":daemon_info["Architecture"],
            "KernelVersion":daemon_info["KernelVersion"],"OperatingSystem":daemon_info["OperatingSystem"],
            "CgroupVersion":daemon_info["CgroupVersion"],"CgroupDriver":daemon_info["CgroupDriver"],
            "Driver":daemon_info["Driver"],"SecurityOptions":daemon_info["SecurityOptions"],
            "DockerRootDir":daemon_info["DockerRootDir"],"DefaultRuntime":daemon_info["DefaultRuntime"],
        }), os_release_hex: raw_hex(&os_release()?),
        kernel_release_hex: raw_hex(&read_file(Path::new("/proc/sys/kernel/osrelease"), 1024)?), landlock_abi: abi,
        archive_sha256: ARCHIVE.into(), inventory_sha256: WF_INVENTORY.into() };
    let bytes = wf_json(&proposal)?; wf_require(bytes.len() <= 262144, "measured proposal bound")?;
    make_file(&Path::new(WF_ROOT).join("proposal/proposal.json"), &bytes, 0o644)?;
    Ok(json!({"status":"MEASURED_PROPOSAL_READY","proposalSha256":sha_bytes(&bytes),"authority":"NO CHILD OR SERVICE PERMISSION; independent ROOT approval of uploaded actual bytes required"}))
}
fn wf_named_step(job: &Value, name: &str, running: bool) -> Result<UploaderStep> {
    let steps = job["steps"].as_array().ok_or("original steps unavailable")?;
    wf_require(steps.len() <= 256 && steps.windows(2).all(|p| p[0]["number"].as_u64().zip(p[1]["number"].as_u64()).is_some_and(|(a,b)| a < b)), "original ordered unique steps required")?;
    let mut matches = steps.iter().filter(|step| step["name"] == name);
    let step = matches.next().ok_or("actual named original step absent")?;
    wf_require(matches.next().is_none() && (!running || step["status"] == "in_progress"), "unique actual named original step required")?;
    Ok(UploaderStep { name: name.into(), number: step["number"].as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| *n > 0).ok_or("original numeric step identity unavailable")? })
}
fn wf_artifact(token: &str, original: &OriginalRun, artifact_id: u64) -> Result<(Value, ktlsp_oracle_replay::receiver::Files)> {
    let artifact = wf_api_json(token, &format!("repos/anilbeesetti/ktlsp/actions/artifacts/{artifact_id}"))?;
    wf_require(artifact["id"] == artifact_id && artifact["expired"] == false
        && artifact["workflow_run"]["id"] == original.run_id && artifact["workflow_run"]["head_sha"] == original.source_head
        && artifact["size_in_bytes"].as_u64().is_some_and(|n| n > 0 && n <= 33554432), "authenticated original artifact binding unavailable")?;
    let zip = wf_api(token, &format!("repos/anilbeesetti/ktlsp/actions/artifacts/{artifact_id}/zip"), true, 33554432)?;
    wf_require(artifact["digest"] == format!("sha256:{}", sha_bytes(&zip)) && artifact["size_in_bytes"] == zip.len() as u64, "actual artifact download/API digest mismatch")?;
    let files = ktlsp_oracle_replay::receiver::retrieval::read_zip(&zip)?;
    Ok((artifact, files))
}
fn wf_wait_approval(token: &str, id: u64, scope: &str, original: &OriginalRun, artifact: &WorkflowArtifact, calibration: Option<(&str, u64, &str)>) -> Result<Vec<u8>> {
    let end = Instant::now() + Duration::from_secs(900);
    loop {
        let bytes = wf_owner_comment(token, id, end)?;
        wf_require(Instant::now() < end, "finite independent ROOT approval wait expired; UNKNOWN, no START")?;
        if let Ok(approval) = parse::<WorkflowApproval>(&bytes) {
            wf_validate_approval(&approval, scope, original, artifact, calibration)?;
            return Ok(bytes);
        }
        wf_require(Instant::now() < end, "finite independent ROOT approval wait expired; UNKNOWN, no START")?;
        std::thread::sleep(Duration::from_secs(5).min(end.saturating_duration_since(Instant::now())));
    }
}
// Consistency helper only: the caller authenticates the actual repository-owner API comment.
fn wf_validate_approval(approval: &WorkflowApproval, scope: &str, original: &OriginalRun, artifact: &WorkflowArtifact, calibration: Option<(&str, u64, &str)>) -> Result<()> {
    wf_require(approval.schema_version == 1 && approval.scope == scope && approval.run == *original
        && approval.proposal_sha256 == artifact.proposal_sha256 && approval.proposal_artifact_id == artifact.artifact_id
        && approval.proposal_artifact_api_digest == artifact.api_digest
        && approval.calibration_sha256.as_deref() == calibration.map(|c| c.0)
        && approval.calibration_artifact_id == calibration.map(|c| c.1)
        && approval.calibration_artifact_api_digest.as_deref() == calibration.map(|c| c.2),
        "owner approval is for different source/proposal/phase/original identity")
}
fn wf_approve(artifact_id: u64) -> Result<Value> {
    let request = wf_load_request()?; let token = wf_token()?;
    let (original, _) = wf_original(&token, &request)?;
    let proposal = wf_proposal()?;
    let bytes = read_file(&Path::new(WF_ROOT).join("proposal/proposal.json"), 262144)?;
    let (artifact, files) = wf_artifact(&token, &original, artifact_id)?;
    wf_require(files.len() == 1 && files.get("proposal.json") == Some(&bytes), "uploaded proposal actual bytes differ from protected measurement")?;
    let admitted = WorkflowArtifact { artifact_id, api_digest: artifact["digest"].as_str().ok_or("artifact digest")?.into(), proposal_sha256: sha_bytes(&bytes) };
    let approval = wf_wait_approval(&token, request.calibration_comment, "PR174_MEASURED_CALIBRATION_ADMISSION", &original, &admitted, None)?;
    wf_file(&Path::new(WF_ROOT).join("proposal-artifact.json"), &admitted)?;
    make_file(&Path::new(WF_ROOT).join("calibration-approval.json"), &approval, 0o400)?;
    for case in CalibrationCase::ALL {
        let mut grant = proposal.grant.clone(); grant.calibration_case = Some(case);
        grant.calibration_step = if case == CalibrationCase::WorkflowStepCancel {Some(proposal.cancellation_step.clone())} else {None};
        grant.validate_shape()?;
        wf_file(&Path::new(WF_ROOT).join("grants").join(format!("{}.json", case.as_str())), &grant)?;
    }
    Ok(json!({"status":"TEN_FINITE_ROOT_GRANTS_INSTALLED","officialPermission":false}))
}
fn workflow_validate_measured_host(a: &Admission, grant: &RootGrant) -> Result<()> {
    let proposal = wf_proposal()?;
    let request = wf_load_request()?;
    let artifact: WorkflowArtifact = load(&Path::new(WF_ROOT).join("proposal-artifact.json"))?;
    let proposal_bytes = read_file(&Path::new(WF_ROOT).join("proposal/proposal.json"), 262144)?;
    let source_bytes = read_file(&Path::new(WF_ROOT).join("source-admission.json"), INPUT_BYTES)?;
    let source: WorkflowSourceAdmission = parse(&source_bytes)?;
    wf_require(proposal.schema_version == 1 && proposal.scope == "PR174_MEASURED_GRANT_PROPOSAL"
        && proposal.source_admission_sha256 == sha_bytes(&source_bytes)
        && artifact.proposal_sha256 == sha_bytes(&proposal_bytes)
        && a.run == source.run && a.run.source_head == request.source_sha
        && a.boot_id == proposal.boot_id && a.daemon_id == proposal.daemon_id,
        "actual host/run differs from independently approved protected proposal; no CREATE/START")?;
    let mut expected = proposal.grant;
    let (approval_path, scope) = match grant.scope {
        GrantScope::RustCalibration => {
            expected.calibration_case = grant.calibration_case;
            expected.calibration_step = if grant.calibration_case == Some(CalibrationCase::WorkflowStepCancel) {
                Some(proposal.cancellation_step)
            } else {None};
            ("calibration-approval.json", "PR174_MEASURED_CALIBRATION_ADMISSION")
        }
        GrantScope::OfficialCapture => {
            expected.scope = GrantScope::OfficialCapture;
            expected.calibration_case = None;
            let raw = read_file(&Path::new(STAGING).join("collected-calibration.json"), INPUT_BYTES)?;
            expected.required_calibration_sha256 = Some(sha_bytes(&raw));
            let report: CalibrationReport = parse(&raw)?;
            report.validate(&expected)?;
            wf_require(report.boot_id == a.boot_id && report.daemon_id == a.daemon_id,
                "official phase lost original same-host completed cohort")?;
            ("official-approval.json", "PR174_SEPARATE_OFFICIAL_ADMISSION")
        }
    };
    grant.validate(&expected)?;
    let approval: WorkflowApproval = load(&Path::new(WF_ROOT).join(approval_path))?;
    let calibration: Option<WorkflowCalibrationArtifact> = if grant.scope == GrantScope::OfficialCapture {
        let measured: WorkflowCalibrationArtifact = load(&Path::new(WF_ROOT).join("official-calibration-artifact.json"))?;
        wf_require(Some(&measured.calibration_sha256) == expected.required_calibration_sha256.as_ref(),
            "official approval artifact differs from actual collected calibration bytes")?;
        Some(measured)
    } else {None};
    wf_validate_approval(&approval, scope, &a.run, &artifact,
        calibration.as_ref().map(|c| (c.calibration_sha256.as_str(), c.artifact_id, c.api_digest.as_str())))?;
    Ok(())
}
fn wf_case_path(name: &str) -> PathBuf { Path::new(WF_ROOT).join("cases").join(format!("{name}.json")) }
fn wf_case_name(name: &str) -> Result<Option<CalibrationCase>> {
    if name == "official" { return Ok(None); }
    CalibrationCase::ALL.into_iter().find(|case| case.as_str() == name).map(Some).ok_or("unknown finite workflow case".into())
}
fn wf_previous_complete() -> Result<()> {
    for entry in fs::read_dir(Path::new(WF_ROOT).join("cases")).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name(); let name = name.to_str().ok_or("case name encoding")?;
        let name = name.strip_suffix(".json").ok_or("unexpected protected case record")?;
        wf_case_name(name)?;
        let receipt: ProtectedUploadReceipt = load(&Path::new(WF_ROOT).join("receipts").join(format!("{name}.json")))?;
        let case: WorkflowCase = load(&entry.path())?;
        let dir = directory(&Path::new(ROOT).join(&case.nonce).to_string_lossy())?;
        let teardown: Value = load(&dir.join("teardown-receipt.json"))?;
        wf_require(receipt.schema_version == 1 && receipt.scope == "PR174_PROTECTED_UPLOAD"
            && receipt.nonce == case.nonce && receipt.artifact_id > 0
            && teardown["nonce"] == receipt.nonce && teardown["containerId"] == receipt.container_id
            && teardown["teardown"] == "CONTAINER_TEARDOWN_CONFIRMED",
            "prior protected upload/original exact-CID closure unavailable; later START blocked")?;
    }
    no_disposal_failure()
}
fn wf_rotate_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let next = path.with_extension("next"); wf_file(&next, value)?;
    fs::rename(next, path).map_err(|e| e.to_string())
}
fn workflow_record_bootstrap_nonce(dir: &Path, a: &Admission) -> Result<()> {
    let case = calibration_case(dir, a)?;
    let name = case.map(CalibrationCase::as_str).unwrap_or("official");
    let reserved: WorkflowCase = load(&wf_case_path(name))?;
    wf_require(reserved.case == case && reserved.nonce.is_empty()
        && dir.file_name().and_then(|name| name.to_str()) == Some(a.nonce.as_str()),
        "creator must reserve one finite case before original service launch")?;
    wf_rotate_json(&wf_case_path(name), &WorkflowCase {case, nonce: a.nonce.clone()})
}
fn wf_start_case(case: Option<CalibrationCase>) -> Result<PathBuf> {
    wf_previous_complete()?;
    let name = case.map(CalibrationCase::as_str).unwrap_or("official");
    wf_require(!wf_case_path(name).try_exists().map_err(|e| e.to_string())?, "finite case already claimed; no retry")?;
    let grant: RootGrant = load(&Path::new(WF_ROOT).join("grants").join(format!("{name}.json")))?;
    grant.validate_shape()?;
    wf_require(grant.calibration_case == case && (grant.scope == GrantScope::RustCalibration) == case.is_some(), "case/official grant scope mismatch")?;
    wf_rotate_json(Path::new(EXPECTED_GRANT), &grant)?;
    wf_rotate_json(&Path::new(STAGING).join("root-authorization.json"), &grant)?;
    if let Some(case) = case {
        wf_rotate_json(&Path::new(STAGING).join("calibration.json"), &CalibrationPlan {schema_version:1,scope:GrantScope::RustCalibration,case})?;
    } else {
        let report: CalibrationReport = load(&Path::new(STAGING).join("collected-calibration.json"))?;
        report.validate(&grant)?;
        wf_rotate_json(&Path::new(STAGING).join("calibration.json"), &report)?;
    }
    // Reserve the case before bootstrap. A failed bootstrap never permits another launch.
    wf_file(&wf_case_path(name), &WorkflowCase {case, nonce: String::new()})?;
    let result = match bootstrap() {
        Ok(result) => result,
        Err(error) => {
            // Recover only the creator's protected, exclusive grant claim; never parse an error string for a nonce.
            let claim = grant_claim(&grant)?;
            if let Ok(raw) = read_file(&claim, 128)
                && let Ok(text) = std::str::from_utf8(&raw)
                && let Some(nonce) = text.strip_suffix('\n')
                && digest(nonce)
            {
                protected(&claim, false)?;
                wf_rotate_json(&wf_case_path(name), &WorkflowCase {case, nonce: nonce.into()})?;
            }
            return Err(error);
        }
    };
    let dir = directory(result["admissionDirectory"].as_str().ok_or("original bootstrap directory unavailable")?)?;
    let a: Admission = load(&dir.join("admission.json"))?;
    let recorded: WorkflowCase = load(&wf_case_path(name))?;
    wf_require(recorded.case == case && recorded.nonce == a.nonce, "creator original nonce was not protected before service START")?;
    Ok(dir)
}
fn wf_case_dir(name: &str) -> Result<PathBuf> {
    let expected = wf_case_name(name)?;
    let record: WorkflowCase = load(&wf_case_path(name))?;
    wf_require(record.case == expected && digest(&record.nonce), "original case bootstrap incomplete; UNKNOWN, no START")?;
    directory(&Path::new(ROOT).join(record.nonce).to_string_lossy())
}
fn wf_wait_export(dir: &Path) -> Result<()> {
    // Six minutes leave the existing eight-minute collector step its copy/failure/output reserve.
    wf_wait_export_until(dir, Instant::now() + Duration::from_secs(360))
}
fn wf_wait_export_until(dir: &Path, end: Instant) -> Result<()> {
    let binding=dir.join("export/export-binding.json");
    loop {
        if binding.try_exists().map_err(|e|e.to_string())? {
            protected(&binding,false)?;
            wf_require(fs::symlink_metadata(&binding).map_err(|e|e.to_string())?.nlink()==1,
                "protected export binding must retain single-link custody")?;
            return Ok(());
        }
        wf_require(Instant::now() < end, "finite original outside-step closure/export unavailable; UNKNOWN")?;
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn wf_finish_export(name: &str) -> Result<Value> {
    let dir = wf_case_dir(name)?;
    wf_wait_export(&dir)?;
    let binding: ktlsp_oracle_replay::authority::ExportBinding = load(&dir.join("export/export-binding.json"))?;
    let a: Admission = load(&dir.join("admission.json"))?;
    let teardown: Value = load(&dir.join("teardown-receipt.json"))?;
    wf_require(binding.nonce == a.nonce && teardown["nonce"] == a.nonce && teardown["containerId"] == binding.container_id,
        "protected export original identity mismatch")?;
    let closed = teardown["teardown"] == "CONTAINER_TEARDOWN_CONFIRMED";
    let target = Path::new(WF_ROOT).join("uploads").join(name);
    if target.try_exists().map_err(|e| e.to_string())? {
        wf_require(read_file(&target.join("export-binding.json"), INPUT_BYTES)? == read_file(&dir.join("export/export-binding.json"), INPUT_BYTES)?,
            "partial protected export is retained, never reused as complete")?;
        wf_require(closed, "original closure UNKNOWN; failed protected export retained")?;
        return Ok(json!({"status":"PROTECTED_EXPORT_RETAINED","case":name}));
    }
    mkdir(&target, 0o755)?;
    let end = mono_ms()?.checked_add(60000).ok_or("export copy deadline")?;
    wf_require(binding.files.len() <= 47, "bounded protected export inventory")?;
    for member in &binding.files {
        wf_require(ktlsp_oracle_replay::receiver::retrieval::safe_name(&member.path) && member.bytes <= 8388608, "safe bounded protected export member")?;
        let dest = target.join(&member.path);
        let parent = dest.parent().ok_or("export parent")?;
        let relative = parent.strip_prefix(&target).map_err(|_| "export ancestry")?;
        let mut path = target.clone();
        for component in relative.components() {path.push(component); if !path.try_exists().map_err(|e| e.to_string())? {mkdir(&path,0o755)?;} protected(&path,true)?;}
        let src = dir.join("export").join(&member.path);
        wf_require(sha_path(&src,end)? == member.sha256 && fs::metadata(&src).map_err(|e|e.to_string())?.len() == member.bytes, "original protected export inventory bytes changed")?;
        copy_sealed(&src,&dest,member.bytes,0o444,end)?;
    }
    copy_sealed(&dir.join("export/export-binding.json"),&target.join("export-binding.json"),16384,0o444,end)?;
    wf_require(closed, "original closure UNKNOWN; failed protected export retained")?;
    Ok(json!({"status":"PROTECTED_EXPORT_READY","case":name,"path":target}))
}
fn wf_finish(name: &str) -> Result<Value> {
    let result = wf_finish_export(name);
    if result.is_err() {
        let target = Path::new(WF_ROOT).join("uploads").join(name);
        if !target.try_exists().map_err(|e| e.to_string())? { mkdir(&target, 0o755)?; }
        // Unknown custody/closure cannot export child bytes or fabricate a missing binding/CID.
        // Retain only fixed protected creator facts so the always uploader preserves the failure.
        if !target.join("export-binding.json").try_exists().map_err(|e| e.to_string())? {
            let failure = target.join("producer-failure.json");
            if !failure.try_exists().map_err(|e| e.to_string())? {
                make_file(&failure, &wf_json(&json!({"schemaVersion":1,"scope":"PR174_PRODUCER_UNKNOWN",
                    "case":name,"reason":"Original protected export unavailable or incomplete; no authority for later START",
                    "officialCaptureSuccess":false}))?, 0o444)?;
                let end = mono_ms()?.checked_add(10000).ok_or("failure retention deadline")?;
                for file in ["request.json", "source-admission.json", "calibration-approval.json", "official-approval.json"] {
                    let source = Path::new(WF_ROOT).join(file);
                    if source.try_exists().map_err(|e| e.to_string())? {
                        copy_sealed(&source, &target.join(file), INPUT_BYTES as u64, 0o444, end)?;
                    }
                }
                let original_dir = (|| -> Result<PathBuf> {
                    let record: WorkflowCase = load(&wf_case_path(name))?;
                    let nonce = if digest(&record.nonce) {record.nonce} else {
                        let grant: RootGrant = load(&Path::new(WF_ROOT).join("grants").join(format!("{name}.json")))?;
                        let claim = grant_claim(&grant)?;
                        protected(&claim, false)?;
                        String::from_utf8(read_file(&claim, 128)?).map_err(|_| "claim encoding")?
                            .strip_suffix('\n').filter(|nonce| digest(nonce)).ok_or("no original nonce claim")?.into()
                    };
                    directory(&Path::new(ROOT).join(nonce).to_string_lossy())
                })();
                if let Ok(dir) = original_dir {
                    for subdir in ["bootstrap", "receipts"] {
                        let path = target.join(subdir);
                        if !path.try_exists().map_err(|e| e.to_string())? {mkdir(&path, 0o755)?;}
                    }
                    for (file, cap) in [("bootstrap-receipt.json",4096),("bootstrap/observer.evidence",126976),
                        ("admission.json",16384),("root-authorization.json",16384),("expected-grant.json",16384),
                        ("calibration.json",16384),("workflow.yml",16384),("binding.json",16384),
                        ("teardown-receipt.json",16384),("receipts/observer.evidence",786432),("receipts/cleanup.evidence",262144)] {
                        let source = dir.join(file);let dest = target.join(file);
                        if source.try_exists().map_err(|e| e.to_string())? && !dest.try_exists().map_err(|e| e.to_string())? {
                            copy_sealed(&source, &dest, cap, 0o444, end)?;
                        }
                    }
                }
            }
        }
    }
    result
}
fn wf_receipt(name: &str, artifact_id: u64) -> Result<Value> {
    let request=wf_load_request()?; let token=wf_token()?;
    let (original,job)=wf_original(&token,&request)?;
    let dir=wf_case_dir(name)?;
    let binding_bytes=read_file(&dir.join("export/export-binding.json"),INPUT_BYTES)?;
    let binding:ktlsp_oracle_replay::authority::ExportBinding=parse(&binding_bytes)?;
    let grant_bytes=read_file(&dir.join("root-authorization.json"),INPUT_BYTES)?;
    let (artifact,files)=wf_artifact(&token,&original,artifact_id)?;
    wf_require(binding.run==original && files.get("export-binding.json")==Some(&binding_bytes)
        && files.get("root-authorization.json")==Some(&grant_bytes)
        && files.len()==binding.files.len()+1, "actual uploaded export binding/ROOT inventory differs")?;
    for member in &binding.files {
        let actual=files.get(&member.path).ok_or("uploaded protected inventory member absent")?;
        wf_require(actual.len() as u64==member.bytes && sha_bytes(actual)==member.sha256,"uploaded inventory hash mismatch")?;
    }
    let receipt=ProtectedUploadReceipt {schema_version:1,scope:"PR174_PROTECTED_UPLOAD".into(),repository:original.repository,
        run_id:original.run_id,job_id:original.job_id,suite_id:original.suite_id,attempt:1,
        source_head:original.source_head,source_tree:original.source_tree,workflow_sha256:binding.workflow_sha256,
        artifact_id,artifact_api_digest:artifact["digest"].as_str().ok_or("artifact API digest")?.into(),download_zip_sha256:None,
        export_binding_sha256:sha_bytes(&binding_bytes),nonce:binding.nonce,container_id:binding.container_id,
        root_grant_sha256:sha_bytes(&grant_bytes),uploader_step:wf_named_step(&job,&format!("PR174 receipt {name}"),true)?};
    let path=Path::new(WF_ROOT).join("receipts").join(format!("{name}.json"));
    wf_require(!path.try_exists().map_err(|e| e.to_string())?, "protected receipt already published; no retry")?;
    let bytes = wf_json(&receipt)?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(UPLOAD_PREFIX.as_bytes()).and_then(|()| stdout.write_all(&bytes))
        .and_then(|()| stdout.write_all(b"\n")).and_then(|()| stdout.flush()).map_err(|e| e.to_string())?;
    wf_file(&path,&receipt)?;
    if name!="official" {release_calibration(&dir)?;}
    Ok(json!({"status":"PROTECTED_UPLOAD_RECORDED","artifactId":artifact_id}))
}
fn wf_collect_inner() -> Result<Value> {
    wf_previous_complete()?;
    let mut locations=Vec::new();
    for case in CalibrationCase::ALL {
        let record:WorkflowCase=load(&wf_case_path(case.as_str()))?;
        wf_require(record.case==Some(case)&&digest(&record.nonce),"all ten real case identities required")?;
        locations.push(json!({"case":case,"nonce":record.nonce}));
    }
    wf_file(&Path::new(STAGING).join("calibration-cases.json"),&json!({"cases":locations}))?;
    let result = collect_calibration()?;
    let raw = read_file(&Path::new(STAGING).join("collected-calibration.json"), INPUT_BYTES)?;
    let output = Path::new(WF_ROOT).join("calibration-proposal");
    mkdir(&output, 0o755)?;
    make_file(&output.join("calibration.json"), &raw, 0o644)?;
    Ok(result)
}
fn wf_collection_result(target: &Path, collected: &Result<Value>, snapshot: &Result<Vec<&'static str>>) -> Result<()> {
    let bytes=wf_json(&json!({"schemaVersion":1,"scope":"PR174_PROTECTED_COLLECTION",
        "status":if collected.is_ok()&&snapshot.as_ref().is_ok_and(|missing|missing.is_empty()){"COLLECTION_COMPLETED"}else{"UNKNOWN"},
        "collectionError":collected.as_ref().err(),"snapshotError":snapshot.as_ref().err(),
        "missingFacts":snapshot.as_ref().ok(),"collectionResult":collected.as_ref().ok(),
        "officialCaptureSuccess":false}))?;
    wf_require(bytes.len()<=131072,"bounded collection result exceeded; initial UNKNOWN snapshot retained")?;
    make_file(&target.join("collection-result.json"),&bytes,0o444)
}
fn wf_collection_snapshot(target: &Path) -> Result<Vec<&'static str>> {
    let end=mono_ms()?.checked_add(10000).ok_or("collection evidence copy deadline")?;
    let mut missing=Vec::new();
    let files=[
        (Path::new(STAGING).join("calibration-cases.json"),"calibration-cases.json",16384),
        (Path::new(STAGING).join("collected-calibration.json"),"collected-calibration.json",16384),
        (Path::new(STAGING).join("root-authorization.json"),"root-authorization.json",16384),
        (Path::new(STAGING).join("workflow.yml"),"workflow.yml",16384),
        (Path::new(WF_ROOT).join("request.json"),"request.json",16384),
        (Path::new(WF_ROOT).join("source-admission.json"),"source-admission.json",16384),
        (Path::new(WF_ROOT).join("calibration-approval.json"),"calibration-approval.json",16384),
        (Path::new(WF_ROOT).join("proposal-artifact.json"),"proposal-artifact.json",16384),
    ];
    let cancellation = match wf_case_dir("workflow-step-cancel") {
        Ok(dir)=>Some((dir.join("cancellation-authentication.json"),"cancellation-authentication.json",262144)),
        Err(_)=>{missing.push("original-workflow-step-cancel-directory");None},
    };
    // Preserve the actual timeout authentication result first, even if a later metadata copy fails.
    for (source,name,cap) in cancellation.into_iter().chain(files) {
        match fs::symlink_metadata(&source) {
            Err(error) if error.kind()==ErrorKind::NotFound=>missing.push(name),
            Err(error)=>return Err(error.to_string()),
            Ok(_)=>copy_sealed(&source,&target.join(name),cap,0o444,end)?,
        }
    }
    Ok(missing)
}
fn wf_collection_started(target: &Path) -> Result<()> {
    let started=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_|"collection clock before epoch")?.as_millis();
    // This actual entered/UNKNOWN fact exists before any collector API operation can fail or stall.
    make_file(&target.join("collection-start.json"),&wf_json(&json!({"schemaVersion":1,
        "scope":"PR174_PROTECTED_COLLECTION","status":"IN_PROGRESS_UNKNOWN","startedUnixMs":started,
        "officialCaptureSuccess":false}))?,0o444)
}
fn wf_collect() -> Result<Value> {
    protected(Path::new(WF_ROOT),true)?;
    let target=Path::new(WF_ROOT).join("uploads/collection");
    mkdir(&target,0o755)?;
    wf_collection_started(&target)?;
    let collected=wf_collect_inner();
    let snapshot=wf_collection_snapshot(&target);
    wf_collection_result(&target,&collected,&snapshot)?;
    match (collected,snapshot) {
        (Ok(value),Ok(missing)) if missing.is_empty()=>Ok(value),
        _=>Err("original cohort collection UNKNOWN; actual protected collection evidence retained for always upload; no official START".into()),
    }
}
fn wf_official(calibration_artifact_id: u64) -> Result<Value> {
    wf_previous_complete()?;
    let request=wf_load_request()?;let token=wf_token()?;
    let (original,_)=wf_original(&token,&request)?;
    let proposal=wf_proposal()?;
    let artifact:WorkflowArtifact=load(&Path::new(WF_ROOT).join("proposal-artifact.json"))?;
    let raw=read_file(&Path::new(STAGING).join("collected-calibration.json"),INPUT_BYTES)?;
    let hash=sha_bytes(&raw);
    let mut grant=proposal.grant;
    grant.scope=GrantScope::OfficialCapture;grant.calibration_case=None;grant.calibration_step=None;grant.required_calibration_sha256=Some(hash.clone());
    let report:CalibrationReport=parse(&raw)?;report.validate(&grant)?;
    let (calibration_artifact, files) = wf_artifact(&token, &original, calibration_artifact_id)?;
    wf_require(files.len() == 1 && files.get("calibration.json") == Some(&raw),
        "uploaded actual calibration report differs from protected cohort bytes")?;
    let calibration_digest = calibration_artifact["digest"].as_str().ok_or("calibration artifact API digest unavailable")?;
    let approval=wf_wait_approval(&token,request.official_comment,"PR174_SEPARATE_OFFICIAL_ADMISSION",&original,&artifact,Some((&hash,calibration_artifact_id,calibration_digest)))?;
    make_file(&Path::new(WF_ROOT).join("official-approval.json"),&approval,0o400)?;
    wf_file(&Path::new(WF_ROOT).join("official-calibration-artifact.json"), &WorkflowCalibrationArtifact {
        artifact_id: calibration_artifact_id, api_digest: calibration_digest.into(), calibration_sha256: hash,
    })?;
    wf_file(&Path::new(WF_ROOT).join("grants/official.json"),&grant)?;
    wf_start_case(None)?;
    wf_finish("official")
}
fn workflow_entry(argv: &[String]) -> Result<Value> {
    match argv {
        [mode] if mode == "workflow-prepare" => wf_prepare(),
        [mode,id] if mode == "workflow-approve" => wf_approve(wf_positive(id)?),
        [mode,name] if mode == "workflow-case" => {
            let case=wf_case_name(name)?.ok_or("official requires separate approval route")?;
            wf_require(case!=CalibrationCase::WorkflowStepCancel,"runner cancellation requires split original step routes")?;
            let dir=wf_start_case(Some(case))?;await_calibration(&dir)?;wf_finish(name)
        },
        [mode] if mode == "workflow-cancel-start" => {
            let dir=wf_start_case(Some(CalibrationCase::WorkflowStepCancel))?;
            let end=Instant::now()+Duration::from_secs(120);
            loop {
                if dir.join("calibration-observer-ready.json").try_exists().map_err(|e|e.to_string())? {
                    wf_verify_cancellation_ready(&dir)?;
                    return Ok(json!({"status":"ORIGINAL_CANCELLATION_PEER_ACTIVE"}));
                }
                wf_require(Instant::now()<end,"actual original cancellation peer did not become active")?;
                std::thread::sleep(Duration::from_millis(20));
            }
        },
        [mode] if mode == "workflow-cancel-wait" => {
            let dir=wf_case_dir("workflow-step-cancel")?;
            wf_verify_cancellation_ready(&dir)?;
            // Only the actual runner timeout may interrupt this foreground finite wait.
            // A successful return is a failed cancellation experiment, never a timeout marker.
            let end=Instant::now()+Duration::from_secs(600);
            while Instant::now()<end {std::thread::sleep(Duration::from_millis(100));}
            Err("runner did not enforce the independently admitted one-minute step timeout".into())
        },
        [mode,name] if mode == "workflow-finish" => {wf_case_name(name)?;wf_finish(name)},
        [mode,name,id] if mode == "workflow-receipt" => {wf_case_name(name)?;wf_receipt(name,wf_positive(id)?)},
        [mode] if mode == "workflow-collect" => wf_collect(),
        [mode,id] if mode == "workflow-official" => wf_official(wf_positive(id)?),
        _=>Err("unknown workflow creator mode/arity; fixed original workflow commands only".into()),
    }
}

#[cfg(test)]
mod workflow_source_tests {
    use super::*;
    #[test]
    fn synthetic_creator_inputs_do_not_grant_authority() {
        assert!(wf_positive("01").is_err());assert!(wf_positive("0").is_err());assert_eq!(wf_positive("17").unwrap(),17);
        assert!(wf_case_name("../official").is_err());assert!(wf_case_name("normal --retry").is_err());
        assert_eq!(wf_case_name("official").unwrap(),None);
        assert_eq!(wf_case_name("workflow-step-cancel").unwrap(),Some(CalibrationCase::WorkflowStepCancel));
        assert!(parse::<WorkflowApproval>(br#"{"schemaVersion":1,"schemaVersion":1}"#).is_err());
    }
    #[test]
    fn synthetic_step_identity_rejects_duplicates_and_nonrunning_receipts() {
        let job=json!({"steps":[{"name":"receipt","number":2,"status":"in_progress"}]});
        assert_eq!(wf_named_step(&job,"receipt",true).unwrap().number,2);
        assert!(wf_named_step(&job,"missing",false).is_err());
        assert!(wf_named_step(&json!({"steps":[{"name":"receipt","number":2,"status":"completed"}]}),"receipt",true).is_err());
        assert!(wf_named_step(&json!({"steps":[{"name":"receipt","number":2},{"name":"receipt","number":3}]}),"receipt",false).is_err());
    }
    #[test]
    fn synthetic_owner_approval_consistency_cannot_cross_phases_or_artifacts() {
        let original = OriginalRun {repository:"anilbeesetti/ktlsp".into(),event:"workflow_dispatch".into(),
            attempt:1,run_id:11,job_id:12,suite_id:13,source_head:"a".repeat(40),source_tree:"b".repeat(40)};
        let artifact = WorkflowArtifact {artifact_id:14,api_digest:format!("sha256:{}","c".repeat(64)),proposal_sha256:"d".repeat(64)};
        let mut approval = WorkflowApproval {schema_version:1,scope:"PR174_MEASURED_CALIBRATION_ADMISSION".into(),
            run:original.clone(),proposal_sha256:artifact.proposal_sha256.clone(),proposal_artifact_id:14,
            proposal_artifact_api_digest:artifact.api_digest.clone(),calibration_sha256:None,
            calibration_artifact_id:None,calibration_artifact_api_digest:None};
        assert!(wf_validate_approval(&approval,"PR174_MEASURED_CALIBRATION_ADMISSION",&original,&artifact,None).is_ok());
        let mut missing = serde_json::to_value(&approval).unwrap();
        missing.as_object_mut().unwrap().remove("calibrationSha256");
        assert!(parse::<WorkflowApproval>(&wf_json(&missing).unwrap()).is_err());
        assert!(wf_validate_approval(&approval,"PR174_SEPARATE_OFFICIAL_ADMISSION",&original,&artifact,None).is_err());
        approval.proposal_artifact_id=15;
        assert!(wf_validate_approval(&approval,"PR174_MEASURED_CALIBRATION_ADMISSION",&original,&artifact,None).is_err());
        approval.proposal_artifact_id=14;
        approval.scope="PR174_SEPARATE_OFFICIAL_ADMISSION".into();
        let report="e".repeat(64);let digest=format!("sha256:{}","f".repeat(64));
        assert!(wf_validate_approval(&approval,"PR174_SEPARATE_OFFICIAL_ADMISSION",&original,&artifact,Some((&report,16,&digest))).is_err());
        approval.calibration_sha256=Some(report.clone());approval.calibration_artifact_id=Some(16);approval.calibration_artifact_api_digest=Some(digest.clone());
        assert!(wf_validate_approval(&approval,"PR174_SEPARATE_OFFICIAL_ADMISSION",&original,&artifact,Some((&report,16,&digest))).is_ok());
        approval.run.job_id=99;
        assert!(wf_validate_approval(&approval,"PR174_SEPARATE_OFFICIAL_ADMISSION",&original,&artifact,Some((&report,16,&digest))).is_err());
    }
    #[test]
    fn synthetic_source_consistency_requires_explicit_measured_future_scope() {
        let original = OriginalRun {repository:"anilbeesetti/ktlsp".into(),event:"workflow_dispatch".into(),
            attempt:1,run_id:11,job_id:12,suite_id:13,source_head:"a".repeat(40),source_tree:"b".repeat(40)};
        let request=WorkflowRequest {source_sha:original.source_head.clone(),checkout_tree:original.source_tree.clone(),
            run_id:11,source_comment:21,calibration_comment:22,official_comment:23};
        let workflow=b"synthetic workflow bytes, not an API response";
        let mut admission=WorkflowSourceAdmission {schema_version:1,scope:"PR174_SOURCE_ADMISSION".into(),run:original.clone(),
            workflow_sha256:sha_bytes(workflow),contract_sha256:CONTRACT.into(),image:WF_IMAGE.into(),
            containment_template_sha256:containment_template_sha256().unwrap(),
            measured_future_facts:["boot","daemon","host","image-config","native-binaries","tools"].map(String::from).to_vec(),
            calibration_comment:22,official_comment:23};
        assert!(wf_validate_source(&admission,&original,&request,workflow).is_ok());
        admission.measured_future_facts[0]="jvm-permission".into();
        assert!(wf_validate_source(&admission,&original,&request,workflow).is_err());
        admission.measured_future_facts[0]="boot".into();admission.official_comment=22;
        assert!(wf_validate_source(&admission,&original,&request,workflow).is_err());
    }
    #[test]
    fn synthetic_owner_authorship_does_not_authorize_nonowner_edits() {
        let owner=json!({"id":17,"login":"owner"});
        let comment=json!({"id":31,"node_id":"opaque-comment-id","body":"approved bytes"});
        let mut graph=json!({"data":{"node":{"__typename":"IssueComment","fullDatabaseId":"31",
            "body":"approved bytes","author":{"databaseId":17,"login":"owner"},
            "lastEditedAt":null,"editor":null}}});
        assert_eq!(wf_comment_body(&owner,&comment,&graph).unwrap(),"approved bytes");
        graph["data"]["node"]["lastEditedAt"]=json!("2026-10-16T12:00:00Z");
        graph["data"]["node"]["editor"]=json!({"databaseId":18,"login":"writer"});
        assert!(wf_comment_body(&owner,&comment,&graph).is_err());
        graph["data"]["node"]["editor"]=json!({"databaseId":17,"login":"owner"});
        assert!(wf_comment_body(&owner,&comment,&graph).is_ok());
        graph["data"]["node"]["body"]=json!("different bytes");
        assert!(wf_comment_body(&owner,&comment,&graph).is_err());
        graph["data"]["node"]["body"]=json!("approved bytes");
        graph["data"]["node"].as_object_mut().unwrap().remove("lastEditedAt");
        assert!(wf_comment_body(&owner,&comment,&graph).is_err());
    }
    #[test]
    fn synthetic_readiness_publication_never_exposes_partial_json() {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("ktlsp-workflow-ready-{}-{unique}",std::process::id()));
        fs::create_dir(&dir).unwrap();
        let ready = WorkflowCancellationReady {schema_version:1,scope:"PR174_ORIGINAL_CANCELLATION_PEER_READY".into(),
            case:CalibrationCase::WorkflowStepCancel,nonce:"a".repeat(64),container_id:"b".repeat(64),
            boot_id:"synthetic-boot".into(),daemon_id:"synthetic-daemon".into(),init_pid:11,init_starttime:12,
            supervisor_pid:13,supervisor_starttime:14,armed_ms:15,observed_ms:16};
        let next = dir.join("calibration-observer-ready.next");
        let final_path = dir.join("calibration-observer-ready.json");
        make_file(&next,b"{",0o600).unwrap();
        let mut evidence=Evidence::preflight();
        assert!(wf_publish_readiness_file(&mut evidence,&dir,&ready).is_err());
        assert!(!final_path.exists());
        assert_eq!(fs::read(&next).unwrap(),b"{");
        fs::remove_file(&next).unwrap();
        let mut evidence=Evidence::preflight();
        wf_publish_readiness_file(&mut evidence,&dir,&ready).unwrap();
        assert!(!next.exists());
        assert_eq!(fs::read(&final_path).unwrap(),wf_json(&ready).unwrap());
        let mut replacement=ready;
        replacement.nonce="c".repeat(64);
        assert!(wf_publish_readiness_file(&mut evidence,&dir,&replacement).is_err());
        let published:WorkflowCancellationReady=parse(&fs::read(&final_path).unwrap()).unwrap();
        assert_eq!(published.nonce,"a".repeat(64));
        assert_eq!(fs::read(&next).unwrap(),wf_json(&replacement).unwrap());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn synthetic_concurrent_completed_publishers_cannot_replace_the_winner() {
        let unique=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir=std::env::temp_dir().join(format!("ktlsp-workflow-concurrent-{}-{unique}",std::process::id()));
        fs::create_dir(&dir).unwrap();
        let barrier=std::sync::Arc::new(std::sync::Barrier::new(2));
        let final_path=dir.join("calibration-observer-ready.json");
        let mut publishers=Vec::new();
        for (name,bytes) in [("a.next",b"{\"complete\":\"a\"}".as_slice()),("b.next",b"{\"complete\":\"b\"}".as_slice())] {
            let next=dir.join(name);make_file(&next,bytes,0o600).unwrap();
            let final_path=final_path.clone();let barrier=barrier.clone();
            publishers.push(std::thread::spawn(move || {barrier.wait();publish_complete_no_replace(&next,&final_path)}));
        }
        let successes=publishers.into_iter().map(|publisher|publisher.join().unwrap().is_ok()).filter(|success|*success).count();
        assert_eq!(successes,1);
        let final_bytes=fs::read(&final_path).unwrap();
        assert!(final_bytes==b"{\"complete\":\"a\"}"||final_bytes==b"{\"complete\":\"b\"}");
        assert_eq!(usize::from(dir.join("a.next").exists())+usize::from(dir.join("b.next").exists()),1);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn synthetic_expired_export_wait_retains_actual_unknown_collection_facts() {
        let unique=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir=std::env::temp_dir().join(format!("ktlsp-workflow-collection-{}-{unique}",std::process::id()));
        fs::create_dir(&dir).unwrap();
        wf_collection_started(&dir).unwrap();
        let collected:Result<Value>=wf_wait_export_until(&dir,Instant::now()).map(|()|Value::Null);
        assert!(collected.is_err());
        wf_collection_result(&dir,&collected,&Ok(vec!["cancellation-authentication.json"])).unwrap();
        let started:Value=parse(&fs::read(dir.join("collection-start.json")).unwrap()).unwrap();
        let result:Value=parse(&fs::read(dir.join("collection-result.json")).unwrap()).unwrap();
        assert_eq!(started["status"],"IN_PROGRESS_UNKNOWN");
        assert!(started["startedUnixMs"].as_u64().is_some_and(|time|time>0));
        assert_eq!(result["status"],"UNKNOWN");
        assert_eq!(result["collectionError"],json!(collected.as_ref().err()));
        assert!(result["collectionResult"].is_null());
        assert!(!dir.join("collected-calibration.json").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}

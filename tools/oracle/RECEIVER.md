# Optional original-job evidence receiver (#336 / #330 / #45)

This native tool belongs only to the isolated `tools/oracle` workspace. It does
not launch an official server, JVM, Docker, systemd, parser or hosted workflow.
Source publication is not runtime admission. No original official job, valid
calibration cohort, mapped JVM observation or confirmed original-CID teardown is
claimed by the committed synthetic tests.

## Inputs and invocation

After ROOT separately admits the exact reviewed producer/workflow/source and
actual original jobs, the receiver operator provides **independent ROOT grants**:

```text
ktlsp-oracle-receive ROOT-receiver-admission.json ARTIFACT_ID ROOT-calibrations.json
```

The first file is strict `ReceiverAdmission`: `{schemaVersion:1,
scope:"PR174_RECEIVER_ADMISSION",grant:<RootGrant>,uploaderStep:{name,number}}`.
`ROOT-calibrations.json` is an array of ten objects `{ "admission":
<ReceiverAdmission>, "artifactId": <positive integer> }`, one per independently
admitted calibration case. `RootGrant`, `CalibrationReport` and `ExportBinding`
are the shared `authority` module's strict schemas, not receiver-owned copies.
An expected grant or uploader step obtained inside the artifact is not independent
authority. No artifact-provided program, URL or command line is executed.

Original retrieval uses root-owned, non-symlink `/usr/bin/gh`, `GH_TOKEN`, and
fixed GET endpoints on `github.com` for this repository. It independently joins
the completed original run/attempt, numeric original job and attempt-specific
job membership, head/tree/suite/workflow, exact workflow bytes and immutable
artifact ID/digest. More than 100 original jobs refuses instead of silently
accepting a partial page. Artifact names are irrelevant. GitHub's artifact API
has no job/attempt proof: matching self-declared binding fields and job membership
alone are insufficient. The receiver requires an independently ROOT-admitted
original post-upload step and exactly one strict receipt for the **target artifact
ID** in authenticated original **job logs and that exact step's logs**. Its
timestamp and serialized bytes must match; the timestamp must lie inside the
step's actual API interval at its reported second precision. Distinct receipts
for other artifacts/steps are allowed, so the same-job ten-case cohort works.
The [current GitHub step-log API](https://docs.github.com/en/rest/actions/workflow-jobs#download-step-logs-for-a-workflow-run-job)
takes the zero-based position, derived from authenticated ordered, unique
job-step records; ROOT still binds the step name and numeric API identity.
An unavailable endpoint, absent/truncated/duplicate-target/wrong-step receipt
refuses.

Cancellation calibration is a runner-enforced one-minute **step** timeout in the
same completed original job/VM, with independently admitted step name/number,
original job-step facts and the exact runner timeout log. A canceled whole job,
exit 130, child-written timeout text, generic API failure or report boolean is
not a substitute. Missing/unavailable original logs refuses authentication.
Protected workflow custody includes not echoing untrusted child output as
runner diagnostic lines. No source-to-binary cryptographic attestation is
claimed: reviewed workflow and protected original-job OS-root custody remain
explicit assumptions.

Producer/collector callers can reuse
`receiver::validate_step_timeout(&RootGrant, &Value, job_logs, step_logs) -> Result<u64>`
for the exact granted cancellation step and its completion Unix milliseconds.
This is bounded data validation, **not authentication by itself**: callers must
independently retrieve the original authenticated APIs and both log streams.
It binds job/run/attempt/head and rejects whole-job cancellation/failure.
The [runner's accepted outcome/conclusion contract](https://raw.githubusercontent.com/actions/runner/main/docs/adrs/0274-step-outcome-and-conclusion.md)
distinguishes pre-continuation `outcome=failure` from final/API `conclusion=success`.
The [timeout implementation](https://raw.githubusercontent.com/actions/runner/main/src/Runner.Worker/StepsRunner.cs)
applies `continue-on-error` before completing the step; the helper does not
reinterpret a failed/canceled API conclusion as this required continued timeout.

The fixed ROOT-owned post-upload step emits one compact serialized line per artifact:
`KTLSP_PR174_PROTECTED_UPLOAD ` followed by `ProtectedUploadReceipt` JSON:
`schemaVersion:1`, `scope:"PR174_PROTECTED_UPLOAD"`, repository/runId/jobId/suiteId/
attempt/sourceHead/sourceTree/workflowSha256, artifactId, artifactApiDigest,
optional downloadZipSha256, exportBindingSha256, nonce/containerId,
rootGrantSha256 and uploaderStep `{name,number}`. `rootGrantSha256` hashes actual
`root-authorization.json` bytes, not an invented canonicalized document.
The receiver independently downloads and hashes the ZIP; equality with the
authenticated API's SHA-256 digest is **observed and required**, never assumed.
An optional workflow-claimed download hash is checked separately. ROOT owns the
real original upload/receipt wiring; a caller-authored receipt or a copied log
file is never accepted through the authenticated retrieval constructor.
The reviewed workflow must not echo child-controlled bytes into protected logs.

## Protected export contract

`export-binding.json` is a strict camelCase `ExportBinding`:

- `schemaVersion: 1`, `scope: "PR174_PROTECTED_EXPORT"`;
- `run`: repository/event/attempt/runId/jobId/suiteId/sourceHead/sourceTree;
- exact nonce/full containerId/bootId/daemonId/image/imageId;
- rootAuthorizationSha256/calibrationSha256/workflowSha256;
- `files`: exact unique `{path,bytes,sha256}` inventory of **all other** members.

The binding cannot inventory itself. The authenticated ZIP digest seals its
actual bytes; calibration `exportSha256` hashes these binding bytes, not ZIP.
All inventory members are independently size-checked and SHA-256 hashed.
Official exports need the actual Admission/ROOT grant/calibration/workflow,
protected cleanup binding/process identity/arm/START/cleanup clock receipts,
preflight identity/preflight/authorization, observer and cleanup evidence, final
teardown receipt, six protocol capture files, protected host
`child-confinement-profile.json` and original `capture/child-confinement.json`.
Missing failed finals remain missing.
The shared typed host measurement must match the actual independent pre-START
ABI/profile observation, exact admitted source/template and boot/daemon/nonce.
The shared recorder `Receipt::validate_enforced` must accept the exact production
policy and measured nondumpable parent/ABI/enforcement; its original parent PID
is joined to the actual recorder's namespace PID. File/manifest/metadata receipts
must agree. Mere 0600/0700 permissions do **not** isolate same-UID JVM descendants;
post-closure copying/hashing alone cannot authenticate original child exit or raw
bytes. Official success requires the real shared child-only production custody
hook and authenticated same-profile Rust denial calibration, not a claimed flag.
Absent/unsupported/refused/unmeasured custody never grants official success.

Forced calibration cases require bound actual live-original-init action evidence
plus ordered successful exact-CID kill, wait137 and stopped inspection with no
OOM/restart. Workflow-step timeout additionally requires actual outside-step
service action observation after the authenticated original step completion time.
Cleanup independently joins original arm/clock and actual systemd service start,
the 60-second clamp, 12-second teardown reserve and 2-second completion reserve.
Unfinished, prematurely dead or partially exported cases cannot pass calibration.

Only bounded stored/deflate ZIP is accepted (32 MiB archive/expanded total,
8 MiB/member, 48 members). ZIP64, encryption, comments, multidisk, unsafe or
duplicate paths, links, overlapping/hidden sections, wrong local headers,
descriptors, CRCs and decompression tails refuse. Tar input is refused without
extracting it. Nothing from an archive is written into a filesystem tree.
Each fixed network command has a byte cap and 30-second deadline.

## Separate verdicts

The JSON report keeps provenance, transport, runtime, teardown and calibration
verdicts distinct; no missing/truncated/untrusted fact is interpreted as success.
Transport independently walks #331's original directional chronology, exact
raw frame offsets, fully persisted and OS-accepted client bytes, decoded server
bytes, EOF, normal original exit and original limits, then reuses
`Evidence::parse` for the existing replay contract. It does not reconstruct
ordering from separate directional arrays. An OS-accepted write is not proof
that a peer consumed every byte, especially an exit notification.

The official route requires the frozen unchanged client script, release
`ILS-263.6379.0`, document-symbol capability, UTF-16, actual valid IDs 2/3
`DocumentSymbol[]` or `SymbolInformation[]` or genuine null, and shutdown-null.
UTF-16 ranges use `ktlsp-text`, including surrogate-interior rejection and range,
selection and parent containment. Null and empty arrays remain valid protocol
observations but never useful symbol/parity proof. `officialSuccess` additionally
requires useful symbols, actual mapped sealed JVM/original-process/runtime
receipts, normal original exit 0 without OOM/restart/forced termination,
authenticated calibration originals and confirmed exact original-CID teardown.

For non-evidence adapter mechanics only:

```text
ktlsp-oracle-receive --synthetic CAPTURE_DIRECTORY
```

This consumes the recorder's six exported files and always reports
`officialSuccess: false`. Deterministic Rust fixtures deliberately fabricate
identities and protocol records; an executable Rust-peer test exercises the
actual recorder export → receiver → `Evidence::load` → existing replay path.
Those mechanics do not authenticate GitHub origin, containment calibration or
official behavior. No default native graph or workflow is changed here.

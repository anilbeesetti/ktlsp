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
ktlsp-oracle-receive ROOT-expected-grant.json ARTIFACT_ID ROOT-calibrations.json
```

`ROOT-calibrations.json` is an array of ten objects `{ "grant": <RootGrant>,
"artifactId": <positive integer> }`, one per independently admitted calibration
case. `RootGrant` and `CalibrationReport` are the shared `authority` module's
strict schemas, not receiver-defined aliases. An expected grant obtained from
inside the artifact is not independent authority. The receiver never executes
an artifact-provided program, URL or command line.

Original retrieval uses root-owned, non-symlink `/usr/bin/gh`, `GH_TOKEN`, and
fixed GET endpoints on `github.com` for this repository. It independently joins
the completed original run/attempt, numeric original job and attempt-specific
job membership, head/tree/suite/workflow, exact workflow bytes and immutable
artifact ID/digest. More than 100 original jobs refuses instead of silently
accepting a partial page. Artifact names are irrelevant. GitHub's artifact API
has no job/attempt proof: the required separate join is the original-job API
facts **and** the protected export binding produced by the reviewed original
workflow outside the child-writable capture. GitHub transport authentication
alone cannot prove which process authored arbitrary self-declared metadata.

Cancellation calibration is a runner-enforced one-minute **step** timeout in the
same completed original job/VM, with independently admitted step name/number,
original job-step facts and the exact runner timeout log. A canceled whole job,
exit 130, child-written timeout text, generic API failure or report boolean is
not a substitute. Missing/unavailable original logs refuses authentication.
Protected workflow custody includes not echoing untrusted child output as
runner diagnostic lines. No source-to-binary cryptographic attestation is
claimed: reviewed workflow and protected original-job OS-root custody remain
explicit assumptions.

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
teardown receipt and six capture files. Missing failed finals remain missing.

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

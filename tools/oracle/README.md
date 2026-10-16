# Committed-observation replay (T045a / #59)

This optional standalone Rust workspace replays decoded official JSON observations against a supplied stdio executable. It is outside the root Cargo workspace, native default tests, CI and shipped server. It reuses `ktlsp-protocol` and the repository's existing pinned serde versions; it downloads/runs no official distribution, Java, Gradle or Maven.

From repository root:

```sh
cargo build --locked -p ktlsp-server --bin ktlsp
cargo build --locked --manifest-path tools/oracle/Cargo.toml --bin ktlsp-oracle-replay
tools/oracle/target/debug/ktlsp-oracle-replay docs/research/oracle/lifecycle-folding "$(git rev-parse HEAD)" -- ./target/debug/ktlsp > /tmp/comparison.json
```

Exit 0 means exact comparison match; 1 means recorded differences; 2 means invalid evidence, process/I/O failure or unsupported interaction. The revision argument is caller-declared, not inferred from the executable. Preserve executable/input hashes alongside published reports (see comparisons README). Commands execute directly without a shell; use only the intended executable. The child inherits the caller's environment/current directory.

Fixture directory requires `transcript.json` directional `{direction,message}` records and `metadata.json` with release, archive SHA256 and recorded integer returncode. Original fixture files remain unchanged. Both committed observation directories are supported. Client requests must be sequentially answered in the evidence; interleaved client messages, duplicate/unknown IDs, server requests, malformed JSON-RPC and absent exit evidence are rejected. This runner cannot reconstruct omitted exchanges or interact with dynamic server requests.

No normalization is applied: IDs, original URIs, error codes/severities, ranges, edits, completion ranking and array order remain intact. JSON object key order is irrelevant. The original URIs work for native in-memory `didOpen`; this does not create the original Linux workspace on macOS. Responses include exact expected/actual JSON and JSON Pointer differences, distinguishing absent fields from null. Notifications compare the entire captured sequence separately; logs include machine-specific values and differences can be environmental. Recorded exit status is compared separately. Missing/additional notifications do not prove incompatibility outside the incomplete original capture; shutdown-tail completeness is explicitly unverified.

Defaults: 5 seconds per write/request/exit phase, 30 seconds per session; 8 MiB each input file and aggregate framed stdout, 1,024 fixture/stdout messages, 64 KiB stderr. Existing framing limits also apply. Concurrent I/O workers keep blocked stdin and partial stdout reads under the controller deadline. Every return path kills/reaps the direct child; no unbounded I/O thread join is attempted. `shortcut: cleanup covers the direct child only; add platform process-tree supervision before using launchers that spawn descendants.` Descendants can survive and retain pipe-worker threads until they close inherited handles; invoke the actual native server executable. This is a research tool, not a sandbox for arbitrary executables.

Optional tool checks (independent from default native checks):

```sh
cargo fmt --manifest-path tools/oracle/Cargo.toml -- --check
cargo clippy --locked --manifest-path tools/oracle/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path tools/oracle/Cargo.toml
```

`replay-test-peer` is a Rust-only subprocess fixture for these tool tests. No compiler/oracle is invoked. The first baseline publishes genuine initialize/folding/notification differences and matching shutdown/exit. T045 remains open for live isolated official capture, richer corpus, dynamic interactions, portable project materialization, semantic/performance parity and process-tree supervision.

## Synthetic-only raw recorder (T045c / #91)

`ktlsp-oracle-record` adds one sequential scripted session with raw evidence. It is separate from replay; replay retains its documented direct-child cleanup limitation. No dependencies, lockfile, native workspace membership, shipped capability or default CI graph are changed. The optional `oracle-tools.yml` workflow actually tests this standalone workspace on Linux and macOS using Rust peers only.

```sh
cargo build --locked --manifest-path tools/oracle/Cargo.toml --bins
tools/oracle/target/debug/ktlsp-oracle-record --synthetic-only \
  tools/oracle/fixtures/synthetic-recorder/script.json /tmp/FRESH-capture-directory -- \
  "$PWD/tools/oracle/target/debug/record-test-peer" fragmented
```

The output directory must not exist (parent must exist); collisions fail without overwriting. Exit 0 means complete capture, including a nonzero child exit; 2 means incomplete capture or invalid input. A response need not match any expected value. Script input must be a regular file (symlinks, directories, devices and FIFOs are rejected; Unix opens are nonblocking and opened metadata is rechecked). Script is a bounded JSON array of client methods with unique integer/string request IDs and a final `exit` notification. Requests wait for their exact response before sending the next message. Notifications are retained; unexpected server requests, malformed JSON-RPC, unknown/duplicate response IDs, invalid frames and incomplete EOF cause controlled failure. There is no dynamic request handler or semantic normalization.

`client-intended.raw` is persisted before each write. Manifest `clientFrames` contains exact half-open persisted raw offsets, `persistedBytes`, original `intendedLength`, and `acceptedBytes` for the prefix acknowledged by the pipe. A partial evidence write maps its persisted prefix and sends none of that frame. This proves bytes accepted by the OS, not peer consumption. `stdout.raw` and `stderr.raw` are written before interpretation; decoded `stdoutFrames` refer to half-open byte offsets in stdout.raw. `undecodedStart..undecodedEnd` locates the retained malformed/partial tail; interpretation stops on the first failure and cleanup may retain additional raw tail. On storage failure, stream `bytes`/tail offsets describe the persisted prefix; `observedBytes`, `unpersistedBytes` and `storageFailed` explicitly record any read bytes that could not be saved. These raw files are authoritative; the manifest never substitutes lossy UTF-8 for bytes.

Defaults: 5-second phase, 30-second session, 1-second cleanup; at most 8 MiB intended stdin and stdout each, 64 KiB stderr, 1,024 script/decoded messages. Library limits can lower these (deadlines have validated hard maxima). Reads/writes have an 8 KiB quota per 2 ms tick, decoding at most 32 frames per tick. A capped stream preserves one extra observed byte and marks truncation, then stops reading; EOF may remain unconfirmed. Remaining peer bytes cannot be captured beyond the explicit cap. No unbounded thread/join/pipe wait occurs. Deadlines assume responsive local filesystem and process-spawn operations; they are not hard real-time filesystem guarantees. Fresh directories are private, environment is cleared, and HOME/config/cache/state/tmp plus cwd are fresh capture-local roots. This is isolation from accidental user state, not a security sandbox.

The initial incomplete manifest exists before launch. Final manifest records script completion, limits/failure, raw byte counts/truncation, direct-child exit code/signal, stdout/stderr EOF, pre-cleanup facts, process-group termination and any cleanup error separately. Spawn/setup/storage failures retain incomplete evidence; a storage failure cannot claim completeness. Interrupted captures can leave only the initial manifest, with acknowledged write counts unknown. Files are flushed to the OS, not fsynced against power loss.

Supervision is explicitly Linux/macOS-only (other platforms fail before creating output or launching). Each child starts its own process group. All return paths attempt SIGKILL on that group, including after direct-child exit; normal completion also kills any same-group descendant that closed its pipes. Cleanup drains within its own deadline and observes/reaps the direct child without blocking. Descendants that escape into another group/session are outside containment, and grandchildren are reaped by their OS parent/init. Tests independently observe a descendant heartbeat stopping and pipe EOF after group termination. No claim of a general process sandbox or Windows support is made.

**Official live execution remains blocked.** This child ran synthetic Rust peers only. A later separately admitted gate must authenticate publisher archive checksum/provenance, validate launcher-specific containment (including escaped groups), review isolation/arguments and admit actual official capture. An existing local official installation does not authorize this recorder to launch it. T045 remains open; no new official diagnostic or feature-parity evidence is produced here.

## Optional PR174 host cleanup ownership (#321)

`ktlsp-oracle-pr174-host` is a Linux-only, root-custody controller; its presence does not authorize provisioning, dispatch or official execution. The official adapter and separately admitted original-job/containment prerequisites remain required. macOS runs synthetic tests only.

Cleanup service ownership transfers only after exclusive unit creation, daemon reload and policy readback succeed. A failed installation retains direct prearm cleanup of the exact known container, even when a partial or preexisting unit file remains; unrelated files are never overwritten or removed. Direct cleanup remains forbidden after watchdog/START ownership. Failed file writes retain cancellation tombstones so persistence failure cannot reopen START.

The supervisor records a protected `cleanup-service-ready` marker bound to nonce, full container ID and boot identity. External `request-cleanup` always latches cancellation, but starts the service only after validating that marker; before transfer it returns `CLEANUP_LATCHED` without starting any unit. If marker publication fails after verified installation, the supervisor retains service ownership in memory and requests its usable service before returning the capture error.

After OS-root and protected nonce-directory admission, `request-cleanup` writes its cancellation tombstone **before** requiring `binding.json`. A missing binding returns `CLEANUP_LATCHED`; an existing invalid/untrusted binding returns an error after latching, never an invented cleanup identity. Existing cancellation objects, including dangling symlinks, are preserved and exclude START. Later binding publication cannot clear early cancellation.

Run the complete optional workspace to build the standalone Rust command peer and expose the deterministic smoke output:

```sh
cargo fmt --manifest-path tools/oracle/Cargo.toml -- --check
cargo clippy --locked --manifest-path tools/oracle/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path tools/oracle/Cargo.toml -- --nocapture
```

The executable regression uses real production installation, final routing and direct cleanup against file-backed Rust command peers, not Docker/systemd or echo scripts. Write, sync, reload, readback-command, readback-policy, preexisting-file and symlink failures each return failure and remove the synthetic exact-CID container, reporting `owner=DirectPrearm containerPresent=false`. Successful installation and marker-publication failure select `owner=Service`, with dispatch recorded but no claimed service execution. A test-only custody callback permits valid/malformed marker paths without OS root; production always enforces protected custody. Receipts remain `UNKNOWN` and `officialCaptureSuccess=false`; this is not containment, fresh official capture, abrupt-death recovery or protection against privileged concurrent mutation.

### Change record

- #321: fixed PR306's file-existence ownership gap and migrated external cancellation routing. Local macOS format and strict all-target Clippy passed; optional tests passed with 19 host, 1 peer-consumer, 9 recorder and 6 replay tests, plus two ignored subprocess helpers exercised by their parents. The first actual-path run exposed `clock_gettime(1)` failing on macOS; selecting Darwin's `CLOCK_MONOTONIC=6` fixed it while Linux retains `1`. Hosted checks and root integration are separate evidence, not implied by these local results.
- #324 optional-tooling union: integrated the single verified main `2e408cfd8d65c68a9eede863e3f983a2db720d95` and PR300 `9318ffbe1ab97a386cf599ae49310e8c036616a6` with corrected PR306/323. At code head `04ccc2e78ad1a35ac42da873e75358d0da8ba44d`, independent functionality/adversarial integration reviews cleared, then local optional format, locked strict all-target Clippy and full synthetic tests passed once: 19 host, 1 peer-consumer, 6 PR174 validation, 9 recorder and 6 replay cases, with two ignored helpers exercised by parents. The existing executable smoke again removed the file-backed container for all seven installation failures; service cases proved dispatch only. This is reviewed/passing optional preparation, not aggregate hosted/exact-main proof or live execution admission.
- #324 early-cancellation correction: ROOT's union review found that the real CLI previously loaded the binding before reaching the latch helper. Code head `eec10ebbd486858bf9d5bc17370158c948e10b60` corrects that boundary. After three independent changed-path source rechecks, optional format, locked strict all-target Clippy and the full synthetic suite passed once: 20 host, 1 peer-consumer, 6 PR174 validation, 9 recorder and 6 replay cases. A real Rust subprocess invokes the shared post-admission CLI dispatcher for missing binding during preflight/preparation/CREATE, existing invalid binding, preexisting cancellation bytes and a dangling cancellation symlink; all six `EARLY_CANCEL_SMOKE` observations report `startAllowed=false`, `serviceDispatched=false` and `officialCaptureSuccess=false`. The subprocess bypasses OS-root/nonce-directory admission only; production admission is unchanged. The existing installation-failure smoke also passed. No pre-fix runtime execution, actual service launch or containment proof is claimed.

## PR174 input validation (source only)

`recorder::validate_pr174_inputs(identity_path, preflight_path, fixture_directory)`
checks bounded proposed-run and preflight data for the two unchanged
[PR174 fixtures](fixtures/official-pr174-symbols/README.md). Consistent inputs
return `Ok(())`; invalid inputs return an error. It creates no evidence and
spawns no process. This success is data validation, never launch authorization.
The validator alone supplies no execution or recording authority.

Both JSON inputs are regular, nonsymlink files limited to 16 KiB; source/script
files must exactly match embedded bytes, including UTF-8/newlines. The proposed
identity schema pins the preparation contract and source baseline, repository,
workflow-dispatch attempt 1 and finite original-run identifiers. Preflight must
repeat that identity and fixed release/hash claims, command, limits and output
path. Test source provides deliberately fabricated schema examples, never run
evidence. Unknown or duplicate top-level, identity, pin and limit fields are refused.

This checks data consistency only. Matching supplied hashes do not authenticate
the publisher, actual executable bytes, run, permissions or authorization. The
validator supplies no trusted host handoff, effective isolation, runtime attestation
or actual teardown evidence. The separate optional controller source above does
not establish those live facts. Live recording additionally requires the separately
admitted adapter/bootstrap, execution admission, real containment calibration and
independent review. Existing `record()` and `--synthetic-only` retain their scope,
environment, limits and raw/error/cleanup behavior. The validator remains data-only;
the optional producer below never enters the default native graph.

## Protected original-job workflow producer (#352 / #330 / #45)

`.github/workflows/t045-pr174-official-symbols.yml` is manual, main-only, attempt
one, with exactly one standard `ubuntu-24.04` job. It is source preparation,
not permission to dispatch or evidence that its platform prerequisites hold.
The job token has only `contents:read` and `actions:read`; it cannot post ROOT
approval. Independent repository-owner comments are authenticated through the
GitHub API by numeric owner identity as well as login and owner association.
Matching two caller-authored grants, a shape validator, a supplied hash or a
source approval never authorizes a calibration child or official JVM.

The workflow first builds the three optional native tools under the pinned
toolchain/lock with Rust wrappers/flags cleared, then calls `workflow-source`
before any privileged installation/preparation. It compares actual context,
checkout head/tree, original API run/job/suite and exact authenticated workflow
bytes. All three comment IDs are distinct, positive and predesignated, including
the unused official comment when `official=false`.

### Independent comment schemas

Bodies are one strict bounded JSON object (16 KiB maximum), without Markdown,
duplicate/unknown fields or trailing input. `run` is the shared `OriginalRun`:
repository, event, attempt, runId, jobId, suiteId, sourceHead and sourceTree,
all bound to the actual original job. No example numeric identities are grants.

The source comment is `WorkflowSourceAdmission`:

- `schemaVersion:1`, `scope:"PR174_SOURCE_ADMISSION"`, `run`;
- `workflowSha256` of actual immutable workflow bytes;
- `contractSha256` equal to
  `65eb16c5bd73c99fe7fc000fddf0242bf6abab09283676612edf5ce9f285b61d`;
- `image` equal to
  `ubuntu@sha256:f610ab94648195aa356059f5b41d6085c9d4d903c072430cdd1af7bdb646106b`;
- `containmentTemplateSha256` from the exact native `describe-template` source;
- `measuredFutureFacts` containing exactly `boot`, `daemon`, `host`,
  `image-config`, `native-binaries`, `tools` (order is immaterial);
- positive `calibrationComment` and `officialComment` matching the fixed inputs.

This explicitly authorizes measuring future facts, not inventing their values,
creating a container, launching a service or executing a JVM. Source approval
must already match the original API identities when the source step runs.
The privileged creator repeats authentication before preparation.

The creator streams the exact publisher archive under finite size/time bounds,
checks the unchanged archive/inventory/fixture pins, measures real binary/tool
hashes, host/boot/daemon, actual image config and ABI/profile, and refuses piped
core policy. It starts no container/service. It seals a root-owned proposal at
`/etc/ktlsp-pr174/proposal/proposal.json`, uploaded as
`pr174-measured-proposal` before any child can execute.

The calibration and official comments use `WorkflowApproval`:

- `schemaVersion:1`, `scope`, `run`;
- `proposalSha256` of raw protected proposal JSON;
- `proposalArtifactId` and `proposalArtifactApiDigest`, joined to the
  authenticated original artifact and independently downloaded ZIP digest;
- `calibrationSha256`, `calibrationArtifactId` and
  `calibrationArtifactApiDigest` are all `null` for calibration approval;
- calibration `scope` is `"PR174_MEASURED_CALIBRATION_ADMISSION"`;
- official `scope` is `"PR174_SEPARATE_OFFICIAL_ADMISSION"` and all three
  calibration fields bind the later exact protected ten-case report bytes and
  original `pr174-calibration-report` artifact.

ROOT independently retrieves the original run/source/job/artifact bytes and
reviews the actual measurements before updating the designated owner comment.
The creator verifies identity/binding before installing ten distinct finite
per-case grants. It polls only the predesignated owner approval, for at most
15 minutes; API errors, mismatches or expiry refuse, without retrying a case.
No matching caller-provided expected/supplied pair can install authority.

### Native command and custody boundary

All `workflow-*` commands are fixed modes of `ktlsp-oracle-pr174-host`.
`workflow-source` alone is nonprivileged. Other modes require Linux OS root:

```text
workflow-source
workflow-prepare
workflow-approve PROPOSAL_ARTIFACT_ID
workflow-case CASE
workflow-cancel-start
workflow-cancel-wait
workflow-finish CASE
workflow-receipt CASE ARTIFACT_ID
workflow-collect
workflow-official CALIBRATION_REPORT_ARTIFACT_ID
```

The first two take the explicit original workflow context environment. Subsequent
modes use protected fixed request/token files, never artifact-provided
credentials. Source inputs/binaries are installed at fixed `/opt/ktlsp-pr174`
paths; state, grants and readable upload snapshots live under
`/etc/ktlsp-pr174`; actual unique native host admissions remain under
`/run/ktlsp-pr174/NONCE`. The creator reuses bootstrap, protected file/hash
helpers, canonical grant schemas and existing eight host tools. No arbitrary
executable, image, flags, hash override or parser plugin is accepted.

Each grant is independently approved by the measured proposal before installation.
The existing exclusive grant claim and OS-random nonce remain authoritative.
The creator reserves each case and protects its actual nonce before the
outside-step service can launch, so interruption cannot erase the join.
Every host authority gate joins the exact derived grant with the approved
proposal and actual same boot/daemon before CREATE/START.
Later START is blocked by a missing protected upload receipt, incomplete
original closure or failed/incomplete ordinary owned-input disposal.

The nine ordinary finite cases each have their own protected artifact and
unique fixed `PR174 receipt CASE` step. Cancellation preparation first proves
the actual original peer is active. Only then does the independently named
`PR174 runner timeout calibration` step run a finite foreground wait, with
runner `timeout-minutes:1` and `continue-on-error:true`. Following `always`
collection waits for the real outside-step action and closure. It never emits
a timeout marker, self-kills, restarts, uses Docker exec or replaces the job/VM.
Unknown failures preserve only protected host/creator facts, not mutable child
bytes or fabricated finals, through the always artifact upload.

`workflow-receipt` independently downloads the uploaded artifact, checks its
actual API digest and all protected inventory bytes, then emits exactly one
canonical `KTLSP_PR174_PROTECTED_UPLOAD ` receipt from native code. It binds
the API-discovered currently running named step, actual artifact ID, raw grant
and export-binding hashes, nonce/CID and original identities. Child bytes are
never echoed into trusted job/step logs. Upload precedes receipt publication;
ordinary owned-input release occurs only afterward and never clears evidence.

`workflow-collect` requires all ten actual same-host exports, genuine completed
STEP timeout proof from authenticated original job **and exact step** logs,
and a later protected outside-step live-original action. It uses the canonical
receiver timeout validator, not a report boolean or whole-job cancellation.
The outer job may still be running. Missing API/logs/custody/closure yields
UNKNOWN and no official START. The final receiver separately requires the
completed successful original outer run/job and independently admitted upload
step identities; a pre-START gate never pretends the running job is completed.

When separately requested, `workflow-official` authenticates an additional owner
approval binding the actual collected raw report SHA and its uploaded bytes.
It uses the same boot/daemon/image/template and a new finite official grant,
not calibration permission. The official export has its own protected upload
and receipt. Actual image dependency sufficiency, privileged host preparation,
same-profile ten-case runtime, original log availability, authentic upload
custody, exact-CID closure and official JVM/runtime/transport/symbol evidence
remain real runtime prerequisites, not results established by this source.

Change record: #352 adds only the optional manual workflow/native producer and
unchanged pinned inventory data. No dispatch, privileged host/container/JVM
execution, official evidence or native/default dependency change is implied.

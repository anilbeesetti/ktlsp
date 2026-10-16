## Summary

**PR300 supplies validation-only inputs; PR306 supplies the host-side consumer, but the official Rust adapter is absent.** The accepted design already specifies a small adapter reusing the existing bounded recorder—not a new protocol framework.

The remaining execution prerequisites are **protected original-job bootstrap/authorization, an admitted immutable image/workflow, and actual same-configuration containment calibration**. Repository source and existing source/CI receipts cannot establish those facts. The locally retained archive/provenance research **does exist**; downloading or rediscovering the release is not the blocker.

Read-only investigation only: no edits, children, builds, tests, dispatch, or product execution. The known host cleanup-ownership P2 remains ROOT-owned and was not duplicated.

## Files and source identities

Abbreviations:

- **A** = `/Users/anil/.codex/worktrees/8f96/ktlsp`
- **H** = `/Users/anil/.codex/worktrees/t045-pr174-host-supervisor/ktlsp`
- **C** = `/private/tmp/ktlsp-t045-pr174-symbol-capture`
- **P** = `/private/tmp/ktlsp-t045-host-supervisor-plan`

| Path | Relevant contents |
|---|---|
| `A/tools/oracle/src/recorder.rs:9-209` | `CaptureLimits`, public `validate_pr174_inputs`, private strict identity/preflight schemas and bounded nonsymlink input reader. |
| `A/tools/oracle/src/recorder.rs:213-730` | Existing real nonblocking duplex capture, raw persistence, framing, sequential IDs, deadlines, caps, process-group cleanup, initial/final manifest. |
| `A/tools/oracle/src/bin/ktlsp-oracle-record.rs:11-86` | Existing **synthetic-only** CLI; not the official entrypoint. |
| `A/tools/oracle/fixtures/official-pr174-symbols/script.json:1-101` | Exact eight-message session; no dynamic replies or readiness requests. |
| `A/tools/oracle/tests/official_pr174.rs:51-84` | Fabricated schema examples explicitly labelled non-evidence. |
| `A/tools/oracle/src/lib.rs:39-183` | Existing replay consumer: `Evidence::load`, `Evidence::parse`, directional transcript format and metadata requirements. |
| `H/tools/oracle/src/bin/ktlsp-oracle-pr174-host.rs:82-155` | `Admission`, `OriginalRun`, `Tools`, `Binding`, `ProcessIdentity`. |
| Same file `:609-995` | Protected admission, archive/extraction verification, authenticated API identity checks and host preflight. |
| Same file `:1075-1238` | Exact launcher/adapter argv, environment, mounts, Docker entrypoint. |
| Same file `:1394-1435` | `seal_handoff`: exact identity, preflight and launch-authorization producer. |
| Same file `:1910-2051` | Actual-process attestation: recorder/launcher ancestry, argv, namespace, limits and mapped pinned `libjvm.so`. |
| Same file `:2811-2872,2883-3115` | Host observation/lifecycle and `supervise\|cleanup\|request-cleanup` entrypoints. Host deliberately does not authenticate child-written transport/symbol evidence. |
| `C/contract.json:276-423` | Frozen limits, containment, raw retention, success verdicts, implementation contract and execution gates. |
| `P/PLAN.md:46-82` | Accepted custody model, adapter responsibility, calibration cases and unresolved facts. |
| `C/provenance/` | Actual archive, adjacent publisher checksum/headers, sealed member inventory and static launcher/JBR research. |

**Recorded committed identities—not newly git-verified here:**

- [PR300](https://github.com/anilbeesetti/ktlsp/pull/300): head `9318ffbe1ab97a386cf599ae49310e8c036616a6`, tree `cbcb251e88cdb5c7ea06d2160d10cd8954997188`; receipt: `/private/tmp/ktlsp-handlers/t045/EARLY_COMMITTED_SOURCE_ACK_PASS.json`.
- [PR306](https://github.com/anilbeesetti/ktlsp/pull/306): head `05fc5ab7033f5bf5a5bb80f1a1e848879ee79b24`, tree `230bf86e3bf7c7e7c2802ad6546b0ad5d5174eff`; receipt: `P/COMMITTED_SOURCE_READY.json`.

These are separate inputs, not a claim that their union is integrated.

## Architecture

```text
separately admitted original-job bootstrap
  → protected Admission + sealed binaries/archive/extraction/image/workflow
  → PR306 root-owned host supervisor
  → Docker CREATE, inspect, seal handoff, arm watchdog, START
  → missing ktlsp-oracle-official-pr174 Rust adapter
  → existing bounded recorder loop
  → exact official ELF launcher / bundled research JVM
  → raw transport + manifest

host independently observes processes/runtime and performs teardown
  → independent authenticated receiver joins host receipts + child evidence
  → reviewed transcript/metadata
  → existing Rust replay consumer
```

The JVM stays exclusively in isolated research storage/execution. Neither adapter nor official distribution belongs in the product/native default dependency graph.

## Exact invocation and input contract

### Official adapter invocation

PR306 already invokes this executable as the container entrypoint:

```text
/tmp/ktlsp-t045-pr174-host/tool/ktlsp-oracle-official-pr174
  --identity /tmp/ktlsp-t045-pr174-host/preflight/identity.json
  --preflight /tmp/ktlsp-t045-pr174-host/preflight/host-preflight.json
  --authorization /tmp/ktlsp-t045-pr174-host/preflight/launch-authorization.json
  --fixture-directory /tmp/ktlsp-t045-pr174-host/fixture
```

Producer/callers: `adapter_argv()`, `create_argv()`, `recorder_argv()`, `config()` in the host source. **No corresponding adapter source exists in either inspected checkout.**

Its only permitted official child command is:

```text
/tmp/ktlsp-t045-pr174-host/distribution/kotlin-server-263.6379.0/bin/intellij-server
  --stdio
  --data-sharing=none
  --region=apac
  --system-path=/tmp/ktlsp-t045-pr174-host/capture/system
```

No shell wrapper, `java`, `jcmd`, arbitrary executable/output argument, or server-derived path.

### JSON schemas

All existing PR300 typed schemas reject unknown/duplicate fields. Identity/preflight are regular nonsymlink files, each bounded to **16,384 bytes**.

| Object | Exact keys / binding |
|---|---|
| `Pr174Identity` | `schemaVersion:1`, `contractSha256`, `sourceBaseline`, `originalRun` |
| `Pr174Run` | `repository`, `event`, `attempt`, `runId`, `jobId`, `suiteId`, `sourceHead`, `sourceTree`, `workflowSha256`, `containerId`, `imageSha256`, `nonce` |
| `Pr174Preflight` | `schemaVersion:1`, `identity`, `pins`, `command`, `limits`, `outputDirectory` |
| `Pr174Pins` | `release`, `platform`, `archiveSha256`, `launcherSha256`, `scriptSha256`, `declarationsSha256`, `errorNodeSha256` |
| Host-produced authorization | `schemaVersion:1`, `rootAuthorizationSha256`, `contractSha256`, `identity`, `preflightSha256`, `bootId`, `daemonId`, `authority` |

Important: authorization’s **`identity` is the flattened `originalRun` object**, not the `Pr174Identity` wrapper.

Fixed bindings:

- Contract: `65eb16c5bd73c99fe7fc000fddf0242bf6abab09283676612edf5ce9f285b61d`
- Historical `sourceBaseline`: `b4fa25286a5fa0d7600b0b0420a7bcae6d520edc`
- Repository/event/attempt: `anilbeesetti/ktlsp`, `workflow_dispatch`, `1`
- Positive run/job/suite IDs; lowercase hex head/tree 40 characters, other run digests/nonce 64.
- Output: `/tmp/ktlsp-t045-pr174-host/capture`, absent before launch.
- Authorization’s `preflightSha256` hashes the **actual preflight file bytes**, not reserialized JSON.

`validate_pr174_inputs(...) -> Result<(), String>` already checks identity consistency, exact pins/command/limits/output and exact embedded fixture/script bytes. **It neither reads authorization nor hashes/authenticates the executable or preflight.**

## Real duplex, timeout and manifest contract

The existing recorder already implements the necessary bounded transport:

- Concurrent nonblocking stdin/stdout/stderr progression, **8 KiB I/O quota per 2 ms tick**, at most **32 decoded frames per tick**.
- Sequential requests, exact response ID matching; notifications retained.
- Unexpected server requests, unknown/duplicate responses, malformed JSON-RPC/framing and incomplete EOF fail honestly. **Do not invent client replies.**
- Frozen official limits: `phaseMs=60000`, `sessionMs=300000`, `cleanupMs=5000`, `streamBytes=8388608`, `stderrBytes=65536`, `messages=1024`; framing header ceiling **8192 bytes**.
- Absolute session deadline; notifications cannot extend the session indefinitely.
- Initial incomplete manifest before launch; persisted raw before interpretation; cap overflow retains one detection byte.
- Child exit and stream EOF are separate facts. Cleanup is bounded; escaped process groups require the outer container boundary.

Files and existing manifest keys:

| Evidence | Contract |
|---|---|
| `client-intended.raw` / `clientFrames` | `start`, `end`, `intendedLength`, `persistedBytes`, `acceptedBytes`; half-open persisted offsets. Accepted bytes mean OS pipe acceptance, not peer consumption. |
| `stdout.raw` / `stdoutFrames` | Each decoded record has `start`, `end`, `message`. |
| `stderr.raw` | Bounded raw stderr. |
| `stdout`, `stderr` | `bytes`, `observedBytes`, `unpersistedBytes`, `storageFailed`, `eof`, `truncated`; stdout additionally `undecodedStart`, `undecodedEnd`. |
| `exit` | `observed`, `code`, `signal`. |
| `cleanup` | `before`, `forcedTermination`, `groupKillAttempted`, `error`. |
| Top level | `schemaVersion`, `scope`, `complete`, `failureReason`, `limits`, `scriptCompleted`, `elapsedMs`. |

**Missing official behavior:** `platform::run()` currently initializes `scope` to `"synthetic-only; official live execution blocked"`, and child setup supplies only synthetic private environment roots. Merely wrapping `record()` and changing the final JSON would violate the accepted contract.

Official mode must be selected **before the initial manifest/spawn**, preserving synthetic behavior, with cleared environment, private roots, and only fixed additions `PATH=/usr/bin:/bin`, `LANG=C.UTF-8`, `LC_ALL=C.UTF-8`.

## Provenance and trust

The real artifact is already retained at:

`C/provenance/kotlin-server-263.6379.0.tar.gz`

Publisher URLs and locally retained checksums/headers are recorded in `C/contract.json:140-236`:

- [Official archive](https://download.jetbrains.com/language-server/kotlin-server/263.6379.0/kotlin-server-263.6379.0.tar.gz)
- [Adjacent publisher checksum](https://download.jetbrains.com/language-server/kotlin-server/263.6379.0/kotlin-server-263.6379.0.tar.gz.sha256)

| Item | Bytes / SHA-256 |
|---|---|
| Archive | 368,488,700 / `ab8ca4455dc2fc5fe1a24db2bccc46c104254d2c465155c4251ee65df8f3f7cc` |
| Launcher | 1,073,600 / `adf02baec622b4fcd1e7db3ec96f7c7b3309d5ab59e170e60260d7511222878e` |
| Loaded-JVM target | `ba231c43718732491a965b742b54a1f9206c2415c61502311c0172b88d6c79d5` |
| Script | 2,351 / `930a87ea1fa9c3f6e6a6b6008d010dd9435082329ea8b897a42084d7369ab008` |
| declarations | 146 / `fa935b144a49838fd9e603bfc95462bcf3701eeb19877501d451ca069ce9e233` |
| error-node | 44 / `c052b78e7603a88f3531f5b06f3a533f8df5394ce8ce9eeab3eb2b3ce3f4bd2c` |

Frozen upstream public source revision: `4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748`. PR174 fixture origin: `44f77b9`.

The retained TLS/co-hosted checksum research **does not establish independent signing or source/binary correspondence**. Static launcher imports/strings do not establish actual loaded JVM or containment.

### Existing host trust versus missing authority

PR306 already:

- Requires Linux OS root, protected `/run/ktlsp-pr174/<64-hex nonce>`, root-owned non-writable ancestry and executable custody.
- Checks archive/member/extraction hashes; extraction inventory requires **778 regular files / 221 symlinks**, sealed inventory and no extra entries.
- Queries authenticated GitHub run/job/commit/workflow endpoints and matches original identities.
- Creates/seals read-only handoff files after CREATE, solving the CID/preflight-digest cycle.
- Observes actual recorder/launcher executable device/inode, parentage, argv, PID starttime, namespace/cgroup, zero core limits, and pinned mapped `libjvm.so`.
- Leaves `officialCaptureSuccess:false`; host observation is not a symbol/transport verdict.

But **`root-authorization.json` and `calibration.json` are opaque hashed inputs**, not internally interpreted permission/calibration schemas. The protected bootstrap must establish their meaning and authorization before creating `Admission`. Consistent caller JSON is insufficient.

The accepted custody model explicitly says copied root-owned files can pass filesystem checks elsewhere. Original-run authenticity comes from the admitted host START gate plus independent authenticated artifact receiver—not from adapter filesystem checks alone.

## Minimum real Rust adapter proposal

This follows the already accepted `contract.json` implementation contract; it is not a replacement plan.

1. **Add the missing auto-discovered bin** `tools/oracle/src/bin/ktlsp-oracle-official-pr174.rs`, accepting exactly the four fixed argument pairs above.
2. **Reuse PR300 validation**, then strictly validate the host-produced authorization, bounded custody/read-only mount/file identity, exact run projection and raw preflight digest. Fail before output creation/spawn on mismatch.
3. **Add one private official configuration route inside the existing recorder**, carrying validated official scope/identity/environment into the initial manifest and existing capture loop. Keep `record()` and synthetic CLI unchanged. No unconsumed abstraction or second I/O engine.
4. **Retain ordered directional transport events with raw references.** Current separate `clientFrames` and `stdoutFrames` do not encode their complete cross-direction chronology. Record client events when fully accepted and server events when decoded; preserve partial-write evidence separately. Do not manufacture a transcript later by guessing notification placement.
5. **Provide the actual evidence consumer:** bounded `transcript.json` records `{direction:"client"|"server", message:...}` plus `metadata.json` keys `release`, `sha256`, integer `returncode`, genuine provenance/identity. This feeds existing `Evidence::load` → replay. Its existing **8 MiB / 1024-record** admission must remain explicit; reject oversized export rather than silently truncate.
6. **Keep final official acceptance outside transport completion.** The authenticated receiver validates original artifacts, raw offsets/hashes, host/runtime/teardown receipts and symbol shapes before publication. Synthetic peers exercise adapter mechanics only.

**Digest implementation handoff:** the optional oracle package currently has no SHA implementation/dependency. The accepted contract permits platform SHA256, but does not pin an in-container hashing executable. Freeze that tool/path/image custody and bounded invocation before implementation, or obtain ROOT-approved dependency reuse. `ktlsp-artifacts::verify_sha256` exists but is not an oracle dependency; adding it is not a free/no-lock change. Do not introduce handwritten SHA.

## Missing facts / execution blockers

These are established from source, frozen contracts, handler/admission receipts and artifact discovery—not from failed web access.

| Missing prerequisite | Exact consequence |
|---|---|
| Real adapter and protected authorization consumer | PR306 cannot start its required entrypoint. PR300 intentionally omitted a dead launch-blocked CLI: `/private/tmp/ktlsp-handlers/t045/FIRST_SOURCE_SCOPE_REFINEMENT.json`. |
| Manual original-job workflow/bootstrap | `.github/workflows/t045-pr174-official-symbols.yml` is absent; existing workflow is synthetic PR/push testing only. No protected admission producer is implemented in inspected source. |
| ROOT execution authorization and actual calibration receipts | Existing grants are source/compile/publication grants, not official execution permission. Exact-name searches found no applicable `root-authorization.json`/`calibration.json` artifacts. |
| Immutable image digest and dependencies/locale | No accepted concrete image value; loader/JBR library sufficiency and locale availability remain unobserved. |
| Real host effective configuration | systemd/Docker/cgroup/security/mount/core-policy facts need original hosted measurement. Leading-pipe host core handler blocks START absent separate bounded admission; no sysctl workaround. |
| Same-configuration containment calibration | Actual same-group and setsid/setpgid descendants; normal, timeout, workflow-step cancellation, bounded storage exhaustion; denial of host PID/network/socket/input writes; surviving cleanup receipts. Unit/synthetic callbacks cannot prove these. |
| Runtime/closure/export proof | Actual timely loaded-JVM observation, daemon continuity, stopped-state/wait, original cgroup disappearance/emptiness, removal and typed healthy-daemon absence, authenticated original artifact custody. |
| Receiver/export implementation | Host intentionally does not inspect child-written symbols; no inspected implementation joins all evidence into the frozen official verdict. |

Official success additionally requires exact version `ILS-263.6379.0`, document symbols advertised, UTF-16 selected/default, IDs 2/3 containing valid `DocumentSymbol[]`/`SymbolInformation[]` or genuine null, shutdown ID4 result null, complete normal exit0 before watchdog expiry, and confirmed original-container teardown. Null/empty results remain honest observations, **not useful-symbol or parity proof**. No VM-destruction, lowering, semantic-parity or synthetic-as-official claim follows.
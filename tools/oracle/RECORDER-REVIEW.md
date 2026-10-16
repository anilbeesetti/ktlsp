# T045c / #91 recorder review evidence

## #331 protected adapter source

This source adds the fixed PR174 entrypoint and private official configuration
inside the existing recorder; it does not authorize an official launch.
Authorization consumes the host producer's flattened `originalRun` projection
and actual preflight-byte SHA-256, while PR300 validation still supplies strict
pins/schemas and exact embedded fixture bytes. The optional package reuses the
existing audited exact SHA2 graph and retained artifacts-crate notices.

ROOT's producer-compatible custody interpretation permits only the exact
UID/GID65532 mode0755 INNER tmpfs ancestor, with distinct root-owned read-only
private mounts at the four input roots and read-only image-root coverage above
it. Held-descriptor openat traversal and fdinfo/mountinfo identity checks reject
symlink, hidden/nested mount, type, link-count, ownership and digest substitution.
This is not independent run authentication: original host START and an
authenticated receiver remain mandatory. Separate hosted calibration must
prove the child cannot rename/remove/unmount/bind-over those roots or enter an
effective alternate mount namespace; no such runtime proof is claimed here.

Chronology is recorded at complete client OS-byte acceptance and server decode,
with references to retained raw frame tables. Borrowed directional export has
the existing Evidence consumer's 8 MiB/1,024-record ceilings. Incomplete/error/
budget/storage observations never become official acceptance. Synthetic peer
mechanics regressions use the real executable recorder→Evidence→replay path and
explicit nonofficial metadata. Linux custody tests require no official child,
Docker, systemd or mount mutation. Source-review and exercised-check receipts
are recorded separately on the source PR, not inferred from historical results.

Independent source review found three correctness edges: a matching response
could be decoded before the request was fully accepted; a malformed raw tail
could leave a replayable valid prefix; and applying the combined consumer count
inside transport changed synthetic limits. The recorder now fails premature
responses without changing actual chronology, withholds consumer files after
failed transport, and checks combined budgets at export only. Synthetic
`complete` remains transport-only, with separate `export.complete`; official
capture additionally requires export. Permanent Rust-peer regressions retain
each case's raw/acceptance/error facts. Independent functionality and adversarial
source rechecks cleared all three fixes with no remaining findings.

After those reviews, optional formatting and locked strict all-target Clippy
passed locally on macOS. The first full test attempt found a new assertion typo:
`bad-frame-tail` emits `malformed-tail`, not another peer mode's `raw-tail`.
Both reviewers source-rechecked the exact-byte correction, with no production
or limit change. The corrected full optional suite passed: **55 ordinary cases**
(7 library, 20 host, 1 host-peer consumer, 12 PR174, 9 recorder, 6 replay), plus
two ignored helpers actually exercised by their parents. Executable synthetic
Rust peer → recorder export → Evidence load → replay passed, including notification
placement, errors/nonzero exits, record/byte ceilings and premature responses.
No parser, JVM, Docker or systemd launch occurred. The two Linux-only custody
regressions and Linux compilation are hosted-check requirements, not local facts.

Original PR338 head `266843824194cf5826f86bdcd97cf152add07982` hosted run
`38857754048` passed Linux compilation, both custody regressions and all 12 PR174
mechanics cases, then failed the historical recorder test's incidental primary
error wording assertion. The actual retained observation was incomplete capture,
71 intended/0 accepted client bytes, child exit0, exact 23-byte partial stdout
and a BrokenPipe first failure. It is retained as a failure, not retried unchanged.
The optional macOS leg was cancelled by matrix fail-fast; it was not a Mac
product-test failure. Separate native PR run `38857754012` passed.

ROOT rejected changing production error precedence to satisfy wording. The
correction deletes that needle assertion and instead requires exact emitted raw
bytes, offsets/counters, client persisted/accepted bounds, observed exit and
cleanup, incomplete capture and absent consumer export. Malformed-frame peers
emit their bounded frame+tail in one pipe write so a userspace buffering race
cannot leave an un-emitted tail. Recorder production semantics and all existing
limits/deadlines remain unchanged; an uncommitted post-failure decoding proposal
was removed rather than shipped.
Both independent reviewers cleared the test/fixture correction. Its first strict
Clippy attempt caught the fixture's ABI-compatible but noncanonical `write`
pointer spelling; the declaration now uses `*const c_void` with an explicit
pointer cast, without lint suppression. Both reviewers rechecked that correction.
Final corrected-source optional format check, strict locked all-target Clippy
and full synthetic suite passed locally: the same 55 ordinary cases plus two
actually exercised ignored helpers, with no production/limit change.

At corrected head `076b3a1d77ccc3ae23326258f49c35affada61e3`, original hosted
optional run `38859058554` passed Ubuntu completely. macOS failed the new
aggregate-byte-budget subprocess test because capture was incomplete; its
assertion did not print the failure reason, so the log does not establish that
reason. No unchanged rerun or deadline inflation was used. The 4 MiB request/
echo fixture coupled a serialization-budget assertion to large-pipe scheduling.
It is replaced with a deterministic real exporter → filesystem → Evidence
consumer test: valid sequential evidence exactly at 8 MiB is accepted with
verified returned count/file length; adding only one ASCII byte on the same
filesystem publishes no consumer files and is refused. Each direction remains
individually below 8 MiB. The unused echo peer is removed. Actual small Rust-peer
recorder → Evidence → replay smoke and the live record-count boundary remain.
Production transport/limits remain unchanged. Current ordinary-case distribution
is 8 library, 20 host, 1 host-peer consumer, 11 PR174, 9 recorder and 6 replay
on macOS; Linux additionally has the two custody cases.
Both source reviewers cleared the exact boundary replacement. Its local format/
strict Clippy and boundary/PR174 cases passed, but the strengthened failure test
then exposed another new assumption: it required a successful group-kill flag
on every failed observation. The actual flag was false; the assertion did not
print the accompanying cleanup error. The test now checks truthful consistency:
a null cleanup error requires the successful flag; otherwise an explicit
nonempty cleanup error is required. Incomplete/forced/raw/EOF/exit/no-export
checks remain, and no clean-closure or suppressed-error claim is made.
Both independent reviewers rechecked the cleanup-fact correction clear. Final
corrected-source optional formatting, strict locked all-target Clippy and the
complete synthetic suite passed locally: 55 ordinary cases plus the two
actually exercised ignored helpers. All original failed observations above
remain separate evidence, not erased by this changed-source pass.

## Historical PR300 source-review checkpoint

PR174 source preparation adds a working input-consistency validator.
It checks bounded proposed identity/preflight data and exact embedded source/
script bytes, returning success or an error without output or process creation. Hash
claims remain unauthenticated until a separately reviewed supervisor handoff;
matching caller-provided values never grant execution. No official CLI is added.
Synthetic recording is unchanged. New focused test source covers consistent proposed claims,
missing/mismatched/unknown identity fields, fixed preflight/fixture mismatches,
input bounds/types, contradictory raw-JSON duplicate pin/limit fields and
explicitly synthetic Rust-peer recording. Correctness review found that nested
`Value` objects accepted last-wins duplicate keys; strict typed pins and existing
`CaptureLimits` now reject them before comparison.
Those new tests have not been executed locally under #96; historical execution
evidence below does not apply to this new slice. Source review and compile-only
validation receipts are recorded separately in its delivery handoff. No actual
official, Docker, host-controller, workflow or genuine-observation implementation
is delivered here.

## Historical synthetic-recorder delivery evidence

Scope: optional standalone synthetic-only raw recorder. No JVM/official launch/download, semantic observation, dependency/lock change, shipped provider or native-workspace graph change. Parent #45 remains open. Official live execution remains blocked by a separate admission/provenance/launcher containment gate.

Architect reviewed the plan, repeated test failures and finished design. Corrections included acknowledged stdin prefixes, fair bounded ticks, header completeness caching, independent exit/EOF, descendant readiness, EOF decoding backlog, direct stdout-byte-cap and actual CLI coverage. Early 350 ms fixture deadlines were sensitive to parallel startup; ordinary test phases are 1200 ms with separate deterministic short no-read deadlines. Production defaults remain 5/30/1 seconds (phase/session/cleanup).

Separate correctness/functionality reviewer independently inspected and exercised the implementation. Found P1: EOF completion could skip frames behind the 32-frame decode quota. Fixed completion to require all raw stdout interpreted; independent Rust peer emitting 100 post-stdin-EOF notices plus malformed tail now retains all 5734 raw bytes, decodes all 100 valid frames, rejects the tail and returns incomplete. Regression also covers post-exit server request and message-cap tails.

Separate adversarial reviewer independently exercised duplicate headers, zero JSON payload, UTF16 charset, partial header, duplicate response and same-group silent descendants. Found two P2s: partial evidence writes produced inaccurate raw counts, and FIFO script input could block before launch. Prefix-counted writes now map actual persisted stdout/stderr/intended bytes and explicitly report unsaved read bytes. CLI requires regular input, opens nonblocking on Unix and rechecks opened metadata. Independent RLIMIT_FSIZE=4096 probes verified raw stdout=manifest bytes/tail end=4096, observed8192/unpersisted4096/storageFailed=true; intended failure maps4096 persisted of10095 intended and accepts0. Independent FIFO/device/symlink probes reject immediately with exit2 before creating output. Changed behavior rechecked by both reviewers; final approvals recorded in PR delivery evidence.

Local macOS pinned Rust1.99.0 checks:

- `cargo fmt --all -- --check`: pass.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: pass.
- `cargo test --locked --workspace`: 137 active tests pass; two ignored subprocess helpers are exercised by parent tests. Existing artifact FIFO creation required approved sandbox escalation; initial sandbox denial was not a product failure.
- `cargo fmt --manifest-path tools/oracle/Cargo.toml -- --check`: pass.
- `cargo clippy --locked --manifest-path tools/oracle/Cargo.toml --all-targets -- -D warnings`: pass.
- `cargo test --locked --manifest-path tools/oracle/Cargo.toml`: 15 active integration tests pass, plus one ignored storage-limit helper exercised by its parent test.
- Actual CLI committed-script/fragmented-peer smoke: complete, child exit0, sent126 bytes, received67 bytes, exact decoded raw range0..67. CLI tests cover required flag, exit0/2, device/directory rejection.

Recorder tests additionally cover partial/malformed raw tails, blocked/unread stdin and acknowledged partial writes, phase deadline, stdout/stderr floods, valid-output byte cap with cap+1 retained, message cap, unknown requests, nonzero/signal exits, child exit independent from inherited pipes, descendant heartbeat termination on timeout and normal completion, evidence collision, and partial disk-write offsets via isolated process limits. No tests run Gradle/Maven or Java. Optional hosted workflow separately runs actual tool fmt/clippy/tests on Linux and macOS; hosted evidence and exact reviewed head are recorded on the PR, not inferred from local checks.

Residual limits: process groups cover descendants remaining in the group; escaped groups/sessions and Windows supervision are unsupported. This is not a security sandbox. Local filesystem/spawn responsiveness is assumed; no power-loss fsync guarantee. Captures stop at explicit caps (retaining one overflow detection byte), storage failures disclose unsaved read bytes, and interrupted manifests cannot infer acknowledged writes. Replay cleanup remains direct-child-only. No official parity claim follows from synthetic capture.

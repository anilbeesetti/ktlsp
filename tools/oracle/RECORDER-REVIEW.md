# T045c / #91 recorder review evidence

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

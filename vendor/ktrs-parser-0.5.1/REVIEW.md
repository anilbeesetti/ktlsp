# Historical source-local review evidence — #87

This records the source-local phase before #86 delivery and coordinator handoff.
The handoff is now complete; production checks/reviews/CI remain required. No JVM/oracle/Gradle/Maven or forbidden native dependency ran. Exact
source patch is recovery.patch; upstream-sha256.json fingerprints pristine source.

Architect plan review identified shared-loop contexts, terminal recovery consuming
siblings, annotation/modifier prefixes, token remaps and nested grammar. Explicit
function-list context, parameter-boundary checks and non-consuming diagnostics
address the original three counterexamples. Constructor/initializer boundaries
use guarded lookahead, preserving soft-keyword parameter names.

Independent functionality review ran 210 valid structural comparisons against
registry upstream, including 200 soft-keyword/annotation combinations. Independent
adversarial review ran 60 valid structural comparisons, 210 malformed cases and
1,000 repeated recovery points with original ranges/nearest parents/token partition.
Both found a P1 exponential modifier-probe replay on nested annotation anonymous
functions. Fixed with scoped probe suppression; both rechecked the exact cases,
valid tree equality and nested malformed recovery. No remaining source-local
blocker reported; both require production integration recheck.

Six source-local tests pass (cargo test --offline --locked --manifest-path
vendor/ktrs-parser-0.5.1/Cargo.toml --test recovery), checking original malformed
function boundaries, later names/kinds/nearest parents, diagnostics, annotation
prefixes, repeated recovery, constructor/initializer recovery and valid defaults.
EOF retains exact upstream diagnostics. The depth-20 annotation regression finishes without recursive branching; production
acceptance is constrained by unchanged 128 tree-depth limit.

Read-only production-source experiment: standalone /private/tmp/ktlsp-t007b2-worker
manifest (now preserved portably as worker-probe/Cargo.toml) uses ktlsp-parser path dependency, the unchanged production worker source
as its binary path, and a local crates.io parser patch. No production manifest or
lock changes. Two worker unit checks plus six existing parser integration checks
and two original test probes pass. Worker allocation-denial child was invoked by
its parent test; existing generated/crash/backtracking/coverage/limits/healthy-
followup gates passed. Original source recovery passed nine top/member/local x
LF/CRLF/lone-CR combinations with Unicode/backticks. Nested annotation depths
8/12/14/16 accepted at ~10/16/19/26ms on this busy macOS debug host; depths
20/30/40/50 explicitly returned NestingLimit at ~30/84/178/332ms, each followed by
a healthy parse. These are observed local checks, not a performance guarantee,
final integrated #86 API evidence, hosted CI, or product-graph delivery.

Residual recovery examples and version/full-grammar limits are in PROVENANCE.md.
Architect finished review approves the source-local phase/draft PR. Root fmt and
clippy pass on the unchanged product graph; full-workspace tests initially hit
sandbox-denied mkfifo in artifact tests, then passed with actual escalation:
137 active tests plus two invoked helper subprocess fixtures. Hosted Native Rust
CI passed source-local draft head 2534c83e783d8b4e4b2ea2b688a751c5e51c05cc:
https://github.com/anilbeesetti/ktlsp/actions/runs/38257790041/job/114840244988
The hosted root checks do not execute either standalone vendor test workspace.
Production pinned full-workspace checks, hosted CI and both integration
rechecks and expected-head coordinator merge grant remain pending.

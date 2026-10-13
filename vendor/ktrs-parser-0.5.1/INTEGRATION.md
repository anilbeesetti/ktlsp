# Production integration contract — #87

Coordinator transferred parser ownership after #86/PR92 delivery. The final
preparation integrated exact main758e53b17566e4fde908705fc090d516ba4b0065 once
at b3bdc97cd21c181582a31fdf541f3f3ca7c880d9, preserving the independently reviewed
formatting phase70bad3e and both source patches/inventories. Both independent
source-only integration reviews approve that merge.

Controlled validation uses existing Linux Native Rust CI for all three unchanged
pinned commands on the exact published head. Local pinned format and Clippy
passed on the final integrated implementation. Locked metadata/tree and patch
reproduction audits also passed. Earlier Darwin runtime checks are historical;
no new Darwin parser execution is used to obtain a passing result. Issue #96
remains OPEN/UNKNOWN. The PR records exact-head CI, both final behavioral reviews,
finished architect review and expected-head coordinator merge grant. No merge or
full recovery/parity acceptance follows from preparation alone.

Authorized graph changes:
- `crates/ktlsp-parser/Cargo.toml` direct dependency becomes
  `ktrs-parser = { version = "=0.5.1", path = "../../vendor/ktrs-parser-0.5.1" }`.
- Root Cargo.lock removes only ktrs-parser's registry source/checksum; its
  dependencies and every other package remain unchanged.
- Root workspace adds only `exclude = ["vendor/ktrs-parser-0.5.1"]`, explicitly
  approved after Cargo auto-enrollment conflicted with the standalone workspace.

No new package/external dependency, syntax/server/common API/limit change. Existing
supervised worker stays the sole product invocation. The fork and optional
worker-probe have standalone workspaces/locks; default native tests do not enroll
upstream dev dependencies or tests. Original source inventory/checksum/notices
remain preserved. recovery.patch plus the separate formatting.patch reproduces
all 54 final source hashes; no formatting opt-out is used. Locked metadata
confirms nine workspace members and the direct vendor parser path. Normal/build
and all-feature dependency inventories contain only the existing pure Rust
closure; Clippy compiler output explicitly names the vendor parser directory.
Exact-head Linux CI must independently compile/test that production graph.

Production declaration tests replace three parameter-list known-defect assertions
with exact `parse_snapshot`/`tree_for` restoration expectations. Generic recovery
and token-loss Coverage remain known blockers. Tests cover original LF/CRLF/lone
CR/Unicode/backticks, immediate/nearest parents, malformed boundaries/diagnostics,
annotation prefixes, repeated recovery/EOF, valid nested defaults and member
initializer/constructor contexts. Production recovery_limits tests exercise
annotation-probe work, postparse nesting, repeated recovery below preflight
nesting, balanced-delimiter diagnostic overflow and healthy followups. Existing
fault/stale/source mapping/budget tests remain strict.

Both reviewers must recheck production behavior; architect reviews finished work.
Pinned fmt/clippy/workspace tests run in exact-head Linux Native Rust CI during
the coordinator-approved validation turn. #83/#7/T008 remain gated on joint recovery assessment, especially
unfixed generic/default/deep type/annotation recovery; this repair alone is no
syntax/parity unlock.

## #100 classifier-generic recovery candidate

The candidate recovers simple named sibling function headers after incomplete
class/interface type-parameter lists at parameter entries or after completed
parameters/bounds. Optional complete annotations/modifiers stay with the function.
Newlines and keywords alone do not establish a recovery boundary. Generic and
extension function headers, other sibling kinds, objects, and incomplete nested
bounds/annotations remain outside this positive contract; ordinary parsing stays
in effect for them. #83/#7 and T008a require the remaining acceptance audit.

Scratch prefix parsing preserves live state and respects existing truncation;
256 copied raw tokens/4096 source bytes per attempt and 4096 copied raw tokens per class list
bound scratch parsing. Leading trivia inspection is outside the copied-token
charge and remains bounded by the input limit and advancing parser cursor.
Exceeding the scratch limits declines recovery without changing global budgets,
Coverage rejection, snapshot identity, strict result validation or the 500 ms
worker deadline. No server provider or semantic API changes. Exact independent
regressions are in declarations.rs/recovery_limits.rs. Authorized Linux CI and
both independent behavioral reviews passed for candidate054e352 on the actual
synthetic merge86bbd04: 169 active tests, zero failures, two exercised helper
fixtures and 19 worker fault cases. Exact checkout/tree evidence is in REVIEW.md.
Local format/Clippy and ordered patch/hash replay also passed. Authorized main ef120be was integrated once; final combined-head CI/reviews and
coordinator merge grant remain required. This is not a completed-delivery claim. Issue #96 remains unresolved.

The coordinator explicitly amended the accounting contract after a focused audit:
leading trivia and cheap kind/modifier/EOF prefilter misses may remain outside
the copied-token allowance. Every eligible scratch window is charged before
grammar evaluation, including failure; a zero-copy eligible rejection debits one.
The separate allocation-free leading scan is bounded by the 64 KiB input and
local comma-consuming loop progress. This is not a bound on every examination,
whole-parser complexity, RSS or total work. New leading-trivia regression evidence
is prepared but not executed. Unconditional window-predicate checks compile
against the actual byte/EOF guard. These do not establish an end-to-end active-EOF
recovery case; that gap remains for #83. Final Linux execution and renewed resource
review remain required before final integration approval.

## C14 / #121 finite ordinary-default recovery

This child changes only parameters.rs and its ordered patch/hash provenance.
Production tests in recovery_defaults.rs freeze the exact top-level LF/CRLF/CR
seed, full declaration/parameter/type/annotation projection, names/owners,
original partition/UTF-16/newlines, snapshot identity and healthy revision followup.
Four valid LF default controls plus declined nonempty-header/scratch-limit cases
preserve existing expression ownership. C14 alone in recovery_residuals.rs becomes
positive sibling acceptance; C15/C16 stay unchanged. Constructor/function-type and
existing resource tests remain in the unchanged C12/production suite.

The local ordinary recovery argument is scoped by calls, independent of recursive
probe suppression. Read-only scratch construction copies at most256 complete raw
tokens/4096 bytes, stops after four nontrivia tokens, respects active truncated EOF,
and performs exactly four grammar expect calls. Ordinary expression parsing runs
when the complete simple zero-parameter named header is not recognized. No source,
worker, API, dependency, graph, limit or diagnostic-dedup policy outside that finite
entry is changed. Ordered replay still reproduces54 final source hashes across
9 behavior/49 formatting files; archive/upstream hashes and notices are retained.

See ../../crates/ktlsp-parser/reviews/c14-default-recovery.md for pre-runtime
expectations and review/controlled-validation gates. Exact reviewed combined-head
and merged-main Linux all-three evidence must be retained in the PR/receipt before
completed delivery. No local native/parser/performance/JVM run is authorized;
#96 remains unresolved, #83/#7 remain open and full semantic parity is unfinished.

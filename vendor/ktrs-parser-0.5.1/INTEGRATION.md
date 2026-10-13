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
and performs at most four grammar expect calls. Ordinary expression parsing runs
when the complete simple zero-parameter named header is not recognized. No source,
worker, API, dependency, graph, limit or diagnostic-dedup policy outside that finite
entry is changed. Ordered replay still reproduces54 final source hashes across
9 behavior/49 formatting files; archive/upstream hashes and notices are retained.

See ../../crates/ktlsp-parser/reviews/c14-default-recovery.md for pre-runtime
expectations and review/controlled-validation gates. Exact reviewed combined-head
and merged-main Linux all-three evidence must be retained in the PR/receipt before
completed delivery. No local native/parser/performance/JVM run is authorized;
#96 remains unresolved, #83/#7 remain open and full semantic parity is unfinished.

## C13b / #136 finite extension sibling source stage

Initial source is frozen37bfe388211fe7596986e51a222af4c89a44c3c8, tree
8ba503ba15c5379d2a9c14d1ec7b97abaa8ded36, in a separate owned checkout/branch.
Root admitted only finite source and issue publication after SAME correctness,
adversarial and architect contract reviews; [issue136](https://github.com/anilbeesetti/ktlsp/issues/136)
preserves the exact historical staged body and linked admission update. No main
chase or integration has occurred. Product changes are the private optional-DOT
predicate, one obsolete String-extension negative member, the new focused test
and owned ordered artifacts/review note. No dependency/graph/worker/API/limit change.

Final contract JSON supplies all 70 exact sources and 38 full selected projections;
the new test copies selected fields literally, avoiding the obsolete intermediate
auxiliary generator. Data-only comparison verifies sources, projections, names,
diagnostics, positions and supplementary fields before any runtime. Source-only
compatibility assertions do not become complete declined-tree equality or B11
active-layer end-to-end evidence. Ordered archive -> behavior -> formatting replay
reproduces all54 final hashes with14 behavior/50 formatting/four pristine survivors.

Local validation is restricted to pinned formatting and compile-only locked
workspace all-target Clippy. No local native/parser/test/worker/server/bench/JVM/
oracle/Gradle/Maven execution is admitted. Actual implementation behavior requires
a future coordinator-assigned final integration and unchanged Linux Native Rust
all-three exact checkout/tree evidence, SAME2/architect completed-source and PR-CI
reviews, guarded expected-SHA merge and owner/root exact-main verification.
There is no push/PR/CI/integration/merge admission in this source stage. #96 remains
OPEN/UNKNOWN; no Linux-proves-Darwin or full syntax/semantic/parity claim follows.

## One assigned final integration / branch publication stage

Root assigned stable base8548ead597146a4ca52d3a4a2ec6ffd724fac7a3, tree
3f98c1ec916437445fb6e499a85caa04c5959e63, sole parent
64234587e333ebcd32d96600a7a193b06b0ac9df. It was fetched by exact SHA into the
owned independent Git directory and integrated ONCE into held source
293157402dbf1640cb27ae04901943ade03b31f2. Conflict-free merge
8bcc917588c6640a39b30abf14157bd61aa19b50 has tree
070e07b9a83fe57e3a94f2f9548937d592abc900 and ordered parents held2931574 then
assigned8548ead. No main chase or other-owner checkout mutation occurred.

All held parser/test/frozen fixture/ordered-patch/hash bytes were preserved.
Incoming39 paths comprise merged field type annotations, metadata name resolver and
V2 path-prefix budget handling plus their data/spec/tests. C16 and metadata string
table125/PR128 are inherited unchanged. The only further owned edits are this
integration ledger and the corresponding owned review/integration note. These
notes do not change source, fixtures or expectations. No Cargo/lock/workflow/worker/
API/limit/server/root-doc edit is made; Native Rust workflow blob remains
fc6202e6d32a00d430f4d4e12b502b6ca538edab. Platform-gated V2 Windows/Cygwin bodies
are not executed by local Darwin compilation or future Linux CI.

SAME correctness/functionality, adversarial and own architect must approve the
actual final combined head/tree/parents/assigned-base diff, full inherited graph,
resource and execution effects before branch-only push, one attached PR and the
existing unchanged Linux Native Rust ORIGINAL-attempt all-three run. Local checks
remain formatting and compile-only locked workspace/all-target Clippy; no local
native/parser/test/worker/server/bench/JVM/oracle/Gradle/Maven runtime is admitted.
Exact held/final owned-note diffs, source hashes and combined compile receipts are
recorded externally in the coordinator package to avoid self-referential commit
hashes. No previous source approval is treated as combined-head or CI approval.

Completed original-attempt CI must expose actual generated checkout/tree, workflow/
executor, unchanged fmt/clippy/full-workspace commands, counts and all failures.
SAME2 plus own architect perform actual completed-CI readbacks and changed-behavior
rechecks. Merge remains HELD for root's expected-head grant and predecessor137
exact-main all-three success independently verified by root and its owner.
Following a future identical-tree merge, owner/root actual exact squash/tree/sole
assigned-base/workflow/executor and exact-main all-three verification follow the
admitted policy; changed identity/tree/workflow/failure/new finding renews affected
reviews. #83/#7 OPEN, C13 aggregate GAP, T008/T008a HOLD, B3/B4 GAP, B11 UNVERIFIED,
#96 OPEN/UNKNOWN and historical audit totals remain unchanged.

## C13 qualified receiver — held source preparation after C13b

Root admitted one additional DOT/name in the private generic sibling scratch
predicate on frozen882028224a252d728dd479f2403fd2f2d97f3f67. Only
declarations/type_parameters.rs changes final vendor Rust bytes. Existing caller,
complete-bound, scratch isolation, minimum active-EOF and all resource/debit gates
remain. The recorded pkg.String seed supersedes only C13b decline-header-8's
blanket class.end>11 assertion; historical70/38/779 data remains unchanged. New
source-authored test data has59 sources,24 selected projections,35 compatibility
cases and397 UTF-16 expectations. No broader receiver/full-C13/parity claim.

Owner data-only pristine archive→recovery.patch→formatting.patch replay verifies
all54 final source files, zero offsets/fuzz. Re-derived counts remain14 behavior/
50 formatting/four pristine survivors. Upstream archive/inventory/minimal crate/
manifests/locks/notices remain unchanged. Adversarial reviewer must perform its
own independent pristine replay; neither party may borrow the other's receipt.

Pinned formatting and compile-only offline locked workspace all-target Clippy
passed. No parser/native runtime ran locally. Actual held-source SAME2+architect
reviews/rechecks and future root-assigned integration/unchanged original LinuxCI/
guarded merge/main-verification gates remain pending. Automatic approval review
rejected finite issue publication; no issue was created. Push/PR/CI/integration/
merge remain HOLD. Exact frozen fixture/ordered-patch hashes and limits are in
../../crates/ktlsp-parser/reviews/c13-qualified-extension-recovery.md.
#83/#7 OPEN; C13 GAP; T008/T008a HOLD; B3/B4 GAP; B11 UNVERIFIED;96 UNKNOWN;
historical45/32/10/3 unchanged. No semantic/server/RSS/official-performance claim.

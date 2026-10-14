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
approved7f51b2442bfca714a7aa0369f9ac9a8e64905aa3 without findings; both independent
pristine replays verify54 files/zero offsets/fuzz/counts14/50/4. Initial issue
publication was rejected twice. After fresh direct human approval, the SAME
connector published [issue148](https://github.com/anilbeesetti/ktlsp/issues/148)
with unchanged body SHA256
`9eb9210e96a434544ef2082f763375a1f187cdb8e617df30483b468947300d74`; a separate
repoID1415474657 fetch verifies OPEN/exact body. Old rejections/history remain.
Only four owned Markdown publication notes change in this successor; SAME3 docs
rechecks are required. Future root-assigned integration/original LinuxCI/guarded
merge/main-verification gates remain pending. Push/PR/CI/integration/merge stay
HOLD. Exact frozen fixture/ordered-patch hashes and limits are in
../../crates/ktlsp-parser/reviews/c13-qualified-extension-recovery.md.
#83/#7 OPEN; C13 GAP; T008/T008a HOLD; B3/B4 GAP; B11 UNVERIFIED;96 UNKNOWN;
historical45/32/10/3 unchanged. No semantic/server/RSS/official-performance claim.

## C13 single nullable receiver — held source preparation

Root admitted the reviewed finite contractv3 on frozen source
6365dcdf58ecd58b0a4715eba10d60307edddb15. Only private
`declarations/type_parameters.rs` changes vendor Rust behavior: after FUN and the
first identifier, accept joined SAFE_ACCESS or QUEST with required DOT/name;
otherwise preserve the existing plain/qualified branch and final LPAR check.
No joining-state mutation/type-parser callback/new boundary is introduced.
Qualified-nullable/generic/parenthesized/annotated/larger receiver families remain
excluded. Complete zero-parameter fixtures do not imply the recognizer requires
RPAR/body; the incomplete-LPAR case explicitly preserves header-only recovery.

New focused production assertions freeze32 selected positive projections,
53 finite compatibility/header/resource controls and491 original UTF16 literals,
including NULLABLE_TYPE and separate original QUEST/DOT parents. Historical70
C13b source payload remains byte-identical; only decline-header-10 dispatch is
superseded. Prior decline-header-8 dispatch and the entire qualified59 test remain
unchanged. Every new case checks original token reconstruction, snapshot id/kind/
revision/source identity, stale/mismatched id/source/kind rejection, all retained
elements/diagnostics after exact healthy followup on the same Parser. Each call
starts a separate supervised worker, not worker reuse.

Owner independent pristine archive→recovery.patch→formatting.patch data replay
verifies all54 final Rust files with zero fuzz/offsets; rederived14 behavior/
50 formatting/four pristine survivors. Original archive/upstream inventory and
notices are unchanged. Adversarial reviewer must independently perform its own
fresh replay; historical/owner receipts cannot substitute. Exact ordered hashes,
source scope, fixtures and current verification are in
../../crates/ktlsp-parser/reviews/c13-nullable-extension-recovery.md.

Pinned formatting and compile-only offline locked workspace all-target Clippy
passed; no local parser/native/test/JVM runtime ran. Compile success is not
behavior acceptance. At this source preparation checkpoint, issue publication is
pending actual SAME2+architect held-source approval. Final integration, push/PR/CI/
merge remain HOLD for one root-assigned base after ZIP158's actual squash; merge
also waits owner+root exact-main clear. No new executor/retry-to-green/deadline
relaxation. C13/B3/B4 GAP, B11 UNVERIFIED, T008/T008a HOLD, #7/#83/#96 OPEN and
Darwin96 UNKNOWN remain. No semantic/full-parity/RSS/official-performance claim.

## C13 / #161 plain longer-qualified receiver chains — source preparation

[Issue161](https://github.com/anilbeesetti/ktlsp/issues/161) implements the finite
contract at PR160 fe97781cfde7c9e230470463e3e1212925f0c1b8/tree13672a926,
manifest fcaf46f05e26a0a0d3256dab1f164ca3a280d83b0818c23be7bf200834326f69.
Only the private plain DOT arm in declarations/type_parameters.rs changes vendor
Rust behavior: each loop entry consumes DOT, requires IDENTIFIER, and either
advances or immediately rejects. Existing <=256 copied raw tokens imply <=127
entries; no counter/new budget/backtracking/type callback is needed. Unqualified
nullable arms, final LPAR check, boundary eligibility, completed-bound proof,
scratch/live isolation and copied256/4096-byte/4096-per-list gates are unchanged.
Receiver counts exclude the final function name. Qualified-nullable, generic,
annotated and parenthesized receivers stay outside this recovery recognition.

The focused test preserves all116 source-authored cases:64 selected positive
projections,50 finite controls,2 exact NestingLimit expectations and1666 original
UTF16 literals. Complete zero-parameter fixtures do not make RPAR/body a recognizer
requirement. Chain123's deepest top-level identifier reaches depth128; recognized
chains124/126 require the unchanged NestingLimit outcome, not accepted CSTs.
Other owners/trivia/bytes/resource guards can reduce acceptance. The corrected
leading-comment FUN span starts13; the keyword remains13213. Immutable snapshots,
original-byte lossless partitions, stale/mismatched identity, anonymous/lazy
ownership and same Parser healthy followups remain explicit assertions. Resource
failures retain a separately accepted prior snapshot across the failing request.

All historical70 C13b,59 qualified and85 nullable literal payloads remain exactly
unchanged. Only qualified-decline-third-component's contrary negative dispatch
routes to the exact new positive seed; the earlier two C13b supersessions remain.
No shared helper/API,worker,server,Cargo,lock,workflow or root-status change.

Fresh owner data-only pristine archive -> recovery.patch -> formatting.patch replay
verifies all54 source hashes with zero offsets/fuzz and retained notices. Re-derived
counts are14 behavior/50 formatting/four pristine survivors. The independent
adversarial reviewer must freshly replay the complete stack separately; owner and
historical receipts cannot substitute. Only type_parameters.rs final vendor Rust
bytes differ from the frozen base. Full source/provenance/limit details are in
../../crates/ktlsp-parser/reviews/c13-long-qualified-extension-recovery.md.

At this source-preparation checkpoint, pinned formatting and compile-only offline
locked workspace all-target Clippy passed. No local parser/native/test runtime ran.
Actual same correctness/adversarial held-source reviews and architect completion,
then original unchanged Linux Native Rust CI, remain delivery gates. Coordinator
admitted source and issue publication; push/PR require held-source clearance and
merge requires the root expected-head grant. #7/#83/#96 OPEN/Darwin UNKNOWN;
broaderC13/B3/B4 GAP,B11 UNVERIFIED,T008/T008a HOLD. No semantic/full-parity,
whole-parser complexity,RSS,official-performance or Darwin-fix claim.

### Held-source review correction before native execution

Initial held head e65344987bc2d2886ee73fefedcca847405b9ad9 is retained.
SAME independent source reviews found that raw-chain127's frozen downstream-guard
control cannot require an accepted CST: after scratch decline, MissingGT consumes
FUN, leaving259 simple top-level error tokens plus that one diagnostic. The worker
checks diagnostic count260>256 before tree-depth serialization and returns exact
OutputLimit. Its unchanged literal control now asserts ParserError::OutputLimit,
prior-snapshot immutability and healthy followup. The two frozen NestingLimit cases
remain exact. No fixture, product limit or predicate change was made.

Object no-recovery now additionally requires object.end>12 and no direct KT_FILE
FUN at13. parse_object discards its inner SyntaxKind, so merely requiring root
OBJECT_DECLARATION would miss forbidden early recovery. Resource prior snapshot,
source,elements and diagnostics are captured BEFORE the failing request and checked
immediately afterward, then again through healthy revision2. Initial findings and
architect's tentative concern/incorrect withdrawal/correction remain in external
receipts; corrected-head actual SAME2+architect rechecks and original CI remain
required. No local native/parser/worker/test runtime executed.

### Original PR162 failure and narrow harness correction

Original run38417127591/job115364425245 attempt1 on reviewed3b7e9cf/tree87e8a95
passed fmt/Clippy and failed the focused test at broken-modifier-annotation's added
no-top-level-FUN assertion. Raw log SHA256
`f161a37a5b8556aa8f030fae241d15f00c8385a763e13feef7bcb39af218c3f4`,
56840 bytes/721 lines, independently matches root. The frozen control requires
class.end>11/nonempty diagnostics, explicitly no whole declined-CST expectation.
Its failure does not establish a new parser defect. SAME2+architect independently
trace scratch annotation rejection with live state untouched; missingGT consumes
AT[12,13), CLASS ends13, top-level recovery consumes A/(, then normal live grammar
parses FUN[16,46) under KT_FILE. This is token-wise recovery, not annotation
recovery promoting a function. Immutable contract prose saying broken annotation
stays owned is narrowly superseded by this adjacent explicit source-stage erratum:
the consumed AT remains CLASS-owned; the later named function can be top-level.
Incomplete-bound annotation-owned and anonymous/lazy controls are distinct and
retain their stronger ownership assertions.

Only stable ID broken-modifier-annotation receives this source-derived correction:
retain class/diagnostic/token/snapshot checks, assert AT[12,13) nearest CLASS[0,13)
and normal KT_FILE FUN[16,46); every other no-promotion assertion stays intact.
All116 literal fields and70/59/85 historical payloads, grammar, patch stack and
budgets remain unchanged. Initial failure/reviews are preserved; no unchanged retry
or local runtime runs. The failed test did not reach its nine later controls and
two NestingLimit fixtures, so those outcomes remain unverified at this checkpoint.
Root suspended merge and released fe97781 integration reservation to independent
work; next stable base requires root assignment, integrated-delta reviews and
actual combined-tree original Linux CI before a renewed merge lease.

## C13 / #169 ordinary property sibling headers — source preparation

Root assigned exact 941c72efd9896b0847ab342ec0dd22c10a000f3b as source and initial
integration base. Its six docs-only changes preserve all 17 frozen contract inputs
from 50bef1c6d907c673839dd06666023314cad5c1fa, verified before source edits.

Only the classifier scratch predicate gains first-token val/IDENTIFIER/EQ recovery.
No shared API, worker, server, dependency, Cargo, workflow or resource limit changes.
The focused test replaces only the contradictory val exclusion; all historical
function/nullable/qualified families remain unchanged. Exact source/patch hashes,
contract and validation gates are in
../../crates/ktlsp-parser/reviews/c13-property-recovery.md. Publication/PR/original
Linux CI are admitted; merge remains HOLD for root's expected-head/base lease and
passing actual source/CI reviews, followed by owner/root exact-main verification.

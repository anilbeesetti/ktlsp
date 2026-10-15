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

## #83 B3 finite causal observation

An additive doc-hidden vendor wrapper installs a scoped thread-local observer;
actual Parser::eof dispatches it before its ordinary builder predicate. Restoration
on normal/nested/unwind paths and thread isolation are checked in the production
workspace's deadline_observation test, since this vendor workspace is excluded
from unchanged root CI. Parent/worker witness transport is a request-local Unix
datagram pair, never parser stdout or a new relative deadline. Immediate cleanup
is unchanged; no acknowledgement grace, retry or polling wait is introduced.

See ../../crates/ktlsp-parser/reviews/deadline-observation.md for literal request,
causal acceptance, fallback FILE/PIPE flag checks, fixed wire/count/cadence and
unmeasured default/observing overhead. The ordered patch replay now covers16
behavior/50 formatting files and all54 final source hashes with four pristine
survivors. Original notices/archive/manifests/locks stay unchanged. Source-stage
fmt and compile-only Clippy do not prove a runtime witness; reviewed original
Linux CI is still required. B3/B4 GAP, B11 UNVERIFIED, T008 HOLD; no semantic or
performance/full-parity claim.

## C13 two-component nullable recovery — combined source candidate

Root released exact fe4f4a73f0bf9190f7668bf74a9b963f70114449,
tree01e0300947dc10cb4f737fded962987d556358fa, soleparent
cb0c8c7dd064a5a3f2d1c82097a2cfd0547e496a after PR202 owner/root clear.
That delivered source was integrated once into reviewed private C13 source
e7cb4d28529a41e0bf12c4d6d90372daaa257e6b without conflicts; all six C13 blobs
and all incoming B3/parser/binary bytes were preserved before provenance updates.
The former five-file publication HOLD is released for this sole owner; earlier
private-stage mismatch notes are historical, not the current artifact state.

Only declarations/type_parameters.rs adds C13 grammar behavior: after exactly two
successfully consumed identifiers, accept one SAFE_ACCESS or QUEST+DOT separator,
a direct function identifier and complete LPAR. Existing single-nullable/plain
qualified/property branches, caller/bound/recursion/EOF/window/debit guards remain.
Long nullable chains and generic/annotated/parenthesized receivers remain gaps.
Only nullable-decline-qualified and qualified-decline-nullable dispatch change;
all70/59/116/85 historical payloads/supplements and other live overrides remain.
The focused literal packet retains49 positives/61 controls/42 cuts/1416 UTF16
positions. Forty EOF arithmetic rows do not establish active-layer integration.

Inherited B3 observer state is thread-local, not Parser-instance-local. Thus
Parser::eof in same-thread scratch probes also dispatches the callback before
the unchanged builder EOF predicate and contributes to sampled cadence. A receipt
cannot identify live versus scratch/lazy work. C13's syntax-state isolation does
not promise unchanged observation counts. Ordinary parse_snapshot installs no
observer socket/callback, but each EOF still checks TLS; default/observing overhead
and timing/allocation perturbation are unmeasured. Callback dispatch does not
change EOF truth, recovery budgets or add waits/retries. No B3 source/fixture/
protocol/allocator/limit is changed by this child. All three delivered PR202
observations had MissingACK; B3 remains GAP.

Inherited allocation/refusal/status86 and supervisor precedence remain: main-loop
cancellation, original deadline, output error, then child status; allocation
refusal priority at ordinary worker exit; finish_io keeps the original deadline;
guard.finish()? retains ambiguous Io override before scoped joins. Cleanup waits/
joins remain unbounded. No identical timing/containment result is promised by
TLS overhead; ordinary syntax/result compatibility requires actual original CI.

Fresh generation from the verified registry archive, delivered behavioral phase
plus only reviewed C13 delta, then pinned1.99.0/edition2024 scratch formatting
rederived16 behavior files,50 formatting files,54 final hashes/four pristine
survivors. A second fresh owner extraction replayed recovery then formatting with
zero offsets/fuzz and matched all54 current combined Rust files. Independent
adversarial extraction/replay is separately required; no old replay receipt or
counts were reused. Original upstream inventory, archive, notices and manifests/
locks remain unchanged.

Combined phase SHA256:
- recovery.patch 74922955e653db43633e3e49848060f9d667e699ddae370d69d886f632b7ed87
- formatting.patch c5f1625125e46e35ff79ff1094698eb33995464d6b863e184ba7d2e65b1637aa
- formatted-sha256.json 7666e63232ddc2aac66a77311c213417e6c700a8839d3c95bffe00d2160c5773

SAME3 actual combined source/provenance reviews, owned-file formatting and
compile-only locked workspace Clippy precede issue/push/open+attach PR and original
unchanged Linux all3. No local native/parser/worker/tests/server/bench runtime.
Merge remains HOLD for root's exclusive expected-head/base lease and completed
source/CI reviews. See ../../crates/ktlsp-parser/reviews/c13-two-component-nullable-recovery.md.
Aggregate C13/B3/B4 GAP, B11 UNVERIFIED, T008/T008a HOLD, #7/#83/#96 OPEN;
Darwin UNKNOWN. No semantic/provider/full-parity/performance promotion.

## B11 finite actual-copy observation — source candidate

Source base a13ab6ba4100f7c26377a47ab5af0858b6491a48, tree
1fa56ee9bc6c4c6a2e07441aa47fd95e257a99e0, sole parent
2d561ab30812dcdabc56260a8b510979e1e70a61; integrated once, clean, before
implementation. SAME architect/correctness/adversarial reviewers confirmed the
incoming C13 two-component nullable header additions do not change the frozen
bare healthy( path or copy/layer/worker seams. C13 grammar additions are retained.
Root explicitly approved only a finite actual-copy production integration
interpretation and the exact twelve-file additive observation scope. Stronger
clipped-header negative remains unrealized; its prior feasibility HOLD is valid.

Only vendor lib.rs, builder/layers.rs and declarations/type_parameters.rs change.
A separately scoped callback reports the constructed scratch builder's last
copied token endpoint, not a bound-clamped prediction. Independent live-layer
count/minimum do not use the production minimum helper; the production bound is
reported separately. The default path adds an inert callback-slot check at copied
windows and skips the independent walk. Existing copy predicate/loop/debit,
recursion suppression, scratch grammar, C13 branches and B3 EOF bridge remain.
Nested callback installation is declined with observation disabled for that scope;
RAII restores previous callback state on normal return and unwind.

Exclusive Unix facade/worker observation uses a fresh same-child/request datagram
pair, one64-byte aggregate and one nonblocking send attempt; parent makes at most
two65-byte nonblocking reads after existing cleanup/scoped joins. Worker source
clone is bounded by64KiB; fixed state holds first record and event count saturated
at2. Full builder source equality and independently positive layer count qualify
events; no desired offset/bound/endpoint filter or synthetic layer is installed.
Duplicate, failed installation, reentry or malformed records cannot become valid
zero-event controls. Socket address checks precede shared-OFD nonblocking changes;
trusted parent pair construction establishes socket type, not fd conversion.
Source text does not enter the observation wire.

Default Reply schema, allocator/latch/status86, B3 argument/cadence, source/window/
allocation/output limits, cancellation/deadline/error priority and checked cleanup
remain. Observation work can change actual timing/allocation outcomes: overhead
and outcome equivalence are UNMEASURED. No direct observation-error remapping,
wait/retry/grace/relative-deadline/config/dependency/framework is added. Existing
cleanup waits/joins have no hard bound. No observer runs in allocator callbacks.

Fresh verified archive -> delivered a13 behavioral phase plus these three
additions -> pinned1.99.0 edition2024 scratch formatting produced actual counts:
16 behavioral files, 50 formatting files,
54 final Rust hashes, 4 pristine survivors.
A separate fresh owner extraction replayed both ordered patches with zero fuzz/
offsets and matched every current source hash. Notices/licenses, original archive,
upstream inventory, manifests and locks remain byte-identical. Independent
adversarial replay is separately required; no previous delivery replay is credited.

- recovery.patch SHA256 6a9876d6b4ae1fb4fcab6528ac50c88c32a9c8f8db474cb35a9d33078c9fb9cf
- formatting.patch SHA256 310e0541870c7d4e1ab5c6ded355195a081587bf3178090c592cbd5d00fd9338
- formatted-sha256.json SHA256 eddd511b00401c38ba9c62463eaef2456d269167c96e81f9d06dbba4f996de66

The frozen production test requires strict recovery/identity/original range/parent/
diagnostic/UTF16/partition checks, an actual source-matching active event with
count1/min57/start28/end57/next57..58 eligible under other caps, and ordinary strict
healthy followup. Expected values were frozen before runtime; no first-output
retargeting or rerun-to-green. Machinery/unit controls are distinct from that
production evidence. Format and compile-only locked all-target Clippy are local;
no local parser/worker/tests/server/bench/JVM/oracle execution. Original unchanged
Linux all-three CI and SAME3 raw/config/checkout reviews remain required.

B11 UNVERIFIED; B3/B4/broader C13 GAP; T008/T008a HOLD; #7/#83/#96 OPEN;
Darwin UNKNOWN. Even a later passing finite observation does not prove header-cut
rejection, multiple active layers, annotation semantics, full B11/#83/T008, grammar,
semantics or performance. Merge remains held for root's explicit expected-head/base
lease and a successful read-only fail-closed guard in a separate tool call before
any ordinary expected-head merge. See the focused source review at
../../crates/ktlsp-parser/reviews/generic-copy-observation.md.

## C13 finite one-argument generic receiver — source candidate

Integrated exact delivered B11 base f6e2c6b3a8e43a4a76ba27a01205f4e44b7e8e04,
tree a920a637bf947ae8fd6d386d32b27044ddb6fbcd, sole parent cca3a618, ONCE before
editing. SAME correctness/adversarial/architect reviewed incoming B11 source and
confirmed frozen V3 compatibility before implementation. B11 observer, library,
layers, worker and five tests remain unchanged from this base.

Only type_parameters.rs changes source behavior: existing scratch prefix/FUN then
one unqualified receiver/one unqualified argument/direct name with complete LPAR.
The literal generic branch requires IDENTIFIER LT IDENTIFIER GT DOT IDENTIFIER;
bare suspend receiver/argument are excluded because live type grammar treats them
as modifiers. Escaped names retain grammar; bare dynamic argument is DYNAMIC_TYPE.
Failure in this branch cannot fall through to qualified/nullable recognition.
No new builder/API/helper/callback/dependency/ambient state/budget is introduced.
Existing complete-bound gate, recursion suppression, scratch/live isolation,
256complete-token/4096byte/minimum active EOF window and4096copied-window debit
including failed windows remain. Incoming observer logic is byte-preserved.

V3 contract manifest85cdaa57e1cc16dfca40479e7b54de9aed2223dd3097a0a8087bfdb6d1997f90
was frozen and actually SAME3 approved before source. New production-worker test
retains all84sources/26projections/58controls/19200positions, exact diagnostics/
parents/tokens/identity/followup. Only historical decline-header-9 dispatch is
superseded; its42byte/no-final-LF fixture and all other historical bytes/assertions
stay intact. The separately named43byte/final-LF case stays distinct. All36related
inventory occurrences are preserved. Earlier V1 EOF expectation and V2 inventory
regex preparation errors were preserved and corrected BEFORE any runtime.

Fresh owner authoritative crates.io archive checksum
ec8d43d9549490689477385c7678bbd22ecbc445973ab364e8ecb69dd4882bcc -> delivered
behavioral phase plus only this private-predicate delta -> pinned1.99.0/edition2024
scratch formatting rederived16behavior/50formatting files,54final hashes and4
pristine survivors. A second fresh owner extraction replayed both new patches
with zero fuzz/offsets and all54hashes matching. Independent ADV must fetch/extract
and replay separately; old receipts/owner extraction cannot substitute. Original
archive/upstream inventory/notices/minimal manifests/locks stay unchanged.

Combined ordered phase SHA256:
- recovery.patch 3027d625f09b57d132fc8379471dc1824d9710c9f156d9a6ff343fa2db06a2a8
- formatting.patch 7aafd8939e3dfbb695871b3869dd4eb70a92772ccc79cf2057b3ef72d8b35069
- formatted-sha256.json c313db83e58e2dc7abac7c5d59d08b613ba4e16e1bef55ca4bf91f242d51fc4c

Local pinned format/compile-only locked Clippy and SAME3 final source/provenance
reviews precede admitted publication and ORIGINAL unchanged Linux Native Rust
all-three CI. No local native runtime is authorized. Merge still requires the
root exclusive expected-head/base lease, a separate read-only fresh guard with
all matching facts and exit0, then a separate ordinary expected-head merge.
Actual squash tree/sole parent and OWNER+ROOT original exact-main proof follow.
See ../../crates/ktlsp-parser/reviews/c13-generic-receiver-recovery.md.

Broader C13/B3/B4 GAP, full B11 UNVERIFIED, T008/T008a HOLD; #7/#83/#96 OPEN;
Darwin UNKNOWN; observer overhead UNMEASURED. No grammar/semantic/parity/performance
promotion follows from this finite source candidate.


## C13 finite nullable generic receiver — initial source candidate

Assigned initial base1c41e58daf531d96144686638a84bcac157ea0e3/tree9a1465b4bb0efcb43d8d7c70979f02d134727c39,
soleparent626d374ee64a8abc3b50cf7ad050a7e72209e10e, integrated ONCE before edits.
Retained SAME3 incoming reconciliation confirms83binary-property paths disjoint
from unchanged parser/config/vendor pins and frozenV4contract manifest170cfc334b3a961029c021e69cf85ca2ea923361a4ef488edb453783f6c0b570.
Only private type_parameters.rs post-GT header recognition adds one nullable
suffix SAFE_ACCESS or QUEST DOT before directname/completeLPAR. One unqualified
nonnullable argument, bare suspend gates, generic-arm nofallthrough, trivia,
256whole raw tokens/4096normalizedbytes/minactiveEOF/4096copied-token aggregate
and global limits remain. B11 observer/lib/layers/worker and all other production
Rust source remain unchanged. No helper/API/dependency/server capability change.

Focused111case/38exact/113881originalUTF16 expectations remain literal-frozen.
Only three historical dispatch amendments: generic-suite safe-nullable-receiver
and spaced-nullable-receiver, nullable-suite nullable-decline-generic. Original
84/85payloads,82/84otherexpectations,ID13erratum/helpers/priorliveoverlays stay.
Source-derived rootpartition schema supplement on only the two generic records
keeps their unchanged adapter and all frozen fields. Ordered duplicate EOF error
ownership is checked explicitly in the new test. PreparationV1 third-assertion
finding and V2 scope-wording correction remain preserved; root granted tenthfile.

Fresh authoritative archive -> delivered behavioral patch plus only this suffix
delta -> pinned1.99.0 edition2024 formatting -> ONE separate fresh owner ordered
replay with zero fuzz/offsets matches all54finalRust source hashes.73trackedvendor
paths are distinct from54source hashes; all67other vendor paths/notices/licenses/
upstream inventory/manifests/locks retain original bytes. Independent ADV freshly
fetches/extracts/replays separately once; no inherited replay receipt is credited.
Actual phase counts: 16 behavior / 50 formatting /54final / 4 pristine.
New phase SHA256:
- recovery.patch 669994202dc68fee8172e86997922f0ccc96c682577f2e32a8da70622c97f44a
- formatting.patch e9ce26188d086e67207eec9f5bc3f84661f0c2bb61fa2a670b33109c06d50535
- formatted-sha256.json 3dc64e81b48bad57933b0915e3920fa746ebe4c61020e3c1405a0400994627cc

Local fmt and lockedworkspacealltargetsClippy are COMPILEONLY; no local native/
parser/worker/server/tests/bench/JVM/oracle/upstream runtime. Actual SAME3 source
and completed ORIGINAL unchanged Linux all3/raw/config/checkout/count reviews
remain required. First CI contradiction preserve+STOP; no output-derived fixture
tuning or unchanged-head rerun. Merge remains HELD for root futurestablecut/sole
expectedhead-baselease and separate EXIT0valid fresh read-only guard before any
ordinary expectedhead merge. See ../../crates/ktlsp-parser/reviews/c13-nullable-generic-receiver-recovery.md.
BroaderC13/B3/B4GAP/fullB11UNVERIFIED/T008/T008aHOLD/#7/#83/#96OPEN/DarwinUNKNOWN;
observeroverheadUNMEASURED; no semantics/fullparity/performance claim.


## Initial publication preparation — #230

Child issue https://github.com/anilbeesetti/ktlsp/issues/230 records this finite
nullable generic receiver subset. Retained SAME correctness/functionality,
adversarial and architect actually reviewed the source and rechecked the
aggregate TYPE_PARAMETER_LIST ownership/range assertion; all approved with no
remaining findings. Frozen111case payload and all historical inputs/other
expectations remain exact. Source-stage format and compile-only locked Clippy
first attempts and changed-test rechecks passed exit0; no local native runtime.

Owner and independent ADV each performed one successful fresh-authoritative
ordered final patch replay, zero offsets/fuzz,16behavior/50formatting/54final/
4pristine. Independent receipt SHA256
bc00c65fb98366f86a59e85e2b3dd3ec29510b69769c4e1e7b77e474071ae5fe.
ADV first sandbox DNS exit6 was preserved before zero extraction/replay phases;
subsequent authorized exact-fetch network escalation succeeded, with no reuse of
owner archive/extraction. All73trackedvendor/54source inventory distinctions,
67other vendor paths/notices/manifests/locks and intentional patch whitespace
remain verified. No redundant replay is required for this documentation update.

Original unchanged hosted Linux Native Rust CI is still PENDING: exact commands
cargo fmt --all -- --check; cargo clippy --locked --workspace --all-targets -- -D warnings;
cargo test --locked --workspace. Actual raw/config/checkout/counts and111ordered
first outcomes must be reviewed by retained SAME3. First contradiction preserves
and STOPS, without output-derived tuning or unchanged retry. No merge lease or
final integration cut has been assigned; no main chasing or native local runtime.
BroaderC13/B3/B4GAP/fullB11UNVERIFIED/T008/T008aHOLD/#7/#83/#96OPEN/DarwinUNKNOWN.


## C13 finite two-argument nonnullable receiver — source candidate

Exact root-admitted base28ff42d743b6290b9a1e587e36eced80cebd6438,
treef989c624b58fead0144ebe197967e6ff2291444d,
soleparent9b4cf49e817d0bb30cbd2f09b674f91afb1aa5fd. PR231 OWNER+ROOT delivery
released vendor ownership before this child. All ten delivered PR231 paths match
reviewed provisional4c1bad2 by byte/hash and Git mode/type/blob. No other incoming
delta is authored here. Frozen V2 contract manifest
d8ebae1ed7c3bffcf8819bcee6ba179a4ac3e5a0ebf7e596b31d771915e908c3;
actual preparation SAME3 approved corrected V2, preserving V1 findings.

Only type_parameters.rs adds a COMMA arm requiring second non-bare-suspend
IDENTIFIER, GT, DOT, direct name and complete LPAR. The one-argument nullable
suffix remains unchanged. Nullable TWO arguments, trailing commas, extra/missing
arguments, nullable/nested/qualified/annotated/variance/star/function/intersection
arguments and hard in remain declined. Soft out and escaped names stay supported.
No live grammar/helper/export/resource/worker/server/dependency change occurs.

Focused131case/42exact/142131original position objects match frozen V2 exactly.
Complete ordered error ownership retains duplicate zero-width EOF entries;
nonempty filtering is used only for source partitions. Four stale identities
include same-length2→3 for the65536-byte case, with separate65537InputLimit;
strict healthy followups preserve old snapshot/tree/source. Only historical
generic-receiver multiple-arguments gains one explicit dispatch. Its original
46-byte input, all84embedded inputs/83other records/prior nullable overlays and
malformed-prefix erratum remain exact, as does the full111nullable231 test.
The canonical historical overlay adds only its necessary original_partitions
schema for the unchanged adapter; frozen131 objects stay untouched.

Owner and independent ADV each completed ONE fresh-authoritative ordered final
archive→recovery.patch→formatting.patch replay, zero offsets/fuzz. Counts remain
16behavior/50formatting/54final/4pristine, with53unchanged Rust files and only
type_parameters.rs changed. All67other trackedvendor paths/notices/manifests/
locks retain exact bytes. ADV receipt SHA256
56c70f5f39eb1d0d5042bfe0e5929355b31674f83a9d1102545701a075c0ea74;
its initial restricted DNS fetch failure is preserved before the authorized
fresh fetch and replay. No redundant replay follows docs/harness-only changes.
Final phase SHA256:
- recovery.patch 1db8080819e4b5357e6bed3d75156b53050f3ee88d3f5fdf10aac75257b761c5
- formatting.patch 3d010eb42f3c1006e2e2ab05b36899345da7ca071df6f38ec6c438da9a8276f3
- formatted-sha256.json 085ecbe6f94b02dba681bca32fed7ef8aa13abce3977906aa186b36f9a14196b

First pinned local format check and lockedworkspacealltargetsClippy passed;
Clippy is COMPILEONLY. No local native/parser/worker/test/server/bench/JVM/oracle
runtime is executed. Actual final-source and first ORIGINAL unchanged Linux
Native Rust CI reviews by retained SAME3 are separate gates. Required hosted
commands remain cargo fmt --all -- --check; cargo clippy --locked --workspace
--all-targets -- -D warnings; cargo test --locked --workspace. Preserve first
failure and STOP for independent source diagnosis/root correction grant; no
output-derived expectation tuning or unchanged retry to green.

Only nine granted source paths are owned. Initial publication needs passing
source review; merge needs a later root stable integration cut/exclusive exact
head-base lease, separate strict read-only EXIT0 valid guard and separate ordinary
expected-head merge, then OWNER+ROOT original exact-main delivery proof. No main
chasing, protection bypass/settings changes or force-main. See
../../crates/ktlsp-parser/reviews/c13-two-argument-generic-receiver-recovery.md.
BroaderC13/B3/B4 GAP, fullB11 UNVERIFIED, T008/T008a HOLD, #7/#83 OPEN and
#96/Darwin UNKNOWN remain. No semantic/provider/full-parity/performance claim;
native product/build/core/default tests stay Rust-only at the existing boundary.


## C13 nullable two-argument receiver — source candidate

This finite child follows delivered PR241 at exact
`fe280c5fab13cde5a9bdf50893aee859fc626a2a`, tree
`b32813f2f200741eb17365a6d7ef00155ce107de`, sole parent
`77bd0ac8b4e74dadd037a3404b82dba7e004c395`. OWNER+ROOT original exact-main
proof released its nine vendor/recovery paths; root then granted these ten paths.
Relevant parser/lexer/Cargo/toolchain bytes match reviewed provisional9a1a33b;
unrelated delivered integration changes stay intact. The sealed preparation
manifest is `b9c4c4e2f8b532128d77052e18ac800607c7e2e3ef6e7dbc29de6bed81177da6`.
Its corrected candidate was approved by retained architect/correctness/adversarial;
the first audit redaction finding remains preserved and independently rechecked.

Only the existing comma-arm suffix changes: after second non-bare-suspend
IDENTIFIER and GT, accept DOT, SAFE_ACCESS, or QUEST then DOT, followed by direct
IDENTIFIER and complete LPAR. Exactly two nonnullable unqualified identifier
arguments on one unqualified receiver; only one nullable receiver suffix.
One-argument/plain-two behavior, modifier/annotation gates, completed bounds,
no generic fallthrough, bare suspend restrictions and all budgets stay intact.
Nullable arguments, trailing commas, extra arguments, nested/qualified/annotated/
variance/star/parenthesized/function/intersection types remain outside this child.
Soft out/dynamic/escaped names retain existing syntax rules, without semantics.

The new focused test embeds all156 frozen objects/51exact projections/206112
original UTF16 entries unchanged. It checks exact snapshot/source association,
lossless token partitions, selected parents/owners/names/diagnostics/tokens,
complete ordered ERROR_ELEMENT ownership including duplicate zero-width EOF,
all four stale identities and healthy followups retaining old trees. Raw QUEST
and DOT are separate; scratch can join adjacent suffix as SAFE_ACCESS; live
QUEST belongs to NULLABLE_TYPE and DOT to FUN. Forty-two raw256/byte4096 cuts,
four +1 LPAR controls, whole-token caps,4096copied-token debit/reset, static EOF
controls and original64KiB admission stay frozen. Full B11 remains UNVERIFIED.

Only nine historical expectations gain additive stable-ID/source-equality
selection: nullable111 multiple-arguments and twoarg131 cases48..55. Their raw
payloads/input bytes, all other110/123records, prior nullable85/generic84 overlays,
PAIR_AMENDMENT and malformed-prefix erratum stay unchanged. Helpers/selections
outside these explicit branches stay intact; no compiler output supplies expected
values. Counts111/131 describe unchanged original payloads, not amended projections.

The changed current candidate requires new ordered patch evidence. Owner performed
one fresh-authoritative original archive replay, recovery then formatting,
zero offsets/fuzz,16behavior/50formatting/54final/4pristine. Only type_parameters.rs
changes among54Rust files;53source and67other trackedvendor paths/notices/manifests/
locks retain delivered bytes. Accepted prior behavioral phase data were verified
against exact old patch bytes for construction; no redundant historical replay.
Current phase hashes:
- recovery.patch SHA256 `e1018f60be1c99c1e27c026a70b2e67bf326f973b39d42416d18209a6b9eddf3`
- formatting.patch SHA256 `f14d022585a3d51ca286ca570f3ec9f72f791153d71551dce3dceac21efad4a7`
- formatted-sha256.json SHA256 `944990615fde23fb4df2ed4688368de5d2ea4c4d9ee6dcb35942803963bdba7a`

First pinned local format check and locked workspace/all-targets Clippy passed;
Clippy is compile-only. No local native/parser/worker/test/server/bench/JVM/oracle
runtime ran under issue96. Current independent ADV replay, complete ten-path
source reviews and source readiness receipts are separate evidence; this note
alone does not assert those approvals. No hosted CI ran in this source stage.

Publication/commit/push/issue/PR/CI awaits separate root source-readiness grant.
Later unchanged pinned format, locked Clippy and locked full-workspace tests
must pass in original hosted Rust CI, preserving first outcomes and failures.
Do not tune frozen expectations from output or rerun unchanged failures to green.
Root must assign one final integration cut and exclusive exact-head/base lease;
a fresh separate strictly read-only guard must return exit0 and complete parsed
matching main/head/base/check/state/scope/config/clean reviewed facts before a
separate ordinary expected-head protected merge. Preserve lease through squash
tree/sole-parent verification and OWNER+ROOT original exact-main proof/release.
No bypass/admin/settings/main force-push or main chasing.

T008/T008a HOLD, broaderC13/B3/B4 GAP, fullB11 UNVERIFIED, #7/#83 OPEN,
#96 local parser gate and Darwin UNKNOWN remain. No semantic/provider/full-parity/
performance claim. Native product/build/core/default tests remain Rust-only;
no bundled JVM/JDK/compiler/JNI/C/C++ analyzer/tree-sitter C. Existing optional
external build tools and isolated oracle stay at their separate boundary, unused here.


## C13 three-or-more simple generic receiver — source-only candidate

Assigned delivered base `66d2a99c752f901dbe7a46a7fc5254cdd8a90484`, tree
`c3e117c44097f533ef4a5440411be1e5b25a27a4`, sole parent
`9bf15b653219f105b99a239121098cef2fd932df`. The explicit nine-path source grant
has SHA256 `5ce8d8b6df203d600fe9894e02843a1585126b4f8824c84c1cf702aa8b3e48da`.
Before edits, all1002 incoming mode/type/blob/SHA identities were verified against
assigned66d2, and all1002 disk hashes matched that assigned base after one exact
fast-forward. Separately, actual253 ten-path inputs and all141 relevant parser/
text/vendor/Cargo/toolchain/workflow inputs matched accepted provisional68a. Frozen preparation remains a distinct fe280/provisional
cut: CONTRACT_READY SHA256
`6904e1e788d1b5535d2206431b84824fef786ce1dcb2709864002b720ff16299`, reviewed
candidate `f7eee3691a9ce3a1b440fd6a784c7fc264b6512e635e4bd3d2c8405c0a05ba24`.

Only the existing private `at_generic_sibling_function` comma arm changes
production behavior. After the second identifier, each further successful
iteration consumes COMMA and a non-bare-suspend IDENTIFIER. Failure exits and
refuses; a boolean marks third-or-later arguments, requiring GT and plain DOT
for that family. Existing one-/two-argument DOT/SAFE_ACCESS/(QUEST DOT) behavior
and direct name plus observable LPAR remain. No new production helper/module/API,
live type grammar, caller, worker, semantic provider or capability changes.
Nullable N>=3 suffixes and nullable/nested/qualified/annotated/variance/star/
function/intersection/empty/trailing/missing arguments remain excluded. Bare
suspend receiver/every argument and hard in stay declined; escaped names, soft
out, dynamic and trivia keep their existing syntax rules.

The new focused test embeds all165 frozen objects/48 full token-inclusive CST
expectations/231567 original-byte UTF16 entries unchanged as JSON values and
source bytes. Exact parents/subtrees/token partitions/names/diagnostics/ordered
ERROR_ELEMENT ownership preserve duplicate zero-width EOF markers. Strict live
refusal never substitutes Coverage/resource/timeout/cancellation for success.
All positions/four independent stale identities/SAME Parser healthy followup/
unchanged old trees are asserted. LF/CRLF/loneCR, source65536/65537 admission,
raw/byte cuts, whole-token straddles, source-dependent largest lists, leading4401
trivia and actual aggregate4082/4083/4084 prior-copy carriers remain frozen.

Exactly one historical expectation gets additive ID plus exact-source selection:
nonnullable twoarg131 case44 `extra-third-argument`, literal
`class Box<T\nfun Triple<String,Int,Any>.healthy() = 2`. All original raw131
objects and existing nullable-two eight-ID overlay bytes stay unchanged;
nullable-two case51 and all malformed comma controls remain declined.
Other historical tests/dispatch helpers/source literals remain untouched.

The existing cloned window FIRST bounds repetition to256 raw tokens/4096
normalized bytes/effective EOF; every successful iteration advances. Whole copied
windows debit the shared4096 classifier allowance before grammar, including
failed windows and trailing/internal trivia. All source64KiB/worker500ms/64MiB
requested allocation/cancellation/snapshot/other limits remain unchanged.
Static arithmetic and compile success do not establish hard total work, RSS,
stack, cleanup bounds, crash safety or full active-layer B11.

Owner verified accepted prior behavioral phase bytes against exact delivered
recovery/formatting patches without rerunning an old replay, applied only the
new contiguous comma arm, then generated current patches. One fresh authoritative
original archive download, checksum
`ec8d43d9549490689477385c7678bbd22ecbc445973ab364e8ecb69dd4882bcc`, and one
fresh extraction replayed current recovery then formatting with zero fuzz/offsets.
All54 final hashes matched; counts16behavior/50formatting/4pristine. Only
`src/parsing/declarations/type_parameters.rs` changes among54 Rust files;
67 tracked vendor paths remain equal in total:53 unchanged Rust source files
and14 other paths, including notices/manifests/locks.
Initial source-stage phase hashes:
- recovery.patch SHA256 `2436b50d6a879b1a80466f05af68e296143f1e13986621724cbb46d8969f9090`
- formatting.patch SHA256 `50ebde4b22a76fc40cee49b795cf89981883c862577d6eee0f309c4d71a0dabc`
- formatted-sha256.json SHA256 `54594b95342ee33292f0ea8f0cb78dab47dfebb8163e76120dee98e582e7ba2b`

First pinned local `cargo +1.99.0 fmt --all -- --check` and compile-only
`cargo +1.99.0 clippy --locked --workspace --all-targets -- -D warnings` passed.
Clippy used an isolated temporary target directory and executed no test bodies.
No local parser/tokenizer/worker/server/tests/bench/JVM/oracle runtime ran under
#96. Independent adversarial pristine replay and retained SAME3 actual final
source reviews are separate required receipts, not established by this note.
No hosted CI, commit, push, issue or PR occurred in this source stage.

SOURCE_READY requires coordinator acceptance before publication or hosted checks.
Later original pinned hosted format/locked Clippy/full-workspace tests, reviewed
head, exact base lease, fresh standalone read-only guard EXIT0/full PASS then
separate ordinary expected-head merge and OWNER+ROOT exact-main delivery proof
remain gated. No speculative main chase, bypass/admin/settings/force-main.

C13/B3/B4 GAP, fullB11 UNVERIFIED, T008/T008a HOLD, #7/#83 OPEN and #96 Darwin
UNKNOWN remain. No semantics/server providers/performance/full parity promotion.
Native product/core/build/default tests remain Rust-only, with no bundled JVM/
JDK/compiler/JNI/C/C++ analyzer/tree-sitter C. Optional existing external tools
and isolated official oracle remain at their separate boundary, unused here.

## N261 unpublished bounded declined-prefix repair draft

Original PR261 head `efabccee5eb4eee6213d606b4bf9be16da67f70f`, base
`46fd28ddba16ddd6d000588cae56548a7af3d732`, ran original Linux attempt1
38661078501/job116182203658. The runner checkout `04f87b22d108a3b71b66e9d5690b492ac73edfe4`
had the exact reviewed tree `1fc1f56d50a252ee99bd8aa8f865e3a2392dd8ad` and all1023
source files. Format and locked all-target Clippy passed; workspace tests failed
at `compact-n125` with `OutputLimit` after147 preceding N cases passed. Later17
N cases and full-workspace completion remain unobserved. The partial run reported
499 Rust passes, one failure and two ignored helpers; the expected656 total was
not achieved. First raw output, original expectations and failed inspections are
preserved separately and are never replaced by this draft.

Source trace explains the failure: N125 needs direct LPAR at raw257, outside the
unchanged256-token scratch window. Scratch correctly declines. The live missing-GT
fallback consumes only FUN, then258 remaining significant terminals each receive
a top-level error; together with MissingGT this yields259 diagnostics, exceeding
the unchanged256 worker cap. This is a declined-path cascade, not widened scratch
admission or a reason to raise worker limits.

The exact seven-path draft grant SHA256
`446f3aafeca10268b5bd44efd1c33e67853ec88609efb9c0222b4b03b7d3bdcc` permits only
existing type_parameters.rs private predicate/classifier callers, this review
text and five derived vendor artifacts. The same already-debited scratch parse
may return a verified exclusive raw endpoint only after a clean N>=3 simple
nonnullable receiver, GT, plain DOT and direct name, when final LPAR is absent
because scratch reached EOF. Capture occurs immediately after name, before any
trivia-skipping query; checked translation confines the endpoint to the copied
window and effective EOF. Successful found, N1/N2, property, recursive, unverified
and explicit-next-nonLPAR paths carry no endpoint.

Both classifier call sites handle a returned endpoint immediately at the unchanged
candidate cursor. Live recovery groups precisely that prefix into one MissingGT
error, keeping recovered_function=false. Raw-index guards precede EOF/token
queries; explicit EOF/progress protection prevents spinning. The following LPAR,
trailing trivia, unverified suffix and later declarations are not consumed by
this grouping. No second scan/copy/parser, new dependency or public API is added.

Source-only retained reviews identify six affected ownership-only controls:
`no-lpar`, `raw-cut-12`, `byte-cut-12`, `compact-n125`, `byte-lpar-over`, and
`aggregate-one-less`. Each still declines sibling recovery, retains original
CLASS/list beyond the candidate and has no candidate-root FUN. Aggregate fresh-list
recovery and all48 exact full-CST expectations remain required. Independent
historical audits of482 raw executable cases and14 overlay/erratum records find
no new expectation amendments: historical case44 already observes LPAR and takes
found=true, nullable/malformed/N1/N2 controls remain excluded.

All165/new and131/historical test bytes, raw payloads, overlays, expected CST,
ranges, diagnostics, UTF16 maps and stale identities remain unchanged. Raw256,
byte4096, shared4096, diagnostics256, source64KiB and all worker limits remain
unchanged. Static review and compilation do not prove the17 unobserved outcomes,
allocation/deadline behavior, full active-EOF B11, crash safety or full parity.

This is an unpublished source draft only: no new commit/push/fetch/incoming merge,
public-body update, local parser/tokenizer/worker/test runtime or CI rerun is
admitted. Remote PR261 remains pinned to originalEFabc through257 OWNER+ROOT
delivery. Retained SAME2/architect changed-source reviews, pinned formatting,
compile-only locked workspace/all-target Clippy and one fresh pristine ordered
patch replay are required before REPAIR_SOURCE_READY; later runtime/publication/
final-cut/merge gates remain separate. #96 Darwin UNKNOWN, C13/B3/B4 GAP,
fullB11 UNVERIFIED, T008/T008a HOLD and #7/#83 OPEN remain unchanged.

The archived initial behavior-to-final diff for type_parameters.rs also contained
an inherited comma-arm fallback/nesting change; its formatting-only classification
was inaccurate. This draft places that inherited behavior in recovery.patch by
reconstructing the behavior file from repaired final source and restoring only
three known formatting spans (imports and two expect calls). The other53 accepted
behavior files remain byte-identical. New behavior-to-final differences for this
file must be whitespace-only. Original patches and their hashes remain preserved;
new phase counts and hashes are derived from the actual data, not assumed.

Completed draft checks: pinned Rust1.99.0 formatting and compile-only locked
workspace/all-target Clippy both exited0 for source SHA256
873331a853582595578acb34cb5bd738ab19b648484cf6129e86a9a8f4bbf3a0.
No test bodies or local parser/tokenizer/worker runtime were executed. One fresh
pristine extraction and one current recovery-then-formatting replay passed with
zero offsets/fuzz and exact54 final byte/hash matches; notices remained unchanged.
Derived counts are16 behavior files,50 formatting files,54 final files and4
pristine survivors. The two reconstructed single-line expect calls retain final
trailing commas, so this file's new formatting delta is strictly whitespace-only.
New draft phase hashes:
- recovery.patch SHA256 `fcb338c7e9bdc60757e76f4f202c2585ae2ec11ab1626dfe97dde668e5f77eb5`
- formatting.patch SHA256 `4b1abb4ab303923222fa4b0a2121afea2b9f8eaed33f380f95149ad4e3448aa4`
- formatted-sha256.json SHA256 `0b56fc9a47b5b164e2ef4bea073d19af2ce924fddc3412dab30ba6f510c95aa1`

These checks and source-only reviews do not replace the preserved first failed
Linux run or establish a passing repair runtime. Remote head remains original
EFabc; unpublished draft readiness still requires completed retained SAME3
packet review and ROOT admission for any later action.

## C13 exactly two-component qualified generic receiver source

The assigned delivery base is56775ca722425af1f27cf3590ced0dbf97171323,
tree3c269c4b4fe8d8547e3ead366f59a2f79f716a84, sole parent45619398c7d8d5ca7b59acc5178acb4b208a67ee.
N261 OWNER+ROOT original exact-main delivery/vendor release cleared this source
turn. Its tuple-return probe/classifier calls and all165 N fixtures are inherited.
Only the existing private plain-DOT scratch arm adds exactly two receiver
components with one simple unqualified nonnullable argument on the second:
qualifier.Receiver<Argument>.name(. A plain suffix DOT and observable LPAR are
mandatory. The new arm leaves longer_receiver=false and verified_prefix=None;
false alone is not a general proof of old FUN-only decline ownership. No new
helper/module/export/live grammar/server/Cargo/limit change is introduced.

Accepted contract81 cases/40 full token-inclusive CST literals/32 boundaries/
262754 UTF16 expectations remain unchanged. Two exact-ID/source historical
overlays supersede generic qualified-receiver and qualified-decline-generic-second
without rewriting raw payloads or existing nullable/N overlays. Qualified nullable,
two arguments, generic-middle/longer qualified chains and complex arguments remain
declined. Shared4096/256 raw/4096 bytes, active EOF,64KiB and worker supervision
limits are inherited. See ../../crates/ktlsp-parser/reviews/c13-qualified-generic-receiver-recovery.md.

Pinned Rust1.99.0 format and compile-only locked workspace/all-target Clippy
passed, including recheck after two harness findings. No local native/parser/
tokenizer/worker/server/test/bench/JVM/oracle execution occurred under#96. One
fresh current-candidate pristine archive -> recovery.patch -> formatting.patch
owner replay passed with zero offsets/fuzz and all54 final bytes/hashes equal.
Previously accepted replays were not repeated. Other53 behavior/final source
files, upstream archive/inventory, manifests/locks and notices remain unchanged.
Counts stay16 behavior files/50 formatting files/54 final files/four pristine
survivors; type_parameters formatting delta is whitespace-only.

Current derived artifacts:
- recovery.patch SHA256 76dce8c91827b34e9ee6016c72b67f4734d12ed81dc2e7c5d727234b43b902e4
- formatting.patch SHA256 0a6965e7bc53b46ec731a042cf7a4a9083a6cf91965c1376cd44b07707c22452
- formatted-sha256.json SHA256 31152d770cc9c2c4cc926882335f26e98c4d383e2a9ace76b049364f8ee9424c
- final type_parameters.rs SHA256 3b72823d6d0817cba5fe433f519973745bc60e2d4f56db86b9dfc2b6fa48ae1b

Retained correctness/adversarial/architect actual-source and original unchanged
hosted Linux fmt/locked Clippy/workspace-test reviews remain required. Publication
is authorized; merge waits for an exclusive exact root head/base lease and fresh
standalone read-only guards, followed by expected-head merge and OWNER+ROOT
original exact-main verification. No full C13/B3/B4/B11, T008 or semantic parity
promotion follows. C13/B3/B4 GAP, fullB11 UNVERIFIED, T008/T008a HOLD, #7/#83 OPEN
and #96 Darwin OPEN/UNKNOWN remain.

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

## #100 review continuation

The new classifier recovery uses isolated bounded scratch parsing and inspects
specific completed type productions, preserving ordinary non-class behavior.
Both independent reviewers supplied production regressions for annotation/operator
state preservation, incomplete bounds, excluded headers, entry deduplication,
exact raw-token/byte thresholds, repeated/aggregate exhaustion and recursion
suppression. Adversarial review independently reproduced all54 final source bytes
from the archive and both ordered patches, with the unchanged Rust dependency
graph. Production exact-head Linux execution is the behavioral authority; the
standalone recovery tests compile locally but are not run by root workspace CI.
The first Linux failure was a new test's blanket nested-FUN prohibition; corrected
parent/range assertions passed on candidate054e352 in Linux
run38285975233/job114934324677, actual synthetic checkout86bbd04/treead10ce11
(full identities in the production parser REVIEW.md). Both independent reviewers
approve after inspecting all169 active passes/zero failures, two exercised helper
fixtures and19 worker fault cases. Final integrated-head rechecks remain required.

## Focused accounting review and explicit contract amendment

Both independent reviewers found that leading-trivia scans and cheap
kind/modifier/EOF misses are uncharged, despite the original broader accounting
reading. Their earlier candidate behavior approvals did not establish that
broader contract. The coordinator explicitly amended it after independent
architect review: at most 4096 copied scratch tokens per classifier list, counting
internal/trailing copied trivia and charging every eligible window before grammar
success/failure; a zero-copy eligible rejection debits one. Leading scans and
cheap prefilter misses may remain outside the allowance. The 256 copied-token/
4096-source-byte attempt caps, minimum active EOF and recursion guard are retained.

The leading scan increments a raw index until nontrivia or lexeme end. Every list
backedge requires and consumes a comma; ordinary token queries normalize skipped
trivia before continuing. Scratch recursion returns before this scan. Together
with the unchanged 64 KiB input limit, this is a local finite-progress argument,
not proof of global linear parsing, total work or RSS. Existing internal-trivia
threshold and failed-eligible aggregate exhaustion/reset fixtures remain relevant;
they do not establish a precise aggregate acceptance threshold.

The focused production-worker fixture places 4401 leading trivia tokens
(13204 bytes) directly after '<', then pins exact damaged class/list and sibling
name/function/parent ranges, entry diagnostic, token partition, snapshot identity
and healthy followup. Format and workspace test-source compilation pass; no
runtime execution has occurred for this addition. A public active-truncation
control alone would not prove negative clipping. Root explicitly authorized a
private const predicate extraction in the owned type-parameter module and accepts
that narrow guard evidence. The actual window loop uses it unchanged; unconditional
asserts cover EOF before/at/inside a token, exact endpoint, absent EOF and independent
4096/4097-byte bounds. The minimum active EOF reaches this real guard; zero copied
tokens return false before scratch construction, and a prefix cut before its
required FUN/name/LPAR cannot pass. This is window-predicate evidence, not annotation
parsing, active-layer integration or end-to-end truncated recovery. That limitation
remains for #83; it does not admit T008 or broader acceptance.

Ordered archive -> nine-file behavior -> 49-file formatting -> all54 final source
hashes replay passed after extraction. Pinned format, locked workspace test-source
compilation and Clippy pass without local runtime. The new worker regression still
awaits final integrated Linux execution; separate resource/provenance and finished
architect reviews remain required.

Source-only correctness review caught an ordinary comment-binding fixture issue
before execution: same-line block comments would bind to the damaged class. The
fixture now brackets the comment/space run with blank lines, independently keeping
class/list and bare sibling boundaries fixed. The leading scan still starts
immediately after '<' and crosses the whole run before candidate copying.

Both independent reviewers approve source preparation after rechecking the
corrected blank-line fixture: 4401 raw trivia tokens/13204 bytes, FUN13214/name
13218..13225, class0..10/list9..10 and entry diagnostic10. Adversarial review
independently replayed the updated archive/ordered patches to all54 final source
bytes; correctness checked the final hashes and actual guard call path. They
withhold final behavior/resource approval until integrated Linux execution.

Owner architect approves this focused preparation: actual-predicate preservation,
large-leading-trivia fixture reach and honest accounting/EOF limitations match
the root-amended contract. This is conditional preparation approval, not delivery
or renewed runtime approval. Final integrated Linux checks and both reviews remain.

## Authorized final integration

Exact main ef120beebb45f96a60bf61866fe15dd228b674bc, tree
f657f81e16237c1ba0903169ab94fd3e2747eafe, was integrated once into focused
preparation f7d56979539cdda7c4216733fd6647bf75ef782f at merge
78e9df525145df31a8dd6ef2b56d31dac53ce7af. The integration changed no owned
parser/vendor source or test bytes and preserves current-main binary exports,
shared docs, manifests and lock exactly. Ordered archive/patch/final-hash replay
passed again. The authoritative combined-head Linux workflow, new leading-trivia
runtime outcome and renewed separate reviews/finished architect remain pending.
Root expected-head merge authorization and exact merged-main checks are still
required. No local runtime or JVM/oracle ran; #7/#83/#96 remain open.

## C13b / #136 finite source preparation (not delivered behavior)

The source-authored contract was approved by the SAME correctness/functionality
and adversarial reviewers and owner architect at frozen37bfe388/tree8ba503ba.
Root then admitted finite source and exact issue publication only. The private
optional-DOT expression preserves old no-dot short circuit, existing caller gates,
completed-bound check, copied-window accounting and scratch suppression/state
isolation. The broader recovery exclusions remain. The single String-extension
negative member is deliberately superseded by exact positive assertions.

The new focused test includes a literal selected-field copy of the reviewed final
70-seed/38-projection contract rather than recalculating expectations from parser
output or reusing the obsolete intermediate generator. Complete selected sequences
include expression-body this references, exact declaration names/nearest owners,
receiver/list/parameter hierarchy and ordered diagnostics. Compatibility-only cases
remain explicitly limited. A corrected source trace proves that single-token
suspend reaches USER_TYPE because DOT-truncated lookahead prevents modifier
consumption; no missing-type or semantic validity inference remains.

Owner data-only replay passed from the original checksummed archive through the
ordered recovery/formatting patches to all54 final source hashes. Counts are14
behavior/50 formatting/four pristine survivors; only type_parameters.rs final Rust
bytes differ from the source base. Details, exact fixture/artifact hashes and
compile-only/source-review receipts are in
../../crates/ktlsp-parser/reviews/c13b-generic-extension-recovery.md.
No local runtime test occurred. Source approval is preparation only; future
coordinator integration, exact Linux all-three/completed CI review, guarded merge
and owner/root main verification remain required. No current delivery/parity or
Darwin96 correction is asserted; #83/#7 stay open and T008/T008a remain held.

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

The finite property branch and exact frozen production contract are described in
../../crates/ktlsp-parser/reviews/c13-property-recovery.md. Both preliminary
reviewers found that the initial revision-2 assertion also changed source; an added
same-source/document/kind revision-2 mismatch isolates revision rejection. Product
source and frozen expectations did not change in that correction.

Pinned formatting and compile-only locked workspace all-target Clippy passed. No
local native runtime ran. Actual SAME correctness/adversarial final source reviews,
independent patch replay, architect review and completed original Linux Native Rust
all3 remain required. Broader C13/B3/B4 GAP, B11 UNVERIFIED, T008/T008a HOLD and
#7/#83/#96 OPEN/Darwin UNKNOWN remain unchanged; no full parity or Darwin-fix claim.

### Original PR170 failure and one historical dispatch reconciliation

Original run38497231040/job115635663818 attempt1 on reviewed78b6c18/tree
c2556e29014085749715d32028ef58b981d5281b passed formatting/Clippy and failed
Tests at recovery_generic_extension_functions.rs:190, decline-header-3's
class.end>11 assertion. Raw log SHA256
a81796c31887a2e8b630d8c513ce6a72802b85b0f42d6ebda03a4eb7861ecd77
(56377 bytes/715 lines) is retained independently by owner and root. The focused
property suite was not reached; the failure establishes no complete recovered CST.

The historical 25-byte source is identical to the preimplementation seed-lf
positive. Owner and all three reviewers missed this duplicate negative. After
actual SAME2+architect failure/proposal reviews, root admitted only stable-ID
decline-header-3's dispatch using the unchanged frozen four-row CLASS/list/T/
PROPERTY projection, direct names and sole Missing '>' diagnostic. All 70 historical
JSON fixtures remain byte-identical, as do every other dispatch/assertion and all
other function/nullable/qualified tests. Frozen property JSON is unchanged.
This adjacent exception supersedes only the earlier blanket dispatch-preservation
claim; it adds no product grammar, patch, resource or expectation changes.

The corrected head requires actual SAME source rechecks and new original unchanged
Linux Native Rust all3. No unchanged retry or local native execution is used. Merge
remains HOLD for root's expected-head/base lease, completed CI approvals and
owner/root exact-main verification; broader parity and Darwin gates stay unchanged.

### Corrected PR170 original failure and nullable initializer ownership

Corrected original run38498826024/job115640658803 attempt1 on reviewed851901e/tree
15d24b7d1521ac708a660c65c8e5ebfcc971ad28 passed formatting/Clippy, C13b70 and
long-qualified116 tests, then failed nullable-decline-safe-call-expression's
class.end>11 assertion. Raw log SHA256
82ac183ac404a8f3bbdfe0618959b8a441a41af13740d1a3b6e5bcfc1c74a8b4
(58486 bytes/741 lines) is retained independently by owner and root. The focused
property suite was not reached. Neither this failure nor the first establishes
a complete recovered property result. Both original failures remain preserved.

Three static audits covered all 330 historical embedded cases and nonembedded/
generated parser inputs. Only the corrected C13b seed and these two nullable
initializer controls cross an unfinished classifier with an ordinary val header.
Owner and reviewers missed val value while first searching for val later. Root
admitted only nullable-decline-safe-call-expression and
nullable-decline-safe-call-elvis-expression dispatches after actual SAME2+architect
source-derived supplemental-contract review. Supplemental JSON SHA256
bf8a8e17aee78e01cc429385e6cf421c77ca4fef6000992d48b196a1df548bc0
is embedded unchanged: six-row projections, exact safe-call/Elvis expression-parent
chains, names, sole Missing '>' diagnostic, 30 original UTF16 positions and no FUN
nodes preserve the original function-lookalike ownership intent. Expectations
come from grammar and source arithmetic, never native output.

All 85 historical nullable fixture bytes and every other dispatch/assertion remain
unchanged, as do 70/59/116 payloads and all 36 new property fixtures. Only these two
old class-boundary invariants are superseded. Grammar, vendor source/patch hashes,
limits, APIs and workflows remain identical to851901e. Common diagnostic/token/
snapshot/healthy-followup checks still execute. Changed-source SAME reviews and
new original unchanged Linux Native Rust all3 remain required; no local native or
unchanged retry is used. Merge remains HOLD for root's lease and exact-main checks.

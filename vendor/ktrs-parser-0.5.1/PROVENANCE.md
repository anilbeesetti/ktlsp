# T007b2/T007b3 source-local parser repairs

Child #87 of #83/#7; production development follows merged #86 and explicit coordinator handoff.

Sources copied from the crates.io `ktrs-parser` 0.5.1 archive, SHA256
`ec8d43d9549490689477385c7678bbd22ecbc445973ab364e8ecb69dd4882bcc`.
`upstream-sha256.json` inventories original Rust source bytes. Upstream repository:
https://github.com/Hexay/ktrs (research pin a703f88d6d4ac9d983a17df66ddf6ac403546c00).
The registry archive is authoritative for this fork. License/NOTICE files retained
from the previously audited project notices; the published archive omitted them.

Only Rust `src/` is copied, including upstream unit-test source, without upstream
examples/integration fixtures/workspace or dev dependencies. Minimal standalone
manifest pins existing `ktrs-lexer`/`ktrs-syntax` 0.5.1; their sole transitive normal
dependency is `text-size` 1.1.1. No external dependency addition. Upstream lib unit
tests depend on expect-test and are not run by this standalone harness; no
mimalloc or facade/native parser dependencies are added. Ticket tests are original.

The product parser selects this fork as a path dependency. The root workspace
excludes the vendor workspace, keeping upstream dev dependencies out of default
tests. Production regressions live in crates/ktlsp-parser and traverse the actual
supervised worker through parse_snapshot/tree_for. Standalone tests alone do not
establish delivery; integrated review/checks/CI remain required. Direct upstream
parse_file requires LF; CRLF/lone CR production tests exercise the existing
normalized-to-original mapping through the supervised worker. Original input source
is never modified by this repair. Full Kotlin grammar and semantic parity remain pending.

Source changes are recorded in two ordered phases:

1. `recovery.patch` changes sixteen files from the pristine archive: lib.rs,
   parsing/abstract_kotlin_parsing.rs, parsing/mod.rs,
   builder/layers.rs, declarations/{annotations,classes,functions,members,mod,modifiers,parameters,type_parameters,types,user_types}.rs
   and expressions/{mod,primary}.rs.
2. `formatting.patch` changes 50 files from the current recovery phase (extending historical cc4920d),
   using pinned Rust 1.99.0 rustfmt with edition 2024. All sixteen behavioral files
   are among those 50; the other 34 have formatting changes only. Four source
   files retain pristine bytes. Exact paths are the patch headers; the final
   54-file byte inventory is `formatted-sha256.json`. No formatting opt-out or
   changed root formatting command is used.

Applying recovery.patch then formatting.patch to the inventoried pristine source
reproduces all final source bytes. Formatting-only review and production rechecks
are separate from the earlier behavioral evidence; commas and import order need
explicit review in addition to whitespace. Original archive hashes and notices
remain unchanged.

The behavioral patch checks ordinary function parameter boundaries, probes existing modifier
and annotation grammar with rollback, and reports a missing ')' without consuming
a following declaration. The finite C14 default-entry, C15 parenthesized-type and C16 annotation-argument
repairs are described below.
Constructor, function-type and context parameter loops keep their original context.
EOF diagnostics retain upstream behavior. A scoped speculative-probe flag avoids
recursive replay when annotation arguments contain anonymous functions. Remaining
work is superlinear in nesting; existing production resource gates remain required.

Known bounded repair limits: missing defaults outside C14's complete simple zero-parameter header,
malformed parenthesized types outside C15's finite ordinary-parameter/simple-zero-parameter-header
contract, malformed annotation arguments outside C16's direct ordinary-parameter/simple-zero-parameter-header
contract, and unclosed class generic lists
outside the bounded #100 simple-function-header and finite C13b
lexically simple extension-function-header contracts can still consume later declarations. These are
upstream grammar/recovery gaps, not accepted recovery or full grammar parity.
Missing simple types and open generic argument types improve recovery but retain
upstream diagnostic deduplication (e.g. only `Type expected` at one offset).

Run the bounded source-local checks with:
`cargo test --offline --locked --manifest-path vendor/ktrs-parser-0.5.1/Cargo.toml --test recovery`

`worker-probe/` preserves the isolated production-source experiment without code
duplication: its manifest references existing production worker/library/tests and
patches ktrs-parser only in its standalone graph. Run:
`cargo test --offline --locked --manifest-path vendor/ktrs-parser-0.5.1/worker-probe/Cargo.toml --tests`
Neither standalone workspace is run by root workspace tests. Equivalent production
regressions now live in the parser crate and must pass integrated review/checks/CI
before #87 can close.

## #100 bounded classifier-generic recovery candidate

The additional behavior is confined to class/interface type-parameter boundaries;
objects and other generic callers retain ordinary parsing. Completed bounds require
specific surviving completed TYPE_REFERENCE/concrete-type production events and
closed type/value lists, not a void-helper return or merely no new diagnostic.
An isolated scratch parser reuses existing annotation/modifier grammar and current
committed token kinds, then requires FUN/name/LPAR. No live cursor, token remap,
production, diagnostic, newline/join or layer state is changed by a probe.

The scratch slice stops at the minimum nonnegative active truncation endpoint,
256 copied complete raw tokens including internal/trailing trivia, or 4096 source
bytes. Every eligible scratch window, including failed grammar, is charged before
parsing against 4096 copied raw tokens per classifier list; zero-copy eligible
rejection debits one and exhaustion declines recovery. Cheap kind/modifier/EOF
prefilter misses remain uncharged under the coordinator-amended contract.
Leading trivia inspection is bounded separately by input size and the advancing
live cursor; nested scratch probes return before scanning it.
These are recovery-coverage bounds, not new worker failures or larger global limits.
The existing recursion-suppression flag disables nested parameter/generic probes.
Scratch byte limits do not bound RSS: the existing builder pool can reuse larger
buffers. Original archive checksum, upstream inventory and notices are unchanged.

The layer minimum uses compile-time assertions for absent, negative, nested,
reordered and zero boundaries. Production tests exercise exact declaration and
name ranges/parents, damaged class/list extents, same-offset diagnostic dedup,
original newline mapping, valid controls, incomplete bounds/excluded headers,
per-probe thresholds, aggregate decline and repeated recovery. Runtime evidence
must come from the authorized exact-head Linux CI; no new Darwin runtime test is
used for this ticket. Standalone vendor tests are not part of root CI.

A focused post-candidate audit identified that the original all-trivia/all-failure
reading was broader than the implemented copied-window counter. The coordinator
explicitly amended the private contract to the accounting above after independent
architect review; documentation alone did not resolve that original mismatch.
The actual loop now uses a private const byte/EOF predicate with unconditional
assertions for endpoints before/at/inside a token, absent EOF and independent
4096/4097-byte boundaries. This extraction preserves the original subtraction,
inclusive endpoint comparison and absent-EOF behavior. The minimum active EOF
feeds that predicate; an empty window returns false before constructing scratch,
and incomplete prefixes cannot satisfy FUN/name/LPAR. These are window-predicate
checks, not annotation parsing, active-layer integration or end-to-end truncated
recovery evidence; that remaining gap belongs to #83's acceptance audit.
Large-leading-trivia runtime evidence and renewed resource reviews remain pending.
No full-parser complexity or RSS bound is asserted.

## C14 / #121 ordinary missing-default entry

Only parameters.rs changes behavior in this child. Call-local ordinary-list
recovery context excludes constructor/function-type/context/direct parameter
paths without ambient Parser state. At post-EQ entry an isolated scratch builder
uses four grammar expectations to recognize FUN/IDENTIFIER/LPAR/RPAR. The window
ends within active truncated EOF,256 complete raw tokens including internal trivia,
4096 bytes, or four nontrivia tokens. The recursion-suppression flag remains
separate. There is no live speculative mutation or text-search reconstruction.
Broader headers remain recovery gaps; leading trivia inspection is bounded by the
input limit, outside copied-window accounting. Worker/resource/Coverage contracts
are unchanged. The ordered phases were recomputed as9 behavior/49 formatting/54
final source files with5 pristine survivors, not inferred from historical counts.
Source-frozen expectations and review requirements are in
../../crates/ktlsp-parser/reviews/c14-default-recovery.md. Controlled Linux validation
and coordinator delivery gates remain required; no new Darwin execution is used.

## C15 / #123 ordinary malformed parenthesized type

Only parameters.rs and types.rs change behavior in this child. The ordinary-list
call-local context also reaches parameter type contents, recursively through
parentheses and intersections. General, constructor, function-type, annotation,
type-argument and classifier entries retain their existing context. Before
parenthesized-type rollback into function-type parsing, the unchanged C14 bounded
scratch grammar header check can retain the damaged type and annotation, leaving
the complete simple named function for the outer declaration loop. No parser
state field, new probe implementation, dependency, builder policy or resource change is
introduced. Existing same-lexeme error-item behavior preserves the deepest
Type expected diagnostic, without blanket equal-offset error-marker suppression.

Ordered replay from the original checksummed archive recomputed10 behavior/49
formatting/54 final source files with5 pristine survivors. Archive/upstream hashes
and notices are unchanged. Complete source-derived expectations for three original
newline spellings and four valid LF controls are frozen in
../../crates/ktlsp-parser/tests/recovery_parenthesized_types.rs; source review and
pending controlled Linux validation are recorded in
../../crates/ktlsp-parser/reviews/c15-parenthesized-type-recovery.md. C14 controls,
C16 residuals and existing incomplete-bound assertions remain unchanged. This
finite syntax repair leaves #83/#7 open and T008 held; #96 remains OPEN/UNKNOWN.

## C16 ordinary parameter annotation arguments

Only declarations/{annotations,parameters,mod}.rs, expressions/{primary,mod}.rs
and parsing/mod.rs change in this child. Separate scoped parsing context permits
bounded sibling recovery only in a direct annotation argument list in eligible
ordinary parameter modifiers. All ineligible modifiers, annotation callees/type
arguments, nested expressions, ordinary calls and block/lambda reparses are scoped
Nested; the latter conservatively excludes local functions in reparsed bodies.
An actual direct-list stop is consumed by its own parameter before context
restoration. Scoped callbacks restore normal early returns and speculative
rollback; no panic-safe guarantee is asserted. Scratch suppression remains
separate, and the C14 four-terminal probe body and its per-call limits are unchanged
(internal visibility only widens). Additional individually bounded invocations
do not establish whole-parser linearity. Existing item-versus-marker diagnostics
remain distinct; no builder policy or global equal-offset deduplication changes.

Ordered pristine-archive replay recomputed14 behavior/50 formatting/54 final
source files with4 pristine survivors. Original archive/upstream hashes and
notices are unchanged. Complete pre-runtime source-derived expectations for
LF/CRLF/CR, three exact valid LF controls and one lazy-body exclusion are recorded
in ../../crates/ktlsp-parser/tests/recovery_annotation_arguments.rs and
../../crates/ktlsp-parser/reviews/c16-annotation-argument-recovery.md. Existing
C14/C15/classifier/incomplete-bound/resource assertions remain read-only.
Local preparation is not runtime delivery: same-reviewer source/provenance checks,
coordinator-assigned Linux all3 and exact-main validation remain required.
Initial local preparation retained an automatic-approval publication hold; the
coordinator later published child #126 after direct human authorization. Integrated
Linux validation remains pending. #83/#7 remain open, T008/T008a held, #96 unresolved;
no full parser or semantic parity is claimed.

## C13b / #136 simple extension sibling headers — source preparation

This child changes only declarations/type_parameters.rs behavior: existing scratch
FUN/IDENTIFIER/LPAR recognition also permits one DOT and required IDENTIFIER before
LPAR. The exact prior FUN/IDENTIFIER/LPAR branch, class/interface callsites, completed
bound gate, isolated builder, recursive-probe suppression, minimum active EOF and
256-raw-token/4096-byte/4096-copied-tokens-per-list accounting are unchanged. Generic,
qualified, nullable and parenthesized receivers and other sibling kinds remain
outside positive acceptance. Bare dynamic uses DYNAMIC_TYPE; get, backticks and
single-token suspend retain normal USER_TYPE parsing. DOT-truncated lookahead is
None for suspend, so the modifier-consumption guard declines; no semantic type
validity is inferred. No receiver/type/function callback is added to the probe.

The focused production test embeds a direct selected-field copy of the final
source-authored contract JSON (SHA-256
5ea819780683402160c821c6e058d07a652c155a09bce80842e5ddaf28ae3097):
70 exact sources, 38 complete selected projections and 779 original-source UTF-16
endpoint expectations. Remaining cases assert explicitly limited declined-header,
incomplete-bound, annotation-owned-function, copied-window/aggregate/reset and
malformed-parameter compatibility invariants. No complete declined AST is invented.
Only the previous String-extension exclusion member is removed from declarations.rs;
all other C13a/C14/C15/C16/resource tests remain unchanged. Every case checks original
token partition, snapshot identity, stale revision rejection and healthy followup.
These assertions have not run locally; no runtime or behavior acceptance is claimed.

Data-only archive/ordered-patch replay verifies all 54 final source files: 14
behavior files, 50 formatting files, four pristine survivors. Exactly one final
Rust source differs from the frozen initial37bfe388 source. Original crate archive
SHA-256 ec8d43d9549490689477385c7678bbd22ecbc445973ab364e8ecb69dd4882bcc,
upstream inventory, manifests/locks and third-party notices are unchanged.
Current source-stage review and compile-only receipts are recorded in
../../crates/ktlsp-parser/reviews/c13b-generic-extension-recovery.md. Push/PR/CI,
integration and merge remain coordinator-gated. #83/#7 OPEN, C13 aggregate GAP,
T008/T008a HOLD, B3/B4 GAP, B11 UNVERIFIED and #96 OPEN/UNKNOWN remain unchanged.

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

[Issue169](https://github.com/anilbeesetti/ktlsp/issues/169) adds only a first-token
VAL_KEYWORD scratch branch in declarations/type_parameters.rs, requiring the
existing IDENTIFIER expectation then EQ. Modifier/annotation prefixes retain the
unchanged function predicate; typed/destructured/var/receiver/generic/delegate
properties remain excluded. Caller/completion/EOF/scratch/debit guards are unchanged.

Fresh owner pristine archive -> recovery.patch -> formatting.patch replay matches
all 54 final source hashes, zero offsets/fuzz: 14 behavior files, 50 formatting
files, four pristine survivors. Archive/upstream hashes, manifests, locks and
notices are unchanged. Independent adversarial fresh replay is separately required.
The frozen contract, phase hashes and review/validation gates are recorded in
../../crates/ktlsp-parser/reviews/c13-property-recovery.md.

## #83 B3 original-deadline grammar observation — source stage

On assigned PR195 main3baf05494c142416c8cd4641707e1c7dbed0a539, only final
vendor lib.rs and parsing/abstract_kotlin_parsing.rs gain behavior: a scoped
thread-local fn() observer wrapper and dispatch at actual Parser::eof, immediately
before its unchanged builder predicate. Default calls now check TLS too; overhead
and timing/allocation perturbation remain unmeasured. No grammar/recovery/token/
builder/layout/EOF-truth change is claimed. The observer includes nested/lazy
grammar contexts and does not establish B11 active-EOF acceptance.

Fresh owner pristine archive -> recovery.patch -> formatting.patch replay verifies
all54 final Rust files with zero fuzz/offsets:16 behavior files,50 formatting files,
four pristine survivors. The original archive/upstream inventory/notices and both
manifests/locks are unchanged. The independent adversarial reviewer must perform
a fresh separate replay before source approval. No source origin is reassigned.

The production witness, strict healthy controls, socket/error/perturbation limits
and exact v3 contract identity are in
../../crates/ktlsp-parser/reviews/deadline-observation.md. No local native runtime
ran; original unchanged Linux CI remains required. No positive witness or B3
promotion is claimed in this source-stage note; B4 remains GAP, B11 UNVERIFIED,
T008/T008a HOLD and #7/#83/#96 OPEN, Darwin UNKNOWN.

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
Current phase hashes:
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

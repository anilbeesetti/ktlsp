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

1. `recovery.patch` changes fourteen files from the pristine archive: parsing/mod.rs,
   builder/layers.rs, declarations/{annotations,classes,functions,members,mod,modifiers,parameters,type_parameters,types,user_types}.rs
   and expressions/{mod,primary}.rs.
2. `formatting.patch` changes 50 files from the current recovery phase (extending historical cc4920d),
   using pinned Rust 1.99.0 rustfmt with edition 2024. All fourteen behavioral files
   are among those 50; the other 36 have formatting changes only. Four source
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
reviews/rechecks and future root-assigned integration/unchanged original LinuxCI/
guarded merge/main-verification gates remain pending. Automatic approval review
rejected finite issue publication; no issue was created. Push/PR/CI/integration/
merge remain HOLD. Exact frozen fixture/ordered-patch hashes and limits are in
../../crates/ktlsp-parser/reviews/c13-qualified-extension-recovery.md.
#83/#7 OPEN; C13 GAP; T008/T008a HOLD; B3/B4 GAP; B11 UNVERIFIED;96 UNKNOWN;
historical45/32/10/3 unchanged. No semantic/server/RSS/official-performance claim.

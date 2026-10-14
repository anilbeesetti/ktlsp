# Kotlin 2.3 declaration baseline v1 — #86/#87/#100/#104, partial #83

This independently authored native corpus pins the source-local ktrs-parser 0.5.1 fork and registry syntax 0.5.1, not
compiler or official LSP parity. Source literals and exact expected extents,
declaration-relative name offsets, immediate CST parents, nearest declaration
parents, and diagnostic spans/messages live in `tests/declarations.rs` and
`tests/recovery_residuals.rs`. Expectations
are hand-authored, not captured worker snapshots. Every parse uses the real
supervised Rust worker. The parser crate has no provider/HIR/server caller.
Delivered recovery is recorded in PR90/#87 and PR102/#100. The final refreshed
[#83 audit](https://github.com/anilbeesetti/ktlsp/issues/83#issuecomment-6130232640)
freezes parser source `ab00d55557cfea9bcee87aa41fe2922f82770027`. C17/#104 starts
from actual main `4d27673f703fd3c7b0ee57fc7a3cf35c6c21f05f`; only delivered metadata
changed between those checkpoints. The C17 delta changes no parser/vendor/API/worker/Cargo/lock files. Its one final
integration base is `0a1a863744a2bc520dac46792a5ce1eebe5b70dc` (delivered PR106/C12);
all inherited source, existing tests, dependencies and tools are preserved.
The audit classification SHA256 is
`b54043bde87143ce0310e18cb2a67bc5f46fcd7b285966e3a3f8076d9267fb1c`.

## Declaration fixtures and assertion limits

| Named test | Contract |
| --- | --- |
| baseline_preamble_and_declaration_kinds | package; import alias/star; class/interface (both CLASS, distinct keyword tokens); object; function; property; typealias; exact declared names and hierarchy |
| baseline_modifiers_annotations_generics_constraints_and_nested_members | annotation/modifier extents; class type parameter/bound; where constraint extent excludes keyword; member property/extension function/nested class |
| baseline_receivers_and_locals | property receiver/accessor; generic function receiver/constraint; local class/function/property owned by enclosing function through BLOCK |
| baseline_incomplete_declarations_retain_later_class | missing class/interface/object/function/property names and extension-function name; missing typealias name/type; later class retained |
| baseline_incomplete_preamble_alias_star_and_annotation_recovery | incomplete package/import alias; invalid star alias; missing annotation name; later declaration retained; exact diagnostic spans/messages |
| baseline_original_unicode_newlines_backticks_and_eof | LF/CRLF/lone CR; consecutive/terminal newlines; astral/combining Unicode; backtick name; original source and UTF-16 boundaries; malformed function EOF |
| malformed_parameter_lists_restore_original_snapshot_declarations | exact top/member/local restoration through revisioned production API for LF/CRLF/lone CR, Unicode/backticks, original malformed boundaries/diagnostics/names/parents/ranges |
| repeated_parameter_recovery_retains_annotation_prefix_and_eof | two broken lists preserve annotated next declaration and later typealias; exact EOF diagnostics |
| valid_parameter_contexts_preserve_default_declarations | nested anonymous function/object declarations retain local parents/ranges; multiline/default/lambda/call/generic/annotation/receiver/constructor/function-type controls |
| malformed_member_parameters_preserve_initializers_and_constructors | initializer/secondary constructor retain original spans and class ownership; local property belongs to initializer |
| recovery_limits integration tests | nested annotation controls now pin all anonymous FUN identities/ranges/owners and absent direct names; repeated recovery pins the healthy direct name; strict NestingLimit/diagnostic-overflow and healthy followups remain; generic threshold/leading-trivia/aggregate/scratch controls remain resource-only |
| valid_classifier_generics_and_nonclass_callers_are_preserved | exact class/named function/property/typealias projections for all nine existing valid controls, including constructor function-type bound and receiver generics |
| snapshots_reject_changed_revision_document_kind_and_reused_identity | exact immutable input/result association, stale revisions/documents/kinds/same-revision changed text, cancellation and retained original |

C12/#103 was delivered in [PR106](https://github.com/anilbeesetti/ktlsp/pull/106),
merge `0a1a863744a2bc520dac46792a5ce1eebe5b70dc`. It adds exact projections to the
six parameter/default and nine classifier/nonclass valid controls, original
snapshot property and member recovery, plus anonymous annotation ownership and
the repeated-recovery healthy name. Existing fixture literals/invocations remain
unchanged. [Exact merged-main Linux CI](https://github.com/anilbeesetti/ktlsp/actions/runs/38332673276/job/115085727770)
passed all three pinned commands: 226 active tests, two exercised ignored helpers
and 19 separately counted protocol faults. Generic threshold/leading-trivia/
aggregate/scratch controls remain resource-only; C12 adds assertions, not recovery
or B3–B5 resource evidence. The frozen audit above describes the earlier source
checkpoint; this paragraph records only the subsequently delivered C12 delta.

Nodes use upstream kinds without inventing an interface kind. An omitted package
is an empty PACKAGE_DIRECTIVE, excluded from the named-declaration projection;
full tree validation still checks it. Package/import qualified names remain CST
REFERENCE_EXPRESSION/DOT_QUALIFIED_EXPRESSION children, not resolved names.
Offsets are UTF-8 source spans, never symbol identity.

## Unresolved recovery and grammar gates

Child #87 repairs unclosed ordinary function parameter-list boundaries in top,
member and local contexts. Production `parse_snapshot`/`tree_for` tests assert
exact malformed declaration extents, later kinds/direct names/original UTF-8
ranges, immediate and nearest declaration parents, diagnostics and token partition.
LF/CRLF/lone CR, astral/combining Unicode/backticks, EOF, repeated recovery and
annotation/modifier prefixes are checked. Valid multiline/default expressions,
lambdas/local declarations, nested calls/generics/annotations/receivers and
constructor/function-type parameter contexts are controls. Member initializer
and secondary-constructor boundaries have explicit original-span/parent checks.
This does not accept all malformed syntax or complete #83/#7 recovery.

`unclosed_classifier_generics_restore_named_sibling_snapshots` replaces the recorded
class-generic defect with exact class/list, sibling kind/name/range/parent
and diagnostic expectations. Entry-boundary errors retain same-offset deduplication.
PR102/#100 delivered the bounded repair; exact final evidence appears below.
`token_loss_still_rejects_coverage_and_allows_healthy_followup` separately preserves
Coverage rejection for `{fun<)]<T:@( {})` and its healthy followup.

## C17 finite residual ledger — #104

`tests/recovery_residuals.rs` supplies the three previously missing complete inputs.
Each literal ends at `2` with **no terminal newline**; all use Source mode through
production `parse_snapshot`/`tree_for`, document `c17`, revision 17. Assertions pin
all CLASS/OBJECT_DECLARATION/FUN/PROPERTY/TYPEALIAS nodes, exact direct identifiers,
original spans, immediate CST parents and nearest declaration spans, the complete
diagnostic sequence, immutable snapshot association and contiguous original token
partition. Revision 18 parses `fun healthy() = 2` with exact FUN/name/parent/range
and no diagnostics, then rechecks the original result. No tree is reconstructed.
These known-defect assertions are **required recovery GAPs**, not accepted recovery.
The exact diagnostic tuples are independently authored in the test, not whole-tree
snapshots. Exact-head execution/review/delivery status is recorded in
[PR109](https://github.com/anilbeesetti/ktlsp/pull/109); this source-authored contract
does not substitute for that controlled Linux evidence.

| Audit family / exact source literal | Named assertion / expected current defect |
| --- | --- |
| C14 `fun broken(x: Int =\nfun healthy() = 2` | `missing_default_consumes_later_function_as_parameter_expression`: broken FUN consumes the file; healthy FUN belongs to VALUE_PARAMETER inside broken instead of KT_FILE |
| C15 `fun broken(x: (@A\nfun healthy() = 2` | `malformed_parenthesized_type_consumes_later_function_header`: broken FUN consumes the later header; healthy has no declaration node; exact diagnostic spans distinguish the nested type failure and remaining top-level tokens |
| C16 `fun broken(@A(\nfun healthy() = 2` | `malformed_parameter_annotation_consumes_later_function_as_argument`: healthy FUN belongs to VALUE_ARGUMENT inside broken's unclosed parameter annotation; this is the smallest named representative of the audit's fragment-only annotation family |

The following existing inputs/assertions in declarations.rs
`failed_generic_probes_preserve_type_parameters_and_existing_recovery` are reused,
not duplicated. Their current expectations are narrower negative evidence, not full
exact declaration/diagnostic projections. All append the stated header after one LF.

| Existing complete input construction | Existing asserted outcome and limit |
| --- | --- |
| `class Box<T : List<Int\nfun healthy() = 2` | diagnostics nonempty; healthy is not an exact KT_FILE FUN; original token spelling retained |
| `class Box<T : (Int\nfun healthy() = 2` | same bounded negative assertions |
| `class Box<T : Int.\nfun healthy() = 2` | same bounded negative assertions |
| `class Box<T : Int &\nfun healthy() = 2` | same bounded negative assertions |
| `class Box<T : @A(1\nfun healthy() = 2` | same assertions plus exact nested healthy FUN/name span under VALUE_ARGUMENT and class/type-list extents through EOF |
| `class Box<T\n` + `fun`, `fun () = 2`, `fun <U> healthy() = 2`, `fun String.healthy() = 2`, `val later = 1`, or `@A(\nfun healthy() = 2` | six excluded header inputs: diagnostics nonempty, class extends past `class Box<T`, original token spelling retained; no exact complete projection claimed |

Other callers outside #100's classifier-owned type-parameter repair are contract
exclusions, not demonstrated grammar failures. Existing
`valid_classifier_generics_and_nonclass_callers_are_preserved` covers valid function,
property and typealias generic lists and a class constructor with a function-type
bound. `generic_recovery_uses_header_grammar_without_newline_dependency` covers
classifier entry/header spellings. Neither supplies an object fixture; object
behavior is unverified here. C12 supplies exact selected declarations for these
existing accepted controls. No new caller matrix is inferred. Missing simple types/open generic arguments retain the existing upstream
same-position diagnostic deduplication behavior; they are not new C17 cases.

Smallest next repairs: first C14 at parameter-default expression entry while
preserving valid nested default declarations; then C15 at parenthesized/function-type
recovery with annotated-type controls; then C16 at annotation value-argument parsing
with legitimate nested-function controls. Each needs its own accepted child and
exclusive parser/vendor/provenance ownership transfer after this fixture lane.
Generic/extension sibling improvements are separate finite contracts if admitted.
No C14–C16 repair is claimed here; the distinct C12 assertion delivery is recorded
above without changing the residual classifications.

#83/#7 remain open and T008a remains blocked on the remaining parent acceptance
and individually named recovery/resource gaps.
The Rust-only fork preserves archive/source inventory and notices under
`vendor/ktrs-parser-0.5.1`; recovery.patch isolates the source change. No wrapper
reconstructs declarations from text or patches returned CSTs. No oracle ran.

No acceptance evidence yet for enum entries, secondary constructors beyond the
specific recovery/control cases, anonymous objects beyond the tested parameter
default, destructuring declarations, delegated properties, context parameters,
when guards, contracts, or string-template expression hierarchy. These are
individual unaccepted grammar areas, not blanket rejections; existing ktrs may
return trees for them, and no compiler/version-validation claim is made. Research
records upstream facade KOTLIN_VERSION 2.4.20; exact internal dependencies are
0.5.1. Our named 2.3 corpus does not enforce a 2.3 language-version switch or reject
newer syntax. Expansion requires exact independent fixtures, not inferred support.

## Result and resource boundaries

SourceSnapshot owns immutable original UTF-8 source, opaque document identity,
revision, and file kind. SnapshotResult exposes a tree only via tree_for(current),
which compares all four fields; even same-revision changed text is stale. Callers
must assign a fresh identity or revision on reopen and synchronize publication
against their document store. This synchronous library cannot prevent a document
changing after tree_for returns. It does not provide symbol IDs or incremental
invalidation. The unversioned parse API remains for standalone parser consumers.

The parent rejects unknown kind names, wrong token/node flags, non-KT_FILE roots,
nonpreorder parents, invalid UTF-8 offsets, overlapping/gapped direct children,
nonempty childless nodes, nonpartitioning tokens, and missing/extra/misranged/empty
syntax diagnostics. Diagnostics correspond to ERROR_ELEMENT nodes in preorder.
These are structural checks, not proof of grammar correctness or a sandbox for
malicious executables. Use only the trusted matching worker binary.

| Evidence path | Proven behavior |
| --- | --- |
| Real parser worker via declarations/parser integration tests | source normalization/mapping, grammar/recovery observations, 64 KiB exact whitespace acceptance, 64 KiB+1 parent InputLimit, raw nesting preflight rejection, output element and diagnostic count rejection, token-loss Coverage, healthy followups |
| worker_protocol assert-based Rust executable | public parse_snapshot supervision/decoding/validation on 19 malformed/fault replies; wrong kinds/root/flags/extents/UTF-8/diagnostics/unknown fields/truncated JSON, exact depth-128/129 token and empty-node boundaries, >4 MiB wire output, abort/normal nonzero exit, deadline, midflight cancellation; real-worker healthy followup after every fault |
| Worker unit subprocess fixture | allocation denial aborts only child; cumulative checked charging, not an identified allocation result |
| Existing bounded generated/backtracking tests | crash containment and healthy followups; exact generic WorkerFailed versus Timeout may depend on allocation/scheduling |

The parser ceiling remains 64 KiB vs 8 MiB document/lexer ceiling: larger input
returns InputLimit with no truncation. No resource limit increased. The allocator
charges 64 MiB cumulative requests, including realloc/churn, not RSS. Allocation
abort/stack/panic still map to generic WorkerFailed; no claim that the cause is
identified. Diagnostic/element counts share OutputLimit. Real-worker adversarial
resource boundary coverage is not complete #83 acceptance. Deadline/cleanup and
trusted-worker assumptions remain in README. No JVM, Gradle, Maven or official
oracle ran for this corpus; the #87 dependency change selects the audited local parser fork without new packages.

Abort fixtures allow WorkerFailed or Timeout: the supervisor checks the deadline
before observing exit status, and abort completion can cross it. Normal nonzero
exit, hang/no-read, output, and healthy-followup assertions remain strict.

## Delivered #100 classifier-generic recovery

PR102 recovers simple named sibling function headers after incomplete
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
regressions are in declarations.rs/recovery_limits.rs. Final reviewed head `d7da79ddeea95901327d774382a37740445a0ef7`, CI checkout
`78c55c4bfb5629704bfdb11203f869af2e24781c` and merge
`ab00d55557cfea9bcee87aa41fe2922f82770027` share tree
`2d46fd856392f961af4c727b9f6ff7f3a561ae09`. Renewed independent correctness,
adversarial and architect reviews approved; the coordinator granted the guarded merge.
[Final merged-main Linux CI](https://github.com/anilbeesetti/ktlsp/actions/runs/38289885940/job/114947148613)
passed all three pinned commands: 192 active tests, two exercised helper fixtures,
19 separately counted protocol faults. #100 is closed; #83/#7 remain open and T008a
blocked. Historical candidate prose in REVIEW/vendor evidence is superseded for
**delivery status** by final PR102/audit evidence; no earlier runs are relabeled.
Issue #96 remains OPEN/UNKNOWN; Linux passes do not establish Darwin reliability.

The coordinator explicitly amended the accounting contract after a focused audit:
leading trivia and cheap kind/modifier/EOF prefilter misses may remain outside
the copied-token allowance. Every eligible scratch window is charged before
grammar evaluation, including failure; a zero-copy eligible rejection debits one.
The separate allocation-free leading scan is bounded by the 64 KiB input and
local comma-consuming loop progress. This is not a bound on every examination,
whole-parser complexity, RSS or total work. The production leading-trivia regression
`large_leading_generic_trivia_keeps_copied_prefix_accounting` passed final Linux CI. Unconditional window-predicate checks compile
against the actual byte/EOF guard. These do not establish an end-to-end active-EOF
recovery case; that gap remains for #83. The final #83 audit still classifies attributable production deadline/allocation/
serialization boundary evidence B3–B5 as GAP. C17 changes none of those limits or
claims and supplies no new resource-boundary execution.

## Finite C13 two-component nullable recovery candidate

The finite header `fun pkg.String?.healthy(` is prepared after unfinished
class/interface generics, requiring exactly two receiver components, one nullable
marker and a complete opening-parenthesis token. The focused test freezes49
selected positive projections,61 finite controls,42 raw-token/byte cuts and1416
UTF16 literals in original LF/CRLF/loneCR; forty EOF arithmetic rows are data-only.
Only nullable-decline-qualified and qualified-decline-nullable are superseded by
exact seed assertions; all historical fixture bytes/other live assertions, longer
nullable declines and resource limits remain. This is syntax-only preparation.

Exact delivered fe4f4a73 was integrated once with all incoming B3/binary work
preserved; combined provenance was rebuilt from the verified original archive.
Inherited observer callbacks include same-thread scratch EOF calls; ordinary EOF
TLS overhead remains unmeasured. PR202's three MissingACK observations leave B3
GAP. Combined SAME3 review and original Linux CI remain required. No activeEOF,
semantic/provider/full-parity/performance or aggregate gate promotion follows.
See [the source review](reviews/c13-two-component-nullable-recovery.md).

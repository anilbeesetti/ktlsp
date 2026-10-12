# Kotlin 2.3 declaration baseline v1 — #86/#87/#100, partial #83

This independently authored native corpus pins the source-local ktrs-parser 0.5.1 fork and registry syntax 0.5.1, not
compiler or official LSP parity. Source literals and exact expected extents,
declaration-relative name offsets, immediate CST parents, nearest declaration
parents, and diagnostic spans/messages live in `tests/declarations.rs`. Expectations
are hand-authored, not captured worker snapshots. Every parse uses the real
supervised Rust worker. The parser crate has no provider/HIR/server caller.
The #87 delivery gate requires exact-head production execution and behavioral
review; current evidence is recorded in REVIEW.md and PR90.

## Accepted fixtures

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
| recovery_limits integration tests | bounded nested annotation probing, strict postparse NestingLimit, repeated recovery through 100 points, balanced raw-delimiter diagnostic overflow, healthy followups |
| snapshots_reject_changed_revision_document_kind_and_reused_identity | exact immutable input/result association, stale revisions/documents/kinds/same-revision changed text, cancellation and retained original |

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
class-generic defect with exact proposed class/list, sibling kind/name/range/parent
and diagnostic expectations. Entry-boundary errors retain same-offset deduplication.
The candidate is not accepted until exact-head Linux execution and reviews pass.
`token_loss_still_rejects_coverage_and_allows_healthy_followup` separately preserves
Coverage rejection for `{fun<)]<T:@( {})` and its healthy followup.

Other individually named upstream limits remain: a missing default (`x: Int =`
followed by named `fun`), deeply malformed parenthesized type (`x: (@A`) and
malformed annotation arguments can still consume later declarations. Recovery
inside those nested grammar productions is outside the parameter-boundary repair.
Missing simple types/open generic argument types may recover with upstream
same-position diagnostic deduplication (e.g. only `Type expected`).

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

## #100 classifier-generic recovery candidate

The candidate recovers simple named sibling function headers after incomplete
class/interface type-parameter lists at parameter entries or after completed
parameters/bounds. Optional complete annotations/modifiers stay with the function.
Newlines and keywords alone do not establish a recovery boundary. Generic and
extension function headers, other sibling kinds, objects, and incomplete nested
bounds/annotations remain outside this positive contract; ordinary parsing stays
in effect for them. #83/#7 and T008a require the remaining acceptance audit.

Scratch prefix parsing preserves live state and respects existing truncation;
256 raw tokens/4096 bytes per attempt and 4096 charged raw tokens per class list
bound probing. Exceeding these declines recovery without changing global budgets,
Coverage rejection, snapshot identity, strict result validation or the 500 ms
worker deadline. No server provider or semantic API changes. Exact independent
regressions are in declarations.rs/recovery_limits.rs. Runtime acceptance is
pending the candidate's authorized Linux CI and independent behavioral rechecks;
local format/Clippy and ordered patch/hash replay have passed. This draft text is
not a completed-delivery claim. Issue #96 remains unresolved.

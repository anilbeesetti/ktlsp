# Kotlin 2.3 declaration baseline v1 — #86, partial #83

This independently authored native corpus pins ktrs-parser/syntax 0.5.1, not
compiler or official LSP parity. Source literals and exact expected extents,
declaration-relative name offsets, immediate CST parents, nearest declaration
parents, and diagnostic spans/messages live in `tests/declarations.rs`. Expectations
are hand-authored, not captured worker snapshots. Every parse uses the real
supervised Rust worker. The parser crate has no provider/HIR/server caller.

## Accepted fixtures

| Named test | Contract |
| --- | --- |
| baseline_preamble_and_declaration_kinds | package; import alias/star; class/interface (both CLASS, distinct keyword tokens); object; function; property; typealias; exact declared names and hierarchy |
| baseline_modifiers_annotations_generics_constraints_and_nested_members | annotation/modifier extents; class type parameter/bound; where constraint extent excludes keyword; member property/extension function/nested class |
| baseline_receivers_and_locals | property receiver/accessor; generic function receiver/constraint; local class/function/property owned by enclosing function through BLOCK |
| baseline_incomplete_declarations_retain_later_class | missing class/interface/object/function/property names and extension-function name; missing typealias name/type; later class retained |
| baseline_incomplete_preamble_alias_star_and_annotation_recovery | incomplete package/import alias; invalid star alias; missing annotation name; later declaration retained; exact diagnostic spans/messages |
| baseline_original_unicode_newlines_backticks_and_eof | LF/CRLF/lone CR; consecutive/terminal newlines; astral/combining Unicode; backtick name; original source and UTF-16 boundaries; malformed function EOF |
| snapshots_reject_changed_revision_document_kind_and_reused_identity | exact immutable input/result association, stale revisions/documents/kinds/same-revision changed text, cancellation and retained original |

Nodes use upstream kinds without inventing an interface kind. An omitted package
is an empty PACKAGE_DIRECTIVE, excluded from the named-declaration projection;
full tree validation still checks it. Package/import qualified names remain CST
REFERENCE_EXPRESSION/DOT_QUALIFIED_EXPRESSION children, not resolved names.
Offsets are UTF-8 source spans, never symbol identity.

## Unresolved recovery and grammar gates

`unresolved_parameter_and_generic_recovery_gate_is_recorded` permanently records
these four separate defects/outcomes. They currently return validated, lossless
CSTs **with syntax diagnostics**, not successful declaration recovery:

- Top-level unclosed function parameter list consumes later property/function.
- Member unclosed function parameter list consumes later property/function.
- Local unclosed function parameter list consumes later property/function.
- Unclosed class generic parameter list consumes the following function keyword.

These tests assert the missing later FUN, making the limitation reproducible;
they are defect evidence, not acceptance of #83 recovery. Fixing the dependency
must replace those assertions with exact retained declaration/name/parent/range
expectations and check valid multiline parameters/defaults/lambdas/generics.
`{fun<)]<T:@( {})` separately returns Coverage for upstream token loss. Original
source is preserved; a healthy parse follows both defect and coverage cases.

#83/#7 remain open and T008a remains blocked. A coordinated Rust-only upstream
patch/fork/vendor or newly audited pinned release must repair these cases. This
wrapper does not reconstruct declarations from text or patch returned CSTs.

No acceptance evidence yet for enum entries, secondary constructors, anonymous
objects, destructuring declarations, delegated properties, context parameters,
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
oracle ran for this corpus; no dependency or lock change is needed.

Abort fixtures allow WorkerFailed or Timeout: the supervisor checks the deadline
before observing exit status, and abort completion can cross it. Normal nonzero
exit, hang/no-read, output, and healthy-followup assertions remain strict.

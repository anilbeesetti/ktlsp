## Summary

The next useful source-binding consumer is **a bounded, snapshot-local binding from an authenticated whole-initializer reference to an earlier unique local `val` in the identical ordinary-function body**. It can supply a real definition target and an identity-bearing reference occurrence without inventing a type system or claiming complete references support.

**#308 admits syntax; it does not authenticate a public reference-site view or resolve names.** The remaining immediate interfaces are an authenticated `ReferenceSite`, reference-side spelling using PR307’s existing helper, and the finite binding operation below.

Read-only investigation only: no edits, children, parser/oracle execution, builds, or tests.

## 1. Kotlin rules: established evidence versus missing oracle evidence

### Primary sources

1. **Scope and declaration-order rules:** Kotlin specification, pinned commit [`f434541dd3ff888762b2d345edbafd444f4d41ee`, `scoping.md`](https://github.com/Kotlin/kotlin-spec/blob/f434541dd3ff888762b2d345edbafd444f4d41ee/docs/src/md/kotlin.core/scoping.md).
   - Function bodies are **statement scopes**.
   - Names in statement scopes bind in appearance order; code preceding a binding cannot access that binding.
   - A later declaration in a statement scope does not retroactively capture an earlier reference: the specification explicitly describes lookup finding an outer/imported property instead, or producing an error.
   - Multiple ordinary values with the same identifier in one scope are generally disallowed; rebinding a name available through linked outer scopes is allowed.
   - File/classifier declaration scopes have different forward-reference rules.

2. **Shadowing priority for property reads:** the same pinned specification, [`overload-resolution.md:313–323,586–601`](https://github.com/Kotlin/kotlin-spec/blob/f434541dd3ff888762b2d345edbafd444f4d41ee/docs/src/md/kotlin.core/overload-resolution.md#resolving-property-access).
   - Local non-extension candidates are considered by scope, smallest first.
   - Bare property reads use the property-access resolution framework; these rules are not limited to function-call syntax.
   - Consequently, **an already-visible inner local value shadows an outer value**, but structural containment alone does not establish visibility.

3. **Initializer/self-reference boundary:** Kotlin compiler tag `v2.2.20`, resolved through GitHub to commit **`693c44ee79f62895a9b92bdd60fdd7a9bc29a975`**:
   - [`FirDeclarationsResolveTransformer.kt:140–168,630–672`](https://github.com/JetBrains/kotlin/blob/693c44ee79f62895a9b92bdd60fdd7a9bc29a975/compiler/fir/resolve/src/org/jetbrains/kotlin/fir/resolve/transformers/body/resolve/FirDeclarationsResolveTransformer.kt#L630): ordinary local properties dispatch to `transformLocalVariable`; the non-delegated initializer is transformed **before** `context.storeVariable(variable, session)`.
   - [`FirExpressionsResolveTransformer.kt:725–750`](https://github.com/JetBrains/kotlin/blob/693c44ee79f62895a9b92bdd60fdd7a9bc29a975/compiler/fir/resolve/src/org/jetbrains/kotlin/fir/resolve/transformers/body/resolve/FirExpressionsResolveTransformer.kt#L725): block statements are transformed in indexed statement order.
   - [`FirLocalScope.kt:56–60,82–87`](https://github.com/JetBrains/kotlin/blob/693c44ee79f62895a9b92bdd60fdd7a9bc29a975/compiler/fir/providers/src/org/jetbrains/kotlin/fir/scopes/impl/FirLocalScope.kt#L56): registration stores the variable symbol under its name; property lookup reads registered entries.

**Conclusion:** for the ordinary non-delegated local-property path inspected, the new local is not registered while its own initializer is resolved. An initializer spelled like its declaration must not automatically bind to that declaration. An outer binding can remain relevant.

This compiler source is **not established as the compiler implementation bundled with official LSP v263.6379.0**. It supplies pinned compiler-source evidence, not release-specific navigation proof.

### Concrete consequences

| Case | Established language/compiler-source consequence | Honest finite-consumer behavior |
|---|---|---|
| `val x = 1; val y = x` | RHS can refer to preceding `x`. | Bind to that original declaration. |
| `val y = x; val x = 1` | Later local `x` is not visible at the earlier RHS; outer/import lookup may still matter. | `NoEligibleLocal`, **not** globally unresolved. |
| `val x = x`, without a previously visible `x` | Inspected compiler path does not register this declaration before its initializer. | Never return the declaration as its own target. |
| Outer `x`, then inner `val x = x` | Inspected ordering leaves outer lookup relevant during initialization. | Reject nested/outer lookup in this subset; do not guess a target. |
| Outer `x`, completed inner `val x = 2`, then inner read | Visible smaller-scope local has priority. | Nested scopes remain unsupported initially. |
| Two same-name local vals in one body | Generally invalid redeclaration; compiler error recovery/navigation is a separate question. | Reject as ambiguous within the subset; never select first/last. |

**Missing official fixture:** existing `docs/research/oracle/standalone-declarations/` records declaration-symbol and hover requests, not reference binding. Release-specific behavior still needs captured definition/reference responses for:
- the exact 33-byte same-block fixture;
- local forward reference with and without an outer property;
- self-spelled initializer with and without an outer property;
- nested shadowing before/after the inner declaration;
- duplicate local declarations, including a duplicate after the queried occurrence.

These are missing **official navigation observations**, not reasons to invent different language rules. No fresh oracle was run.

## 2. Existing interfaces and their actual limits

The #308 checkout inspected was:

`/Users/anil/.codex/worktrees/t009-bare-local-reference-initializers/ktlsp`

Its recorded setup base is `d922747e74e3a19464bcaeea8e806df8906b46ae`, the PR307/PR304 union. The source-ready receipt describes **uncommitted source V1**, not a delivered binding implementation.

| Interface / relevant file | What it actually provides |
|---|---|
| #308 `crates/ktlsp-parser/src/lowering.rs:513–605,650–760,833–856` | Strict reference-bearing ordinary-function/local-val admission. Requires the reference under `KT_FILE → FUN → BLOCK → PROPERTY`, with the captured two-element reference subtree. Output remains declarations/errors only. |
| #308 `crates/ktlsp-parser/tests/lowering.rs:128–270` | Unexecuted-in-this-investigation production assertion for the captured tree and same-origin `lower_declarations → build_regions` association. |
| PR307 `lowering.rs:142–243` | Existing declaration identity/ranges plus `Declaration::semantic_spelling()`. Its helper is private. No reference-side spelling API exists. |
| `crates/ktlsp-parser/src/lib.rs:133–205` | Immutable `SourceSnapshot`; `SnapshotResult::tree_for(current)` checks document/revision/kind/source equality. |
| `crates/ktlsp-parser/src/structural_regions.rs:123–179,213–280` | Original borrowed regions and declaration memberships; rejects a lowerer from an independently produced snapshot, even with equal content. |
| PR299 `declaration_name_index.rs:159–282` | Exact **raw-spelling** groups in an identical region; preserves duplicates and result-local declaration ordinals. No visibility or escaped-name equivalence. |
| PR302 `ancestor_candidates.rs:1–6,113–239` | Complete raw-name groups across structural ancestors. Explicitly no semantic winner, visibility, or reference-position semantics. |
| PR305 `position_regions.rs:15–52` | Half-open structural containment; validates bounds/UTF-8 boundaries. Includes brace bytes, returns all containing regions, and yields none at EOF. Not reference identification. |

The accepted PR304 capture is separately recorded in:

`/private/tmp/ktlsp-handlers/reference-binding/FIRST_CAPTURE_ACK.json`

It identifies source head `b34a370a2a660795f95b610bbc9ee1065549dd7b`, with **37 elements, 27 tokens, zero diagnostics**. The original captured shape is:

- reference expression **32**, identifier token **33**, original bytes **29..30**;
- containing property **25**, body **11**, function **3**, root **0**;
- target declaration `x`: property **14**, name bytes **14..15**, declaration bytes **10..19**.

This is **Rust production-CST evidence**, not Kotlin binding evidence.

## 3. Minimal consumer contract

### Proposed APIs and files—not existing implementations

**Extend `crates/ktlsp-parser/src/lowering.rs`:**

- `reference_site(lowered, result, current, expression_index) -> Result<ReferenceSite, ReferenceSiteError>`, with borrowed lifetimes tied to the original lowerer/result.
- `ReferenceSite` has private fields and borrowed accessors for origin, expression/token CST indices, original range/raw spelling, containing property, body CST index, and enclosing function.
- `ReferenceSite::semantic_spelling() -> Option<&str>` calls the **same existing private helper** used by `Declaration::semantic_spelling()`. No duplicate identifier validator, synthetic `Declaration`, public arbitrary-string symbol constructor, or Unicode normalization.

This closely matches the existing proposed interface recorded in:

`/private/tmp/ktlsp-first-reference-binding-contract/reference-site-next/API_DELTA.json`

**Add `crates/ktlsp-parser/src/local_value_binding.rs`:**

- `bind_local_value(directory, site, current) -> Result<LocalValueBinding, LocalBindingError>`.
- `LocalValueBinding` privately retains the authenticated site, identical original body region, and original target `&Declaration`.
- Errors distinguish **invalid origin, stale snapshot, unsupported context/spelling, ambiguous local declarations, no eligible local, and resource refusal**. They expose no target.
- One module export in `crates/ktlsp-parser/src/lib.rs`; no Cargo/dependency/server changes are inherent to this contract.

### Required invariants

1. **Authenticate the occurrence, not its text.**  
   An expression index is an untrusted selector. Require an original non-token `REFERENCE_EXPRESSION`, its sole direct `IDENTIFIER`, identical positive-width UTF-8 range, and the admitted whole-initializer ancestry. Declaration-name tokens, token 33 used as an expression selector, arbitrary equal ranges, comments, and strings cannot succeed.

2. **Preserve exact producer association.**  
   Follow `build_regions`’ identity pattern: original lowerer snapshot and result snapshot must be pointer-identical; `tree_for(current)` must succeed; internal parsed association must match. Then require `site.origin()` pointer-identical to `directory.lowered()`. Neither equal bytes nor matching document/version/CST indices substitutes for origin.

3. **Use the identical body membership.**  
   Match the authenticated body CST index to its original `Region`; candidates must have membership in that exact region and direct sibling property ancestry. `Declaration::owner()` denotes an enclosing declaration ordinal, **not a scope**. A file region, class body, sibling function, or merely containing region cannot contribute a candidate.

4. **Keep the family closed.**  
   Ordinary top-level zero-parameter functions; plain local immutable untyped vals; admitted integer or whole-identifier initializers. Refuse nested functions/blocks/classes/lambdas, receivers, parameters/defaults, imports, member accesses, calls, assignments, delegated/destructured properties, templates, scripts, and relevant incomplete/error-bearing contexts. #308’s successful lowering does not itself prove the entire file meets this narrower consumer contract.

5. **Select only a preceding declaration.**  
   The unique matching candidate must be complete, be a preceding sibling, and finish before the reference begins. Exclude the initializer’s containing declaration and later declarations. Do not chase initializer chains: in `val y = x; val z = y`, the second reference targets `y`, not `x`.

6. **Compare successful semantic spellings.**  
   Require supported spelling on both sides. Never treat two `None` values as equal. Scan the complete body for semantic-equivalent duplicates, including a later duplicate. Reject duplicates of the queried name rather than navigating through invalid code. Unsupported candidate spelling must not be silently discarded as “different.”

7. **Bound the whole query.**  
   Reuse existing checked-limit/error patterns. Charge element/declaration traversal and compared bytes under one operation budget; no recursion, unchecked indexing, per-candidate whole-function rescans, or reset budget per helper. A limit refusal must not become `NoEligibleLocal`.

### Definition/reference use

A successful binding already provides both useful facts:

- **Definition:** original target declaration/name range.
- **Reference:** authenticated original occurrence/range plus the exact target declaration occurrence.

A later references consumer may collect occurrences using this same binding kernel and group them by **target identity**, never by spelling. The first API must not label one returned occurrence—or only supported occurrences—as complete document/workspace references. Same-body completeness requires complete admitted-body coverage and atomic refusal on unsupported/ambiguous cases.

PR299/302 are not mandatory dependencies here: their raw-name grouping does not solve semantic spelling, and ancestor traversal would cross the deliberate same-body boundary. PR305 can assist position selection, but cannot authenticate the reference.

## 4. Wrong ranges and stale incarnations

- For the captured case, the reference is **29..30**, the definition selection **14..15**, and the target declaration **10..19**. Returning the containing `y` declaration or its name **25..26** is wrong.
- Convert LSP positions through `ktlsp_text` against the **identical snapshot source**. Use strict half-open token containment; do not snap an end position, brace, whitespace, surrogate interior, or EOF to a nearby identifier.
- Before publication, recheck the authoritative open document under its synchronization.
- **Close/reopen is an identity boundary.** `tree_for` uses value equality; if URI, version and bytes are reused after reopening, it cannot detect the new incarnation by itself. Its existing contract explicitly requires a new document identity or revision on reopening. The caller must enforce that requirement or invalidate all old results; pointer association among old analysis objects does not make them current.

## 5. Identity and remaining scope

**Result-local ordinal ≠ stable cross-revision `SymbolID`.**

Declaration array ordinals, `owner()`, membership region ordinals, and CST indices address one retained result. Original borrowed declaration identity is sufficient for this finite query. It cannot be persisted, reconstructed from a range/name, or reused after reparsing. Stable cross-revision identity still requires a defined reconciliation/invalidation contract; merely combining URI, revision and ordinal creates a revision-scoped occurrence key, not stability across revisions.

**Full T009 remains open:** nested visibility/shadowing; parameters and defaults; lambdas/captures/implicit `it`; labels; explicit and implicit receivers; namespace and overload interactions; duplicate/error recovery; full identifier spelling; declaration-scope behavior; and stable resolved source identity.

Likewise, this does not complete [T026](https://github.com/anilbeesetti/ktlsp/issues/26)’s library/type/implementation/override navigation or [T027](https://github.com/anilbeesetti/ktlsp/issues/27)’s cross-file/module references, aliases, read/write highlights, and mutation behavior. No type algebra, name-only navigation, provider advertisement, or official-parity promotion is justified by this subset.
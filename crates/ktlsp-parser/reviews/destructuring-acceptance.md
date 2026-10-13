# One ordinary local two-name destructuring acceptance fixture — held source

Child [#163](https://github.com/anilbeesetti/ktlsp/issues/163), parents #83 / #7.
Frozen preparation base `fe97781cfde7c9e230470463e3e1212925f0c1b8`, tree
`13672a92666a6744718d6b95908ba7890179f393`, sole parent
`ac93bb5f777ca1cc6e6eeb3bc3e8467b199d56b1`. Root admitted only this new note
and `tests/acceptance_destructuring.rs` after actual independent correctness,
adversarial and architect contract READY readbacks. The authoritative contract
SHA256 is `e99e198689aa448d061dbd926837fd0aa313357b3a5c14a5cf2edefcd64b93aa`;
expectations JSON SHA256 is
`8b2f3a95f4e5026bb91f7e87e706c803d07719d39d3612088e03b73ca7f86a12`.
The original contract, final receipts and initial outcomes are preserved outside
the source commit, including an independent copy at
`/private/tmp/ktlsp-destructuring-evidence-01a135b2/contract`.
Issue repository, number, title and complete published body were read back;
only obsolete admission/publication wording changed from the reviewed proposal.

## Intended acceptance, pending runtime execution

One exact 77-byte / 75-UTF16-unit LF source, no terminal newline:

```kotlin
fun split(value: Pair<Int, Int>) { /*😀*/
    val (first, second) = value
}
```

Source SHA256:
`5a0e5879f739527a49322d85031e3388fafffbffb8bb95fe2bc4278beffa5bfc`.
The ordinary Pair parameter supplies a valid initializer, but no component/type
resolution is asserted. This is one syntax acceptance representative, not an
established grammar defect or full destructuring-family coverage.

The frozen source-derived contract selects all21 nonempty non-token CST nodes,
including root, parameter/type nodes, expanded BLOCK, destructuring declaration,
both entries and initializer reference. Empty package/import nodes stay under
the unchanged production validator. Exact sequence/range equality and uniqueness
checks reject missing/extra selected nodes; expected parent/owner indices map to
actual CST IDs. No textual name is treated as a resolved symbol identity.

The destructuring declaration is48..75, directly under BLOCK33..77, whose parent
is FUN0..77. Its entries first53..58 and second60..66 are separate direct children;
initializer REFERENCE_EXPRESSION70..75 is another direct child. The eight exact
IDENTIFIER tokens include split4..9, parameter value10..15, Pair17..21, Int22..25,
Int27..30, first53..58, second60..66 and initializer value70..75. Nearest strict
ancestor declarations include FUN, ordinary declaration kinds and both
destructuring kinds; VALUE_PARAMETER is not in that predicate.

All34 token kinds/spellings/ranges/actual parent IDs and complete positive
contiguous original-source partition are asserted. All75 original scalar
boundaries have independent literal UTF16 expectations, including emoji37..41
mapping(0,37)→(0,39), comment35..43 ending(0,41), and EOF77 mapping(2,1).
Offsets38/39/40 inside emoji UTF8 and78 past EOF return exact InvalidOffset.
Require zero diagnostics and no ERROR_ELEMENT; Coverage, Timeout or another
unsupported outcome cannot stand in for successful acceptance.

The production path is public parse_snapshot/tree_for → supervised matching
worker → parse_file → parse_function/body → lazy BLOCK collapse → reparse_lazy
→ parse_block_expression(collapse=false) → parse_statements/local declaration
→ parse_common_declaration(Local) → parse_property. Local short-form val permits
parenthesized destructuring; accessors.rs closes each ordinary name as a separate
DESTRUCTURING_DECLARATION_ENTRY. Comma and parentheses stay in the declaration;
EQ introduces a normal reference initializer. Ordinary comment trivia remains
under BLOCK; no CST is synthesized, reconstructed or patched by the test.

One ordinary test makes exactly two production-worker requests on the same
Parser object. Snapshot equality/accessors cover document, revision, kind and
source. Clone and independently allocated value-equal snapshots are accepted.
Four worker-free queries independently reject changed revision, document, kind
and same-revision equal-length first→third source as StaleSnapshot. The original
result is fully rechecked before and after strict revision2 `fun healthy() = 2`
followup. That control pins all four nonempty CST nodes, healthy4..11 direct
parent, all18 UTF16 positions, lossless tokens and zero diagnostics. Old/new
results reject each other's snapshots. Each call owns a fresh child; no
persistent-worker recovery claim follows.

## Execution and delivery boundaries

Only two new files are admitted. Vendor/grammar, adapter/worker/API, existing
tests/helpers, exports, Cargo/dependencies/lock, root documentation/status and
workflows remain unchanged. Cargo autodiscovery adds one integration executable
with one ordinary test/two sequential worker requests; no ignored helper,
protocol-fault case, oracle or benchmark target is added. Global and scratch
limits remain unchanged, including64KiB source,500ms deadline and64MiB cumulative
requested allocation (not RSS).

Local formatting and compile-only Clippy are permitted. Exact commands/outcomes,
source hashes, held commit/tree, path inventory and actual SAME correctness,
adversarial and architect source readbacks belong in the external source packet;
compilation does not establish runtime acceptance. No local parser/worker/server/
test/bench/JVM/compiler/oracle/Gradle/Maven execution is admitted under #96.
Push, PR, hosted CI, integration and merge remain HOLD until root assigns ONE
final integration base after predecessor delivery. The source packet preserves
the temporary selected-model capacity interruption as a service event, not a
native failure.

Future runtime verification uses only the original unchanged hosted Linux Native
Rust workflow with all3 pinned commands. Preserve its first legitimate failure;
no expectation weakening, unchanged retry, timeout relaxation or grammar repair
is admitted. A mismatch requires a separately admitted defect scope or an
independently demonstrated contract mistake. Source/combined-head/completed-CI
reviews, expected-head merge and exact-main delivery verification remain gates.

This finite fixture cannot close #163 before reviewed delivery, complete
T007/#7/#83, or admit T008/T008a. Broader C13/B3/B4 remain GAP, B11 UNVERIFIED,
#96 OPEN/UNKNOWN. No semantic/full official parity, Darwin correction, total-work,
RSS or performance claim follows.

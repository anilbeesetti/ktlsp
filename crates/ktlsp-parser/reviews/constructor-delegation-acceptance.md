# One constructor-delegation acceptance fixture — source preparation

Parents #7 / #83; child issue and delivery references are recorded in the PR. This is a finite syntax
acceptance gap, not an established parser bug. Source admission covers exactly
this note and new `tests/constructor_delegation_acceptance.rs`.

Frozen contract base: `048c344c5a75f7cf16cadc627c7ff765c025c7f7`, tree
`2e168920834e285d89ac5faa7c90fdce43047de7`, sole parent
`32e792f71e1397f48d5b56cb22f72c3a58321727`. Root assigned one source integration
base `bb64eb5b5200ef801ffb224336bf2eb997b68054`, tree
`0ebc83052425eefe3747d3f99a3e02e5c6a2190a`, sole parent the contract base. All102
source pins remain unchanged there; both future files were absent before edits.
Incoming binary data tests change workspace counts, not this parser contract.

The actual same architect, correctness and adversarial reviewers approved V2
contract preparation. Contract manifest SHA256:
`be7addd6332578881c8bc3e26c9a027639322e57583ac1402b5cf03654c9ccb8`;
expectations SHA256:
`d1968dbb2611198b7fa39b6067353c5948f9a35d71544e7ba61b814401963e51`.
The initial workflow-inventory omission and corrected V2 readbacks are preserved
outside the repository. Fixture/behavior expectations were unchanged by that fix.

## Independently frozen syntax assertions

The literal is54 ASCII bytes with no terminal LF:

```kotlin
class Box() { constructor(value: String) : this() {} }
```

Source SHA256:
`1a7096c41720b7aa7b679683cd5c855be2ab2051685c5028568a643b95d4c046`.
Existing declarations.rs717's constructor control selects only CLASS through its
shared declaration predicate. Its761 malformed-member fixture pins a nondelegating
secondary constructor extent/ownership. Neither asserts explicit delegation CALL
and REFERENCE nodes; this fixture adds that distinct finite assertion coverage.

All15 nonempty nodes are pinned in preorder, including KT_FILE and CLASS0..54,
PRIMARY_CONSTRUCTOR9..11 with its VALUE_PARAMETER_LIST, CLASS_BODY12..54,
SECONDARY_CONSTRUCTOR14..52, parameter list25..40, VALUE_PARAMETER26..39,
TYPE_REFERENCE/USER_TYPE/REFERENCE_EXPRESSION33..39, delegation CALL43..49,
delegation REFERENCE43..47, argument list47..49 and BLOCK50..52. Exact sequence
and uniqueness checks reject extra/missing nodes. Expected parent row indices map
to actual element IDs. Nearest strict declaration ancestors include primary and
secondary constructors; VALUE_PARAMETER is excluded from that owner predicate.
Empty preamble nodes remain under the unchanged production validator.

All26 token kinds/spellings/ranges/actual parent IDs and the complete positive
contiguous original UTF8 partition are checked. The three identifier tokens are
Box6..9 directly under CLASS, value26..31 under VALUE_PARAMETER, and String33..39
under REFERENCE_EXPRESSION. Constructors have no direct identifier names here.
All55 original ASCII boundaries map to UTF16(0,offset);55 returns InvalidOffset.
Require zero ordered diagnostics and no ERROR_ELEMENT. These coordinates cover
only this single-line ASCII literal, without a general Unicode/newline claim.

The unchanged grammar path is parse_file → parse_class → primary parameter list
and ordinary class body → parse_secondary_constructor → parameter/type parsing
→ parse_this_or_super plus empty value arguments → block. The delegation call
begins at this43; colon41..42 belongs to the secondary constructor. Empty modifier
markers drop; existing builder/comment binders keep boundary whitespace outside
nested constructor/type/delegation/block nodes. The ordinary lazy BLOCK expands
through reparse_lazy/parse_block_expression; both braces are direct BLOCK children.
No test code synthesizes or repairs CST nodes or authoritative source.

One ordinary test uses the public production parse_snapshot/tree_for chain.
Clone and independently allocated value-equal snapshots expose the checked tree.
Four worker-free queries independently change revision, document, kind and the
same-revision equal-length Box→Bag source; each requires StaleSnapshot. Snapshot
equality/accessors and the complete retained original tree are rechecked.

The second and final request uses revision2 and the exact17-byte no-LF
`fun healthy() = 2` control (SHA256
`baaa02cbc60378bf27e61afe800aee16f7a70e3639dfcfdab1a9ade8eb0cb2b9`). It pins
all four nonempty nodes, healthy4..11 direct name, all9 tokens/parents, complete
partition, all18 ASCII positions, InvalidOffset(18), zero diagnostics and exact
snapshot fields. Old/new results reject each other's snapshots. Both complete
trees/snapshots are rechecked afterward. Each parse spawns a fresh trusted worker;
this is not process-reuse or recovery-after-failure evidence.

## Validation and delivery boundaries

Only two new files change. No production/vendor/grammar/adapter/worker/API,
old test/helper, export/config/Cargo/dependency/lock/workflow/root-document change.
Cargo autodiscovery adds one ordinary integration executable with one test/two
sequential worker requests; other workspace tests may run concurrently. No ignored
helper, protocol-fault, oracle or benchmark target is added. Existing source64KiB,
deadline500ms, cumulative requested allocation64MiB (not RSS), wire4MiB,
elements16384, diagnostics256 and nesting128 limits remain unchanged, as do the
scratch/classifier recovery budgets.

Root permits local formatting and compile-only locked workspace all-target Clippy;
these do not establish runtime acceptance. Local native/parser/worker/server/test/
bench/JVM/oracle/Gradle/Maven execution remains held under #96. Exact commands,
source hashes/head/tree, same-three source reviews and completed original Linux
CI/config/checkout/count reviews belong in the external source/delivery receipts.
The original unchanged hosted Linux Native workflow runs all three pinned commands:
format check, locked workspace all-target Clippy with warnings denied, and locked
workspace tests. Preserve the first contradiction and STOP for independent
source-only diagnosis/root correction grant; no output-derived expectation tuning,
unchanged rerun or main chase. Merge requires a separate sole expected-head/base
lease, a fresh separate read-only guard and ordinary expected-head squash merge.

No constructor semantics, overload/this-target resolution, primary parameter
property synthesis, definite assignment, type binding, HIR or server provider is
implemented. This fixture cannot complete C17/#7/#83, admit T008/T008a or create
a new blanket prerequisite. T008/T008a HOLD, #7/#83/#96 OPEN, broader C13/B3/B4 GAP,
full B11 UNVERIFIED and Darwin UNKNOWN remain. Finite acceptance proves neither
semantic/full official parity nor comparative speed/RAM.

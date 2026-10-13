# One bare enum-entry acceptance fixture — held source

Child [#144](https://github.com/anilbeesetti/ktlsp/issues/144), parents #83 / #7.
Frozen base: `12283dc98853dd70b82cf436bc040542db57c663`, tree
`39e3679f797e10cd59be5ddd9f6f64179a48c66e`. Root admitted only this note and new
`tests/acceptance_enum_entries.rs` after the same independent correctness,
adversarial and architect contract approvals. The frozen contract manifest is
`f1140bf52d58901bff8d21651082a09a3f624ec2df4d26998e0554b6f646057e`;
expectations JSON SHA256 is
`5614cc460d7ba6a2708284a1deb41ee5dd8dfdf98919bc060bcfb5341f6eaec5`.
The original reviewed HOLD and revised contract packets remain unchanged outside
the repository. The issue's returned repository, number, title and published body
were read back and verified; only the reviewed proposal status line changed.

## Intended acceptance, pending execution

The one 39-byte ASCII LF source has no terminal newline:

```kotlin
enum class Mode {
    READY,
    DONE
}
```

Source SHA256:
`62f9e65691ccad7fd506a967e4d512160ec853820d3146ccc0c31fa71d9e49d9`.
This is an acceptance fixture, not an established parser defect. The finite
contract search found no enum-class literal, `ENUM_ENTRY` or `ENUM_KEYWORD`
assertion in the ten frozen parser integration-test files and one vendored
recovery test. The existing declaration helper omits enum entries. No other C17
family was investigated and no old fixture is amended.

Expectations were independently frozen before implementation or runtime:

| Selected kind | Original UTF8 extent | Immediate parent | Nearest declaration |
| --- | --- | --- | --- |
| CLASS | 0..39 | KT_FILE | none |
| MODIFIER_LIST | 0..4 | CLASS | 0..39 |
| CLASS_BODY | 16..39 | CLASS | 0..39 |
| ENUM_ENTRY | 22..28 | CLASS_BODY | 0..39 |
| ENUM_ENTRY | 33..37 | CLASS_BODY | 0..39 |

The three direct identifiers are `Mode`11..15, `READY`22..27 and `DONE`33..37.
The test checks actual unique parent node IDs, all fourteen token kinds/ranges/
parent extents, complete contiguous original source partition, all forty UTF16
positions and zero diagnostics/ERROR_ELEMENTs. The selected predicate includes
enum entries and ordinary declarations plus relevant class/modifier/constructor/
parameter/type nodes; exact sequence equality rejects extra selected nodes.
Empty package/import nodes remain under the production validator. This is not a
full-AST or semantic contract.

The source path is `parse_file` -> top-level modifier detection ->
`parse_class(enum_detected=true)` -> `parse_enum_class_body` -> `parse_enum_entries`.
The first entry consumes its comma before closing, so its range includes27..28.
The second entry has no delimiter and stops at the closing brace; that suppresses
the missing-semicolon branch. Empty modifier markers drop. Existing preceding/
trailing comment binders keep pure inter-entry whitespace under CLASS_BODY.
No constructor/type/generic-recovery path or active truncated layer is entered.

One ordinary test calls the existing public production `parse_snapshot`/
`tree_for` chain. Four worker-free queries reject a changed revision, document,
kind and same-revision source as `StaleSnapshot`, then recheck the original tree.
The second and final parse uses the existing 17-byte `fun healthy() = 2` control,
requiring exact FUN/value-parameter-list/name/parent/ranges, all eighteen UTF16
positions, lossless original source and zero diagnostics. The retained enum
snapshot/result is fully rechecked afterward. Each parse starts a fresh trusted
worker; this does not claim child-process reuse.

## Execution and delivery boundary

Only two new files are owned. No production grammar/vendor/adapter/worker/API,
resource limit, Cargo/lock/workflow/root-document or old-fixture bytes change.
Cargo autodiscovery adds one integration executable containing one ordinary test
and two sequential trusted-worker requests; the unchanged workspace harness can
run other tests concurrently. No ignored/helper/fault-case/oracle/bench target is
added. Source64KiB, timeout500ms, cumulative requested allocation64MiB (not RSS),
wire4MiB, elements16384, diagnostics256 and nesting128 remain unchanged. Existing
scratch256-token/4096-byte and classifier-list4096 copied-token limits remain.
This tiny fixture does not establish whole-parser complexity, resource boundaries
or active-EOF runtime evidence.

Root permits local formatting and compile-only Clippy. Their exact commands,
exit statuses, final source hashes/head/tree and actual same-three source reviews
are retained in the external source-preparation packet; compilation is not
runtime acceptance. No local tests/native/parser/worker/server/bench/JVM/oracle/
Gradle/Maven execution is admitted. Push, PR, existing Linux CI, integration and
merge remain on HOLD for root's one final base assignment after method142.

Future controlled Linux execution must preserve the first failure, without
expectation tuning or unchanged CI retries. Any exposed grammar mismatch needs a
concrete at-most-two-grammar-file proposal, renewed reviews and root admission
before repair. Actual source/combined-head/completed-CI approvals and guarded
expected-head merge remain separate gates.

This fixture alone cannot close enum-family coverage, broader C17, #83/#7 or
T007, admit T008/T008a, or establish semantic/full official parity. #83/#7 remain
OPEN, T008/T008a HOLD, B3/B4 GAP, B11 UNVERIFIED and #96 OPEN/UNKNOWN. The historical
45/32/10/3 audit tally is unchanged. No Darwin correction or current official
performance claim follows.

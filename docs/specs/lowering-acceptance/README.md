# T008 lowering acceptance packet (preparation only)

T008/T008a remain **HOLD**. These two independently authored source fixtures and
literal expectations prepare a first small lowering review after parser adoption
clears. They neither implement nor execute lowering, and are not evidence that
these exact files pass the parser, official oracle or a semantic provider.

Reference source is repository commit
`941c72efd9896b0847ab342ec0dd22c10a000f3b`, tree
`c9a8c8ae5ee4984b6b21849007bafbeebea33975`. The separate assessed product cutoff
remains `50bef1c6d907c673839dd06666023314cad5c1fa`; this packet does not advance it.
Existing inputs are [T008](https://github.com/anilbeesetti/ktlsp/issues/8),
[architecture](../architecture.md), [verification](../verification.md),
[parser contract](../../../crates/ktlsp-parser/README.md) and
[declaration corpus](../../../crates/ktlsp-parser/CORPUS.md). There is no delivered
HIR API; JSON keys here describe acceptance facts, not Rust types or a public API.

## Finite expected projection

`declarations.kt` expects exactly seven CLASS/FUN/PROPERTY declarations in preorder:
a class with member property/function, three declarations local to that function,
and a top-level function. Repeated `same` spellings must remain separate records.
The backtick name includes an astral character and a decomposed combining accent;
retain its exact original token spelling without Unicode normalization or
backtick removal in this raw-source projection.

`error-node.kt` expects exactly three selected declarations and one syntax error.
Keep named incomplete `broken` with its original range and error; `later` and
`healthy` remain complete top-level siblings. An ERROR_ELEMENT is retained as
syntax-error data, never promoted to a named declaration. Zero-width errors are
valid. Preserve error order, messages, ranges and multiplicity from the validated
CST; do not deduplicate by range or fabricate compiler diagnostics. This fixture
has only one error and does not itself test duplicate-range multiplicity.
Error association uses CST ancestry, not range overlap: the zero-width error at
the end of `broken` belongs to that declaration. An error inside a declaration
must not discard all its descendants. `incomplete` is a proposed observation for
these fixtures, not a general error propagation or semantic-validity policy.

Every range is a half-open pair of **original UTF-8 byte offsets**, including
name ranges; these are not LSP UTF-16 columns. Indentation, trailing newlines and
separating semicolons are not declaration extents. Immediate `cst_parent_kind`
and nearest selected declaration `owner` are different facts: CLASS_BODY/BLOCK
are structural parents. Owners and labels refer only to this fixture's selected
projection. They neither resolve scopes nor define cross-revision symbol IDs.
Require the complete ordered projection and exact diagnostics, not a subset or
name-based lookup. Structural nodes and token trivia still remain in the CST.

## Snapshot and bounded input contract

Each JSON pins exact source bytes by SHA-256/length and also pins document,
revision and FileKind::Source. A hash authenticates neither origin nor document
identity. A future consumer obtains the checked tree via
`SnapshotResult::tree_for` using the exact authoritative `SourceSnapshot`;
changed document, revision, file kind, or same-revision changed source must fail
stale association before lowering/publication. Reopening requires a new identity
or revision. The caller must still synchronize publication with its current
document store; tree_for cannot prevent a later edit.

Only accepted validated CST is eligible input. Any parser rejection remains a
named rejection; do not lower a partial tree, retry until success, truncate source
or convert Coverage/InvalidTree/resource/cancellation failures into empty success.
Parent/subtree_end values are preorder **element indices**, not byte offsets or
stable symbol IDs. Traverse only a node's validated subtree; declaration names
come from their direct IDENTIFIER token, never the first nested identifier.

Reuse the unchanged parser input/validation limits: 64 KiB source, 16,384 elements,
128 CST depth, 256 diagnostics with 4 KiB messages, 4 MiB wire response, 500 ms
exchange deadline and 64 MiB cumulative worker allocation requests. These do not
bound future lowering allocation/work, hard cleanup time, RSS or stack. A future
implementation must obtain its own finite lowering budget/API contract before
source admission; this fixture packet sets no production budget or new API.

## Validation and exclusions

Before publication, review original source slices and independently verify the
literal hash/length, UTF-8 boundaries, declaration/name ranges, exact counts,
ordered nesting and the zero-width error. Data checks validate the packet, not
parser/lowering behavior. Existing workspace CI does not load these JSON files.
After source admission the unchanged pinned Linux Native Rust CI commands are
required; no local native runtime is authorized under unresolved #96.

Package/import/typealias, unnamed/anonymous declarations, parameters, expressions,
types, cross-file/cross-revision identities, resolution, document symbols and
other providers are outside this packet. Broader C13, B3/B4 and active-EOF B11
retain GAP/GAP/UNVERIFIED respectively; #7/#83/#96 remain open and Darwin remains
UNKNOWN. No JVM, oracle, Gradle/Maven, dependency, worker, server, grammar or
capability change is part of this packet. See [review note](REVIEW.md).

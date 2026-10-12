# T018a1 / issue #97 contract

Child of #68 and #18; neither parent is complete. No protobuf, names, declarations,
types, compiler compatibility, product provider or LSP parity is delivered here.
Consumes the frozen #79 typed declaration annotations; no common reader redesign,
external dependency or Cargo/lock change.

Sources: Kotlin v2.3.20 commit `d57eb4a26ed01ab03cf195da686c8047032ff16f`:
[Metadata.kt](https://github.com/JetBrains/kotlin/blob/d57eb4a26ed01ab03cf195da686c8047032ff16f/libraries/stdlib/jvm/runtime/kotlin/Metadata.kt),
[BitEncoding.java](https://github.com/JetBrains/kotlin/blob/d57eb4a26ed01ab03cf195da686c8047032ff16f/core/metadata.jvm/src/org/jetbrains/kotlin/metadata/jvm/deserialization/BitEncoding.java),
[utfEncoding.kt](https://github.com/JetBrains/kotlin/blob/d57eb4a26ed01ab03cf195da686c8047032ff16f/core/metadata.jvm/src/org/jetbrains/kotlin/metadata/jvm/deserialization/utfEncoding.kt).
Exact tag and contents verified via GitHub API before implementation.

`metadata::decode(&DeclarationAnnotations, Limits) -> Result<Outcome, Error>`:

- `Absent`: no class-level `Lkotlin/Metadata;`.
- `Unsupported { header, reason }`: unknown field, version other than exact
  `[2,3,0]`, or kind other than 1/2/5, in that precedence. No d1 conversion.
- `Decoded { header, bytes }`: supported byte payload, which can be empty.
  This says nothing about protobuf validity or semantic content.
- `Error::Malformed`: duplicate annotation/fields, wrong known field type or
  misplaced field/method metadata. `Error::Limit` names the exhausted budget.

Header retains a reference to the complete raw annotation and its visibility,
presence bitmap (k/mv/bv/d1/d2/xs/pn/xi order), signed i32 k/xi and mv/bv arrays,
borrowed UTF16 d1/d2 strings and xs/pn. Defaults are k=1, mv=[], bv=[1,0,3],
d1/d2=[], xs/pn="", xi=0. No Unicode normalization or surrogate replacement.
Negative integer bits are preserved; negative versions/kinds are unsupported.
Unknown field values remain borrowed/opaque, not recursively traversed. Both
visibility containers are accepted as data even though source retention is
RUNTIME. Duplicate metadata across containers is malformed. @Target(CLASS)
grounds rejection of member metadata. Nested annotations and unrelated values
are never searched. Public forged annotation objects are checked at consumed
positions; this API does not revalidate all annotation/classfile structure.

Kinds 1 (class), 2 (file), 5 (multifile part) use byte payloads. Kind 3 synthetic
forms and kind 4 facade name lists are deliberately unsupported. xi/bv/xs/pn are
preserved without compatibility, multifile linkage or name interpretation.

Only `d1[0][0]` selects encoding. NUL removes that one codeunit and converts all
remaining codeunits to low8 bytes. U+FFFF removes that one codeunit before legacy
decoding; any other first unit, empty first string or no strings uses unmarked
legacy decoding. Legacy converts each unit to low8, subtracts one modulo128,
combines least-significant seven-bit groups and emits floor(7*n/8) bytes.
Trailing incomplete bits are discarded even when nonzero, matching upstream;
encoded truncation is not detectable here. The next wire child must validate
protobuf truncation. Empty chunks do not change marker detection or bit packing.

Hard maxima: 16 Mi source codeunits, 16 Mi decoded bytes, 65,535 fields/records,
64 Mi work units. Caller Limits can only tighten these. One aggregate budget
counts class/member containers and visited annotations, selected header pairs
and array entries; every visited descriptor/header name and consumed string
(including repeated borrowed references) charges codeunits/work. Unrelated
annotation values/unknown field values stay opaque. Duplicate comparisons
charge their maximum possible examined prefix. Decoding charges chunk/unit and
output work before allocation; output length uses checked arithmetic. Header
arrays allocate only after entry count checks. Streaming d1 uses bounded shifts
and no flattened byte buffer/recursion. Default bv has a fixed three-int cost.

Queued #68a2 input is the decoded bytes plus preserved d2, mv/k/header and explicit
caller budgets. Its protobuf/name-record budgets must apply across its own whole
operation, and it must not interpret `Unsupported` as an empty symbol result.

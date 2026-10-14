# Selected TypeAlias data contract (preimplementation freeze)

Base cff36d17cd5cd1386059ecc91572013fdb417b48, tree 4e432a34ecd8c0eb27f563ec6f0dc483202e2472; owned managed worktree /Users/anil/.codex/worktrees/t018-type-alias/ktlsp. Preparation only until root admission. Parents #68/#18 stay open.

## Scope/API

New metadata::type_alias::decode(bytes: &[u8], limits: Limits) -> Result<TypeAlias<'_>, Error> reads ONE unframed, caller-selected TypeAlias body from independently checked mv=[2,3,0] k1/2/5 data (Class field11 or Package field5). No caller/parent traversal/version enforcement added. No header/d1/wire::decode/name resolver calls. API cannot distinguish unrelated messages.

TypeAlias<'a> { pub raw: &'a [u8], pub flags: Option<i32>, pub name: i32, pub underlying_type_id: Option<i32>, pub expanded_type_id: Option<i32>, pub underlying_types: Vec<&'a [u8]>, pub expanded_types: Vec<&'a [u8]> }. Derive Debug/PartialEq/Eq. raw borrows the entire input unchanged and pointer-identically; vectors borrow each selected inline body in encounter order within its field. Cross-field ordering and wrong-wire/unknown occurrences remain faithfully available in raw. Empty inline occurrences are retained. No clone/owned bodies. Public caller-constructed outputs have no validity guarantee.

Limits { pub bytes: usize, pub records: usize, pub work: usize, pub owned_bytes: usize }; defaults MAX_BYTES/MAX_RECORDS/MAX_VALIDATION_UNITS/MAX_BYTES (16MiB/65535/64Mi/16MiB); callers can only tighten, usize::MAX clamps. One private aggregate Budget with existing wire::Budget plus an owned counter for BOTH vectors, never reset mid-operation. No shared helper/API/dependency changes.

ErrorKind { Wire(wire::ErrorKind), MissingName }; Error {offset: usize, kind: ErrorKind}, Copy/Debug/PartialEq/Eq, Display/Error/From<wire::Error> like function_header. All offsets relative to supplied body.

## Source pin and selected behavior

Kotlin v2.3.20 commit d57eb4a26ed01ab03cf195da686c8047032ff16f, existing sources copied into sources/ as data only. metadata.proto blob18cfd13e9e512c491f2f62ec241f49c81d6e5e81 SHA256 a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2 lines450..474. ProtoBuf.java blobfc424d5e7aac8661deb862c8e791d4fcf779c194 SHA256 da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716 TypeAlias parser lines25229..25282.

Recognize only (1,wire0) flags, (2,wire0) required name, (5,wire0) underlying_type_id, (7,wire0) expanded_type_id: raw low32 interpreted signed i32, no zigzag; last RECOGNIZED scalar wins. Absent flags None has schema default6, distinct from explicit6; absent optional IDs None have schema default0, distinct from Some0. No modifiers/index validity/lookup. Wrong wire never sets/clears or overwrites selected values.

Recognize (4,wire2) underlying_types and (6,wire2) expanded_types as ALL borrowed raw occurrences. The pinned generated parser merges singular Type messages; this selected decoder deliberately retains occurrences to support a future correctly merged interpretation. It MUST NOT last-win/drop occurrences or claim merged Type values. Both IDs and inline bodies may coexist without precedence/validation. Inline contents, typeparameters3, annotations8, requirements31, plugin data32/extensions and other fields are opaque, not recursively decoded or required-checked. Empty/malformed nested bodies can succeed.

Existing wire policy unchanged: nonminimal varints terminating within10 bytes; tag/length low32, scalar low64 slow-path bits; no canonical/general protobuf or JVM fast-path compatibility claim. Field0 first; groups3/4 UnsupportedGroup; wire6/7 invalid; negative low32 lengths rejected. No group semantics.

## Accounting and errors (frozen before implementation)

Reject input length > min(bytes,MAX_BYTES) as Wire(Limit("bytes")) offset0 BEFORE work/record checks. No envelope/root/node/depth charge. Empty all-zero limits => MissingName at0. While pos<len call wire::field with ONE budget, then budget.wire.work(1,tag_start) BEFORE dispatch, including unknown/wrong-wire. Wire framing consumes one record, one work for record, one per actual tag/value/length/body byte; failed short takes remaining bytes+1 debit at attempted take offset, then Truncated at EOF. Ten continuation bytes MalformedVarint at tenth byte. No early success: wire/limit failures precede MissingName, even after a selected name. Clean EOF alone checks required name and returns MissingName at bytes.len().

For each selected inline body, reuse the geometric logical reservation policy of string_table::Budget::push privately (no shared edits). Let L=vector.len(): if L==0 or L.is_power_of_two(), reserve slots=max(L,1). Check L+slots, slots*S and aggregate owned subtraction, S=size_of::<&[u8]>() (16 on admitted 64bit, 8 on 32bit). Fail Wire(Limit("owned_bytes")) at field tag start BEFORE work debit/try_reserve_exact. Charge slots*S work at tag start BEFORE try_reserve_exact(slots); reserve failure Wire(Limit("allocation")) same offset. Then push. No additional record/node charge for a retained occurrence (its field already charged). Logical requested capacity, including both vectors, excludes allocator surplus/reallocation churn/RSS; never bytes.len()-derived preallocation. Maximum combined logical slots <= 2*total selected occurrences <= 131070; owned ceiling16MiB. No allocation for scalars/opaque unselected fields. This private ~30line helper is needed; string_table private helper is not accessible without a shared change.

All complete fixture work W=byte length +2*fieldcount + S*logical combined slots, including every inline body byte exactly once; records=fieldcount; owned=S*logical combined slots. This formula is schema/source/arithmetic, not measured decoder output. Exact and +1 budgets succeed; byte len-1 fails bytes0; records F-1 fails at last field tag; work W-1 fails at final dispatch or allocation debit, frozen by fixture below; owned total-1 fails at final growth tag.

## Literal fixture freeze

All authored hex, no compiler/JVM/oracle execution or extracted official alias claim. Pointers for inline bodies must match supplied slices.

| file | hex | flags/name/underID/expandedID | underlying bodies; expanded bodies | F / W / owned / final tag |
|---|---|---|---|---|
| minimal.hex | 10 00 | None/0/None/None | []; [] | 1 /4 /0 /0 |
| explicit.hex | 08 06 10 00 28 00 38 00 | Some6/0/Some0/Some0 | []; [] | 4 /16 /0 /6 |
| duplicates.hex | 08 01 10 01 28 02 38 03 08 06 10 00 28 00 38 00 | Some6/0/Some0/Some0 | []; [] | 8 /32 /0 /14 |
| inline.hex | 10 00 22 02 08 01 32 01 80 22 00 22 01 ff 32 00 | None/0/None/None | [08 01, empty, ff]; [80, empty] | 6 /28+6S /6S /14 |
| opaque.hex | 10 00 1a 01 80 42 00 fa 01 01 80 82 02 01 ff 20 01 30 01 2a 00 3a 00 | None/0/None/None | []; [] | 9 /41 /0 /21 |

inline.hex growth tags underlying2/9/11, expanded6/14, slot increments1/1/2 and1/1 respectively. Bodies positions4..6,11..11,13..14; expanded8..9,16..16. Len16 F6 =>28+6S work; final growth at14. Tests assert cross-field retained raw data. Additional frozen coexistence literals: `10 00 28 01 22 00 38 02 32 00` and `10 00 32 00 38 02 22 00 28 01` each name0/IDsSome1,Some2 and one empty body in each vector, F5/W20+2S/owned2S, last tag8; no ID/inline precedence. This was added at architect plan review before any source implementation. Opaque includes wrong wire4/6/5/7 and malformed nested3/8/31/32.

Prefix table frozen for each fixture: clean boundaries produce selected values accumulated in the left prefix; missing name at empty or explicit prefix2 or duplicates prefix2; every non-boundary cut Truncated at its cut. No incomplete fixture has invalid bytes at the outer wire level. inline boundaries [0,2,6,9,11,14,16], explicit [0,2,4,6,8], duplicates [0,2,4,6,8,10,12,14,16], opaque [0,2,5,7,11,15,17,19,21,23], minimal [0,2]. All cuts covered once, with literals for expected state and inline lists rather than expectations derived from implementation.

Additional tests: signed all four scalar fields with -1 short5/tenbyte, i32MIN/MAX, overwide low32zero/nonminimal; wrong-wire for scalar fields1/2/5/7 (1/2/5 wires), wrong-wire inline4/6 (0/1/5 wires), before/after recognized occurrences, name wrong-wire-only missing. Unknown0/1/2/5 framed fields opaque. Structural wire errors with and without prior name: field0, groups, invalid6/7, truncated tag/value/length/body/fixed fields, negative length, ten continuations. Missing-name versus later structural/owned/work errors tested.

Exact work/precedence literals: 10 00 work0/1=>work0, work2=>work1, work3=>work0, work4=>success; records0/work0=>records0. 10 work2=>work1, work3=>Truncated1. 10 00 22 00 work7=>work2(dispatch), work8=>work2(allocation), work8+S=>success with ownedS; ownedS-1/work8 =>owned_bytes2 before allocation work. 10 00 22 00 32 00 ownedS =>owned_bytes4 (aggregate, not reset). 22 00 owned0 =>owned_bytes0 before MissingName. 10 00 22 02 ff work8=>work4 (short remaining+1 debit), work9=>Truncated5. Huge limit-clamping input >16MiB rejected bytes0; 65536 scalar field attempt fails records at131070; selected repeated inline growth1/2/3/4/5/8/9 exercises exact and minus-one allocation admission, both vectors. Deterministic short hostile bytes/tails use tight finite limits, assert deterministic result/raw borrowing/offset<=len without catch_unwind or timing claim.

## Adversarial preimplementation freeze amendment

Structural literal pairs below use default limits. Each fails with Wire(kind) at offset N; test again with prefix `10 00`, requiring SAME kind at N+2 (structural errors outrank prior selected name). These are pinned wire rules, not outputs from the future decoder.

| hex | kind | N |
|---|---|---|
| 10 | Truncated |1|
| 80 | Truncated |1|
| 00 | InvalidFieldNumber |0|
| 03 | InvalidFieldNumber |0|
| 07 | InvalidFieldNumber |0|
| 0b / 0c / 23 / 24 / 33 / 34 | UnsupportedGroup |0|
| 0e / 0f / 26 / 27 / 36 / 37 | InvalidWireType |0|
| 22 02 ff / 32 02 ff | Truncated |3|
| 0d 01 02 03 | Truncated |4|
| 09 01 02 03 04 05 06 07 | Truncated |8|
| 22 80 80 80 80 08 / 32 80 80 80 80 08 | NegativeLength |1|
| 22 ff ff ff ff 07 | Truncated |6|
| 10 80 80 80 80 80 80 80 80 80 80 | MalformedVarint |10|
| 80 80 80 80 80 80 80 80 80 80 | MalformedVarint |9|
| 22 80 80 80 80 80 80 80 80 80 80 | MalformedVarint |10|

For each scalar tag08/10/28/38 and n1..10 continuation bytes80: n<10=>Truncated at n+1; n10=>MalformedVarint at10. Tag/length cases already above. Empty input =>MissingName0; only wrong-wire name `12 00`=>MissingName2. Prefix flags `08 06` and malformed tail still reports structural N+2, not MissingName.

Deterministic hostile generation finite bounds: for a,b in inclusive0..255, check `[a,b]` and `[16,0,a,b]`; for a in inclusive0..255 and n in0..32 (exclusive32), check n repetitions of a. Fixed Limits {bytes:32,records:8,work:256,owned_bytes:128}; decode twice, compare full Result; success raw pointer/equality and each borrowed occurrence slice is contained in input; error offset<=len. No panic catcher or walltime/RSS assertion. Native execution only hosted Linux CI; no local generator/runtime execution.

## Exact intended source scope

- crates/ktlsp-binary/src/metadata/type_alias.rs (new)
- crates/ktlsp-binary/src/metadata/mod.rs (ONLY serialized additive pub mod type_alias; root slot)
- crates/ktlsp-binary/tests/metadata_type_alias.rs (new)
- crates/ktlsp-binary/METADATA_TYPE_ALIAS_SPEC.md (new frozen selected contract)
- crates/ktlsp-binary/METADATA_TYPE_ALIAS_REVIEW.md (new source/review validation limits)
- crates/ktlsp-binary/tests/fixtures/metadata_type_alias/{minimal,explicit,duplicates,inline,opaque}.hex (new)
- crates/ktlsp-binary/tests/fixtures/metadata_type_alias/{EXPECTATIONS.md,README.md,LICENSE.txt,NOTICE.txt,PROTOBUF_LICENSE.txt} (new); notices byte-copied from existing metadata_wire; EXPECTATIONS is this freeze with corrections from independent review recorded before code.

No existing callers/tests/config/export other than additive mod. No Cargo/lock/workflow/shared helper/rootstatus/docs edits. Existing behavior unchanged. No alias expansion/name/type/symbol semantics, declaration traversal, consumer integration, allocation/RSS/latency comparison or full T018/parity claim.

## Delivery gate

Architect + separate correctness/functionality + adversarial/crash contract reviews, root finite scope/export admission, then smallest source; SAME3 re-review final source and completed ORIGINAL Linux Native Rust CI. Only root-admitted local fmt and compile-only clippy; no local native runtime under #96. Small issue draft below may publish only after root admission/reviews. Push/PR attach/CI then root stable integration cut/exclusive expected-head/base lease, ordinary squash merge, independent actualtree/soleparent/original exact-main attempt1 check receipts. Preserve first failures, no unchanged retry-to-green. Parent #68/#18 stays open; other gates unchanged.

# #125 / T018a3 contract — proposed, preimplementation

Child of #68/#18 after delivered #108/PR112. Frozen base is
6b76477f532f8140a51af1f4492bf46d39864301, tree
920b14ab5bec750f21e1c17994541ac39fe883a3. This decodes schema DATA only.
No names, transformations, range expansion, members, types, semantic symbols,
server consumer, compiler compatibility or LSP parity. Parents remain open.

## Input, API and ownership

`metadata::string_table::decode(bytes: &[u8], Limits) -> Result<StringTableTypes, wire::Error>`
accepts ONE UNFRAMED StringTableTypes message, such as `wire_output.string_table.raw`.
It validates raw bytes anew; public caller-constructible wire fields/spans are not
accepted. All spans/error offsets are relative to this supplied message, not d1.
Header/d1/framing budgets and class/package payload validation are separate calls;
this API does not claim one aggregate budget across those operations.

`StringTableTypes<'a>` has `raw: &'a [u8]`, `records: Vec<Record<'a>>`,
`local_name: Vec<i32>`, `unknown: Vec<wire::Field<'a>>`.
`Record<'a>` has `raw: &'a [u8]`, `span: Range<usize>` (body only),
`range: Option<i32>`, `predefined_index: Option<i32>`,
`string: Option<&'a [u8]>`, `operation: Option<Operation>`,
`substring_index: Vec<i32>`, `replace_char: Vec<i32>`,
`unknown: Vec<wire::Field<'a>>`.
`Operation` is `None`, `InternalToClassId`, `DescToClassId`; data only.
Optional presence is retained; effective absent defaults are range1/index0/
operationNone/stringempty. No convenience transformation or resolver API.
Full borrowed raw messages retain ALL known/unknown duplicates and original order;
unknown vectors additionally retain unknown fields in encounter order, raw bytes,
value and span. This is stronger raw preservation than Java reserialization.

Owned files: new string_table.rs, metadata_string_table.rs, fixture directory,
this spec and METADATA_STRING_TABLE_REVIEW.md. Root-gated additions are
`pub mod string_table;` in metadata/mod.rs and the proposed minimal wire helper
diff (separately submitted before edits). No annotations/lib.rs/Cargo/lock/workflow/
root-doc/vendor/parser/artifact/project changes. Existing wire callers are its
integration tests only; renewed full wire regressions are required.

## Pinned schema compatibility

Kotlin v2.3.20 d57eb4a26ed01ab03cf195da686c8047032ff16f; same protobuf pin and
notices as metadata_wire. Pinned jvm_metadata.proto SHA256
5a80e63021b800afd2297b3713c4cfc12739cbc8e62c726578e1c5d1dfc771f9;
JvmProtoBuf.java SHA256
b1ad864ae872315c06740d6551d2d7233060f5bff3e9ad9b8f80f531018bc3a1.
Sources were read as data only, never executed or bundled.

Table #1/wire2 appends a distinct Record; #5/wire0 or wire2 appends local_name.
Record #1/wire0 is range(default1), #2/wire0 predefined_index(default0),
#3/wire0 Operation(defaultNone: values0/1/2), #4/wire0 or wire2 substring_index,
#5/wire0 or wire2 replace_char, #6/wire2 string(defaultempty bytes).
No required fields; empty table/Record are valid DATA. Recognized singular fields
last-win, repeated fields append in encounter order across mixed packed/unpacked
segments. Empty packed segments append nothing. Negative values/ranges/indices,
odd or long substring/replace lists are preserved without semantic validation.
All int32/enum interpretation uses low32 signed bits, never zigzag. Unknown enum
values are retained as unknown raw fields and do not set/clear operation presence
or overwrite an earlier recognized value. Strings remain raw bytes, even invalid
UTF8. Wrong-wire known numbers are unknown data. Only table#1/wire2 enters a
Record; Record#1/wire2 and all other unknown bytes remain opaque, never recurse.
Wire groups3/4, field0, wires6/7, negative/truncated lengths, malformed varints fail
under unchanged wire rules. Nonminimal low32 tags/lengths and deterministic low64
slow-path values retain #108 compatibility limits. No Java fast-path universality.

## Whole-operation bounds and precedence

`Limits { bytes, records, nodes, depth, work, owned_bytes }`, caller-tightenable.
Hard ceilings: bytes16Mi, records65,535, nodes65,535, depth2, work64Mi,
owned_bytes16Mi. Fixed schema depth table1/Record2; no recursive descent.
One wire::Budget covers ALL root fields, nested Record fields and packed values.
No per-record resets. One schema budget covers ALL nodes/output capacity.
Nested/packed cursors come only from already validated wire payload boundaries.
Call helpers only before their section EOF (clean EOF is free), and propagate
helper errors immediately; budget remains consumed even if caller cursor is unchanged.

Initial byte check at offset0, then root depth1, node, node-work1. Each complete
field uses unchanged wire record1+work1 then byte-read charges (including opaque
payload bytes), followed by schema-dispatch work1 at its tag start. Nested records
pay depth2/node/node-work1 at body start and reread their body bytes/fields under
the SAME wire budget. Packed entries each pay record1+work1 at entry start, varint
read bytes again, then interpretation-work1. Unpacked repeated ints also pay
interpretation-work1 at tag start. Singular assignment/unknown retention is covered
by dispatch-work1. Duplicate/unknown processing always pays these charges.

Output budget charges cumulative requested capacity INCREMENTS of all vectors,
bounding requested RETAINED backing capacity, not cumulative allocation/reallocation
churn:
Record slots, unknown Field slots, int32 slots. Raw strings/messages are borrowed.
Logical capacity grows deterministically from initialized length: at length0 request
capacity1; at each positive power-of-two length request twice that length. Charge
the logical additional slots*sizeof(T) against owned_bytes, then that byte count
against work, then try_reserve_exact(additional relative to current length); report
Limit("allocation") if allocator rejects. Charge the logical reservation even when
allocator surplus makes the reserve call a no-op; never depend on Vec::capacity().
No declared-length allocation or reserve from untrusted range/index/length. Between logical growth points, appends incur no capacity charge; budget never refunded/reset. Temporary old/new reallocation buffers, allocator bookkeeping/rounding/surplus
and stack structs are excluded; this is not actual RSS accounting. Nodes bound root/Record structs;
byte/record/work/output bounds also bound unknown handling and temporary storage.

Wire errors retain #108 offsets/precedence. Schema limit labels nodes/depth/owned_bytes
use node body/tag/entry start respectively. Depth precedes nodes precedes node work;
wire field errors precede dispatch errors. Interpretation work precedes append;
owned_bytes precedes allocation work and allocation failure. No partial result.
Root depth/nodes checks apply even to empty input. Clean section EOF is free.

## Fixture freeze and validation

Fixtures and EXPECTATIONS.md are independently authored/transcribed before source
or runtime; reviewers independently derived values and hostile cases from pinned
schema/generated code. Source-derived corrections and runtime failures must be
preserved separately, never tune expected values to empirical results.
Local native runtime/tests/parser/server/bench/oracle are forbidden (#96 remains
OPEN/UNKNOWN). Local format and compile-only Clippy allowed after source approval.
Existing Linux Native Rust CI must run unchanged fmt, full workspace Clippy and
full workspace tests, including unchanged wire regressions, on reviewed integrated
head and exact merged main. Two retained reviewers plus architect must read back
completed raw logs/behavior and actual checkout parents/tree. Root stable integration
and expected-SHA grants remain required. No broad parity or Darwin fix claim.

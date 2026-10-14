# Frozen VersionRequirement/Table expectations v1

Authored literal bytes and outcomes BEFORE any product runtime/decoder source.
All cases use default limits unless stated. ErrorKind is wire::ErrorKind directly.
S=size_of::<VersionRequirement>(); exact capacities/work use S, not assumed ABI.
Integer tuple order=(version,version_full,error_code,message); '-' means None.
Enum tuple=(level,version_kind); '-' means None (schema defaults remain Error,
LanguageVersion). All raw bytes must equal input; standalone span0..len. Every
unmentioned optional field None. No name/index/version validation.

## Literal files

| File | Expected projection | Fields / work excluding table storage |
|---|---|---|
| empty.hex | all None; table interpretation also empty requirements | 0 / 0 |
| explicit.hex | ints(127,65536,9,7), enums(Hidden,ApiVersion) | 6 / 26 |
| duplicates.hex | ints(2,4,6,8), enums(Hidden,ApiVersion); unknown63 does not overwrite | 14 / 56 |
| wrong_wire.hex | all None; wire2 contents opaque despite invalid-looking packed data | 7 / 47 |
| signed.hex | ints(-1,-1,i32::MIN,i32::MAX); enums(Warning,ApiVersion); high32 truncated for known enums | 6 / 53 |
| nonminimal.hex | ints(1,-,-,-); enums(Error,LanguageVersion); nonminimal/high32 value accepted | 4 / 30 |
| unknown_enums.hex | all None; unknown3/negative enums retain no presence | 4 / 29 |
| table_ordered.hex | 4 entries body spans2..2,4..8,10..14,16..18; first all None; next two(version1,Warning); fourth kindApiVersion; duplicate entries retained | 9 / 46 +4S |
| table_mixed.hex | only2 entries body spans8..10,17..17; first unknown level99=>None, second empty; unknown/wrong-wire outer fields raw | 6 / 31 +2S |
| table_aggregate.hex | entries2..4(version1),6..8(version_full2) | 4 / 20 +2S |
| table_malformed_child.hex | Truncated at4; cursor cannot consume bytes beyond child boundary | outer1+child1 / error path |
| table_truncated_parent.hex | Truncated at4 before dispatch/child/storage | outer1 / error path |

The table projection for explicit/duplicates/etc is NOT implied: only table_* and
empty are table fixtures. No encoder-under-test synthesizes these files.
All .hex hashes/decoded byte hashes are in HASHES.json, computed as data only.

## Independent inline literals, exact offsets and enum/default edges

Standalone empty with bytes0/records0/work0 succeeds all None (no required field).
`08 00 10 00 18 01 20 00 28 00 30 00` => all intsSome0, enumsSome(Error),
Some(LanguageVersion); defaults do not erase presence; work24,records6.
`18 00 30 01`=>Warning/CompilerVersion; `18 02 30 02`=>Hidden/ApiVersion;
`18 01 18 03 30 00 30 03`=>Error/LanguageVersion,records4,work16.
`18 03 18 00 30 03 30 01`=>Warning/CompilerVersion. Unknown-only=>None.
`18 ff ff ff ff 07 30 80 80 80 80 08`=>unknown i32::MAX and MIN=>None.
`18 80 80 80 80 10 30 81 80 80 80 10`=>Warning/CompilerVersion via low32.
Version/full independent: `08 01`=>version1/fullNone; `10 02`=>versionNone/full2;
`08 01 10 02` preserves both with no priority or conversion; negative ints accepted.
`08 80 80 80 80 10`=>Some0 (low32 of2^32); `08 ff ff ff ff ff ff ff ff ff 7f`
=>Some(-1), terminating tenth byte high bits accepted as inherited low64 behavior.
`08 80 80 80 80 80 80 80 80 80 80`=>MalformedVarint at10;
`08 80`=>Truncated at2; `08`=>Truncated at1; `80`=>Truncated at1.
`00`=>InvalidFieldNumber at0; `0e` and `0f`=>InvalidWireType at0;
`0b` and `0c`=>UnsupportedGroup at0; `09 00`=>Truncated at2;
`0d 00 00 00`=>Truncated at4; `0a 02 00`=>Truncated at3;
`0a ff ff ff ff 0f`=>NegativeLength at1;
`08 01 00`=>InvalidFieldNumber at2, no early success.
`0a 01 80`=>allNone, no packed varint processing; `1a 01 80 30 01`
=>levelNone/kindCompilerVersion, does not reject packed scalar body.
`38 01 3d 78 56 34 12 3a 01 00`=>allNone unknown fields retained raw.
`88 80 80 80 10 01`=>version1 using low32 tag8;
`0a 81 80 80 80 10 80`=>allNone (low32 length1, body opaque).
Same standalone wire error cases inside table child `0a <len> <body>` translate
all error offsets by enclosing child body start; exact representative cases below.

## Table structure, bad child boundaries and order

`0a 00`=>one empty entry span2..2; no child fields/extra records; work4+S.
`0a 00 0a 00`=>two empties spans2..2,4..4;records2/work8+2S.
`0a 00 0a 00 0a 00`=>three empties;records3/work12+4S (logical slots4).
`0a 00`*5=>five ordered empty bodies2,4,6,8,10;records5/work20+8S.
`08 01 0d 01 00 00 00 09 01 02 03 04 05 06 07 08`=>empty table,
field1varint/fixed32/fixed64 all wrong-wire opaque;records3/work22.
`12 02 08 80`=>empty table, unknown field2body not decoded;records1/work6.
`0a 02 08 80 0a 00`=>Truncated at4 (first child boundary), not at6 and no
consumption of next outer field. With owned0 still Truncated4, not owned limit.
`0a 01 00`=>InvalidFieldNumber at2; `0a 01 0e`=>InvalidWireType at2;
`0a 01 0b`=>UnsupportedGroup at2; `0a 02 08 80`=>Truncated4;
`0a 02 08 01 00`=>InvalidFieldNumber at4 after successful first entry;
`0a 04 0a 02 00 ff`=>one requirement2..6 allNone: inner scalar wrong-wire
body remains opaque; no third recursion level.
`0a 02 0a 80`=>Truncated4 from missing child length varint; `0a`=>Truncated1;
`0a ff ff ff ff 0f`=>NegativeLength1; `0a 03 08 01`=>Truncated4.
`8a 80 80 80 10 82 80 80 80 10 08 01`=>one child10..12 version1;
low32 table tag/length accepted;records2/work18+S (12 outer+2inner+4dispatch).
Unknown table field0 remains invalid, not discarded.

## Exact budget/error precedence and aggregate controls

Standalone `08 01` needs bytes2,records1,work4; each one lower fails:
bytes1=>Limit(bytes)0;records0=>Limit(records)0;work0=>Limit(work)0(record debit);
work1=>work0(tag read);work2=>work1(value read);work3=>work0(dispatch).
`08 80`: work3=>Limit(work)2 (failed take before Truncated);work4=>Truncated2.
`0a 03 08 01`: work5=>Limit(work)2 (short parent take of remaining2+1),
work6=>Truncated4. Work never reset for child, duplicate or unknown.
`0a 00`: bytes2/records1/work(4+S)/ownedS exact success;
owned(S-1)=>Limit(owned_bytes)0; work(3+S)=>Limit(work)0 at reservation;
work3=>Limit(work)0 at dispatch even with owned0; work4+owned0=>owned_bytes0.
Zero table limits on empty succeed; bytes0 on nonempty wins before every other cap.

Aggregate table_aggregate.hex has B8/I4/F4/N2/C2:
bytes8/records4/work(20+2S)/owned2S exact success; one lower byte=>bytes0;
records3=>records6 on second child's tag; owned(2S-1)=>owned_bytes4 on second
append after child decode; work(19+2S)=>work4 on second reservation.
First child reaches append at work10 then10+S after reservation; second parent
dispatch16+S, second child record17+S, tag18+S, value19+S, dispatch20+S,
second reservation20+2S. No expected values may be chosen from runtime results.

For table_ordered C4: owned(4S-1)=>owned_bytes8 on third append, owned4S=>success;
children share record limits9 (one less fails child last tag16). Allocation growth
atN1,2,3,5 separately frozen above. No allocation based on untrusted length before
framing validation. Caller usize::MAX clamps to hard defaults; bytesMAX_BYTES+1
=>bytes0 before other caps. Hard records: `0a 00` repeated65535 succeeds with
sufficient work/owned;65536=>records131070; root record counter includes children.
Mixed table `0a 02 08 01` repeated32767 +`0a 00` reaches65535 fields exactly;
adding a scalar child consumes beyond shared cap. Work and capacity hard clamps
inspected in helper; deterministic limited cases above prove aggregate behavior.
Strict prefix outcomes are independently fixed: explicit.hex prefix lengths
0,2,6,8,10,12 succeed, projecting the fields completed so far in encoded order;
all other strict prefix lengths1..13 => Truncated at prefix length. The full14
bytes have the explicit projection above. table_ordered.hex strict prefixes
0,2,8,14 succeed with0,1,2,3 ordered entries respectively (same raw prefixes/body
spans/projections as the full fixture); all other strict prefix lengths1..17
=>Truncated at prefix length. Full18 bytes=>four entries above. Deterministic
short-byte/multipart malformed inputs additionally require no-panic checks, not
fuzz claims; clean boundaries are never assumed truncated.
Allocator OOM/32-bit execution, whole-pipeline RSS and JVM fast-path equivalence
remain untested limits; no forced allocation exhaustion or comparative benchmark.

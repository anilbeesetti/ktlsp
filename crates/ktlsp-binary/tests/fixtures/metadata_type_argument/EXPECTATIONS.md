# Independent Type.Argument literal freeze v1 — before source/runtime

Manual expectations from pinned schema/generated parser and unchanged wire
rules; no implementation/encoder/compiler/native parser executed. Unspecified
values None/vector empty; ALL successes retain input raw pointer/bytes identity
and ordered inline raw/span borrowing. Errors are wire ErrorKind at Argument-
relative offset. S=size_of::<InlineType>(), zero k capacity0, otherwise power2.

## Eventual fixture files

| file | literal | selected result | N/F/owned/work |
| --- | --- | --- | --- |
| selected.hex | 08 02 12 02 30 04 18 00 | Some Inv; inline [30 04]span4..6; id Some0 | 8/3/S/14+S |
| duplicates.hex | 08 00 08 63 12 02 30 04 12 02 18 01 12 00 18 01 18 ff ff ff ff 0f 08 03 08 ff ff ff ff 0f | Some Star; inline[30 04]6..8,[18 01]10..12,[]14..14; id Some(-1) | 30/9/4S/48+4S |
| opaque.hex | 0a 01 80 10 01 1a 01 80 a2 06 01 80 12 01 80 | projection/id None; inline[80]14..15 | 15/5/S/25+S |
| pair_argument.hex | 12 02 38 00 | projection/id None; inline[38 00]2..4 | 4/1/S/6+S |

First3 independently authored; fourth genuine Pair.copy return Type first
argument body, with LICENSE/NOTICE unchanged. duplicates distinguishes upstream
merge from false last-body-wins: class_name4 and nullabletrue occur in different
bodies, followed by empty; retain all THREE, never claim a merged Type object.

## Defaults, scalar conversions, order and opacity

[] succeeds all None/empty, including bytes/records/work/owned0.
08 00/01/02/03 -> Some In/Out/Inv/Star respectively, no type/id required.
12 00 -> one empty inline body2..2, projection/id None.
08 03 12 00 18 00 -> Some Star, one empty body4..4, Some0, structurally valid.
08 02 without Type -> Some Inv, structurally valid.
18 00 -> Some0; duplicates18 01 18 00 -> Some0.

For projection and type_id, literal varints interpreted as signed low32:

| value varint | i32 | projection |
| --- | --- | --- |
| 00 / 80 00 | 0 | In |
| 01 | 1 | Out |
| 02 | 2 | Inv |
| 03 | 3 | Star |
| 04 / 63 | 4 / 99 | unknown |
| ff ff ff ff 0f | -1 | unknown |
| ff ff ff ff ff ff ff ff ff 01 | -1 | unknown |
| ff ff ff ff ff ff ff ff ff 7f | -1 | unknown |
| 80 80 80 80 08 | -2147483648 | unknown |
| ff ff ff ff 07 | 2147483647 | unknown |
| 80 80 80 80 10 | 0 | In |
| 81 80 80 80 10 | 1 | Out |
| 82 80 80 80 10 | 2 | Inv |
| 83 80 80 80 10 | 3 | Star |
| 80 80 80 80 80 80 80 80 80 02 | 0 | In |

Unknown enum alone leaves None; known0, unknown99, known2, unknown-1 gives Inv.
For EACH recognized projection value, before/after each unknown above retains
known Some; two known values last-win; exact raw includes unknown bytes.
For EACH selected field1/2/3 test wrong-wire1/5 fixed arbitrary payloads; wire0
wrong for field2, wire2 wrong for fields1/3 (body80). Alone gives no selected
value; before/after known values never changes them. Repeated inline types append
in original order regardless of intervening scalars/unknown/wrong-wire fields.
Field3/wire2 packed-looking 1a 02 01 02 stays raw, id None, no packed reread.

Unknown fields all supported wires: a0 06 01; a1 06 01 02 03 04 05 06 07 08;
a2 06 03 ff 00 0b; a5 06 01 02 03 04; reserved19000 c0 a3 09 01;
maximum536870911 f8 ff ff ff 0f 01. Alone all None/empty; append to selected
fixture leaves its selected fields unchanged and full raw exact. Nested Type
bytes80/00/0b/ff/invalid UTF8 are opaque successes; empty/incomplete body retained.
Nonminimal tag98 00/value80 00 -> type_id Some0. Low32-aliased tag
98 80 80 80 10 value01 -> type_id Some1. Nonminimal length12 80 00 -> empty
body3..3; low32-aliased length12 80 80 80 80 10 -> empty body6..6.

## Structural failures and tail precedence

| bytes | kind@offset |
| --- | --- |
| 08 / 18 / 80 | Truncated@1 |
| 00 / 03 / 07 | InvalidFieldNumber@0 |
| 0b / 0c / 13 / 14 / 1b / 1c | UnsupportedGroup@0 |
| 0e / 0f / 16 / 17 / 1e / 1f | InvalidWireType@0 |
| 12 02 ff | Truncated@3 |
| 0d 01 02 03 | Truncated@4 |
| 09 01 02 03 04 05 06 07 | Truncated@8 |
| 12 80 80 80 80 08 | NegativeLength@1 |
| 12 ff ff ff ff 07 | Truncated@6 |
| ten80 | MalformedVarint@9 |
| 08 then ten80 /18 then ten80 /12 then ten80 | MalformedVarint@10 |

For EACH row also prepend selected.hex, expect same kind at offset+8, no early
success. Each selected tag08/18/12 with1..9 continuation bytes -> Truncated at
supplied end; ten continuations -> Malformed at tenth continuation. For12 this
is length varint. Full length-delimited Type bodies containing same malformed
bytes succeed and remain opaque, never nested errors. Field0 beats invalid-wire.

## Every byte cut: exact exhaustive partition,61 cuts

For each file cut at n0..=N. T cuts -> Truncated@n. Listed success boundaries
retain exact prefix raw and only complete selected fields; these partition all
cuts. No initialization checks at field-boundary EOF.

- selected N8: success[0,2,6,8], T[1,3,4,5,7]. n2 Inv only; n6 adds
  inline30 04 span4..6; n8 adds id0.
- duplicates N30: success[0,2,4,8,12,14,16,22,24,30],
  T[1,3,5,6,7,9,10,11,13,15,17,18,19,20,21,23,25,26,27,28,29].
  n0 none; n2 In; n4 still In; n8 first body; n12 second body; n14 empty
  third body; n16 id1; n22 id-1; n24 Star; n30 still Star.
- opaque N15: success[0,3,5,8,12,15], T[1,2,4,6,7,9,10,11,13,14].
  n<=12 all None/empty; n15 inline80 span14..15, others None.
- pair N4: success[0,4], T[1,2,3]; n4 inline38 00 span2..4.

Count9+31+16+5=61. Every successful prefix owns only capacity for its complete
bodies; errors drop partial allocation/results.

## Exact budgets, growth, clamping and first failures

All4 file exact N/F/owned/work above and each+1 pass. bytesN-1 -> bytes@0 before
records0/work0. recordsF-1 -> records at last field tag selected6,duplicates24,
opaque12,pair0. ownedC*S-1 fails at last growth tag selected2,duplicates12,
opaque12,pair0. workTotal-1 -> work at selected6(dispatch),duplicates24(dispatch),
opaque12(capacity),pair0(capacity). With owned one below AND work4 for12 00,
owned_bytes@0 precedes capacity work; with work3 dispatch work@0 precedes owned.

12 00: records0/work0 -> records@0; records1/work0 or1 -> work@0;
work2 -> work@1(length read); work3 -> work@0(dispatch); work4 and ownedS ->
work@0(capacity); work4+S/ownedS succeeds. records1/work4+S/ownedS-1 -> owned@0.
18 00: records1/work0/1 -> work@0;work2 -> work@1;work3 ->work@0(dispatch);
work4/owned0 succeeds. 18 with work2 -> work@1;work3 -> Truncated@1.
12 02 ff with work4 -> work@2(short bulk debit2);work5 -> Truncated@3.
18 00 80 with records1/work4 -> records@2;records2/work6 ->work@3;
records2/work7 ->Truncated@3. 18 00 00 work6 -> InvalidFieldNumber@2.

k1..5 literal12 00 repeated: C=[1,2,4,4,8], N2k/Fk/W4k+C*S/ownedC*S.
Exact/+1 pass; ownedC*S-1 fails at growth tag[0,2,4,4,8] (k4 fails at tag4,
before lastfield6); workW-1 fails at tag[0,2,4,6,8] (k4 lastdispatch6).
Opaque12 80 08 then1024ff: N1027/F1/W1029+S/ownedS, body1024 bytes remains
borrowed; work1028 -> work@0(dispatch),work1029 -> work@0(capacity), exact passes.
No body-sized heap allocation, recursion, field reset/refund or nested reread.

usize::MAX clamps. MAX_BYTES+1 input -> bytes@0 even records0/work0;
65535 copies18 00 with unlimited caller limits -> N131070/F65535/W262140,
idSome0 success; one extra -> records@131070. Hard work/owned ceilings cannot
be independently exhausted by successful scans within bytes/fields on ordinary
32/64-bit layouts (S16/32); test tightened exact limits, no invented witness.
Empty input under all0 succeeds. Allocation failure mapping static review only.

Finite future hostile probe: all a,b0..255 decode[a,b] and[18,00,a,b] twice,
plus each a repeated n0..31 twice, limits bytes32/records8/work512/owned256.
139264 inputs/278528 calls. Assert identical complete Results/raw+span borrowing
on success, error.offset<=len, span bounds/order. No local execution admission;
eventual unchanged original Linux Native Rust CI, no timing/RSS claim.

## Genuine Pair composition — intended, UNEXECUTED

Existing kotlin-stdlib2.3.20 JAR SHA256
0ae12504a5040ebaf37703908483420d1a5624dd1d93f357665f8c77c848a01e,
same archive pinned by t017c2, local source there. Unmodified kotlin/Pair.class
2983 bytes SHA2568fed0fa88eb2df5723812a95330dee6bc91fd66b13906acb7906f9a813da9736;
distribution2.3.20-release-208, exact compiler provenance unverified. Copied using
unzip -p as data only. Direct manual xxd transcription: CP89 d1 MUTF8 bytes
1276..1599, leading NUL then direct low8 code units. Decoded d1 length285;
prefix2e/table46bytes at1..47, Class47..285. Class flagsSome1031/fq_name0.
Class field9 copy is d1[149..198], Function body151..198, literal:
10 11 1a 0e 12 04 12 02 38 00 12 04 12 02 38 01 30 00
32 08 08 02 10 05 1a 02 38 00 32 08 08 02 10 06 1a 02 38 01
48 c6 81 04 a2 06 02 10 12
Function name17,flags65734,old_flagsNone; header.d2[17] exact UTF16 copy.
Function Type body relative4..18=d1[155..169]:
12 04 12 02 38 00 12 04 12 02 38 01 30 00.
Type Argument bodies relative2..6 and8..12=d1[157..161]and[163..167]; first
matches pair_argument.hex. First inline Type body Argument-relative2..4=
d1[159..161]=38 00, second=38 01. No projection explicitly written, default
Inv remains absent, type_id absent, no type graph or applied generic semantics.

Future test after TypeHeader198 DELIVERY and root-assigned integration base:
actual parse_class -> annotations::decode -> metadata::decode, assert k1/
mv[2,3,0]/d1 length285 -> payload::decode and actual class_header::decode,
assert Class raw pointer=d1[47..285],flags1031/fq0. Separate wire::decode(full d1)
selects payload field9 whose body equals WHOLE47byte copy literal; assert its
field149..198/body151..198 and borrowed containment in Class raw.
Actual function_header::decode(body), assert name17/flags65734/oldflagsNone/raw.
Only AFTER whole literal identity assert function.raw[2..4]=1a0e and select
function.raw[4..18]. Actual TypeHeader198 type_header::decode(Type) asserts
class_nameSome0/all other scalarsNone/raw pointer. Use frozen Type literal offsets ONLY
after complete literal identity to select first Argument Type.raw[2..6] and
second Type.raw[8..12], asserting tags/lengths1204. Actual type_argument::decode
each with bytes4/records1/work6+S/ownedS -> projectionNone/idNone, one ordered
inline raw38 00 or38 01/span2..4; pointer equality to d1 spans above.
Actual type_header::decode(each inline Type) with bytes2/records1/work4 ->
type_parameterSome0/Some1, all other scalarsNone. Separate existing name resolver
get_string(0) -> UTF16 kotlin/Pair; get_string(17) -> UTF16 copy. Expectations
follow table records and d2 data, not raw d2[0] guessing or produced decoder
outputs. All API budgets separate. Fixture-specific offsets are no general
container walker. NO executed composition, oracle/native/compiler/regeneration.

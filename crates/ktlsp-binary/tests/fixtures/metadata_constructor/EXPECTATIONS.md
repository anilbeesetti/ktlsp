# Constructor independent literal/arithmetic freeze v1

No product/compiler/encoder execution supplies expectations. S=sizeof(ValueParameterBody),
I=sizeof(i32). B below logical owned capacity; not assumed platform ABI size.
Every success raw and each body pointer-identical to supplied slices; spans body-only.

## Literal fixtures and every-byte cuts

selected19 bytes/6 fields: flagsSome0; bodies[(4..6,1005),(16..16,empty)];
versions[1,2,3,4]. K2,L2,V4 =>records8,B2S+4I,work39+B.
Success cuts:0 flagsNone/lists[];2 flags0/lists[];6 adds body4..6;
9 adds ID1;14 IDs[1,2,3];16 adds empty body16..16;19 IDs[1,2,3,4].
Truncated@cut:[1,3,4,5,7,8,10,11,12,13,15,17,18].
Cut13 is OUTER payload availability failure, not successful partial packed list.

duplicates24 bytes/8 fields: flagsSome0; bodies[(4..5,80),(15..16,80)];
versions[0,127,128]. K2,L3,V3=>records10,B2S+4I,work48+B.
Success cuts:0 empty/flagsNone;2 flags1;5 body4..5;8 ID0;10 flags6;
13 same (empty packed);16 second body15..16;18 flags0;24 IDs[0,127,128].
Truncated@cut:[1,3,4,6,7,9,11,12,14,15,17,19,20,21,22,23].
Malformed parameter80 stays opaque, even after duplicates; no nested validation.

opaque22 bytes/6 fields: flagsNone,bodies[],versions[],records6,B0,work34.
Success cuts[0,3,5,11,14,18,22], same empty values.
Truncated@cut:[1,2,4,6,7,8,9,10,12,13,15,16,17,19,20,21].
Includes wrong-wire flags1/parameters2/versions31, opaque annotation3/plugin32/
extension100 with invalid inner00/ff/80. No selected presence manufactured.

pair_constructor25 bytes/4 fields: flagsSome70,bodies[(4..10,10051a023800),
(12..18,10061a023801)],versions[],records4,B2S,work33+B.
Success cuts0 empty/None;2 flags70;10 first body;18 both bodies;25 same.
Truncated@cut:[1,3,4,5,6,7,8,9,11,12,13,14,15,16,17,19,20,21,22,23,24].
Total94 prefix cases incl complete/empty. Unframed complete boundaries may succeed;
no claim of original container completeness. No body slices retained on error.

## Selected field matrix

Flags1 and each form of version31 independently: absent/explicit0/6/300;
-1 via5bytes ffffffff0f and10bytes ffffffffffffffffff01; i32MIN8080808008,
i32MAXffffffff07; nonminimal0=8000; low32 overflow8080808010=>0.
Each int32 raw truncates low32; no zigzag/index/range/bit interpretation.
Flags duplicates last-recognized wins across unknown/wrong-wire interleaving.
Versions packed/unpacked before/after/mixed append exactly, including repeated
negative/extreme values and empty packed list; no sorting/dedup/default IDs.
Example f80101 fa01020203 f80104 =>[1,2,3,4].
Nonminimal flags tag8800 then8000=>Some0. Low32 aliased tag8880808010 00=>Some0.
Low32 aliased version tag f881808010 00=>[0].

Wrong-wire1(wires1/2/5),2(wires0/1/5),31(wires1/5) alone yields no selected
projection; before/after baseline080612021005f80107 preserves flags6/body/ID7.
Wire2 version31 is ALWAYS selected packed: fa010180 fails Truncated4, never opaque.
Field2 wire2 empty succeeds and delivered ValueParameter on empty separately
fails MissingName0; field2 wire2 00/80/0b/ff succeeds as opaque body.
Interleave duplicate bodies and flags/IDs; exact encounter order/spans retained.
Unknown fields0 are malformed; valid fields3/32/100/18999/19000/max536870911
are opaque (no extension-range enforcement); all legal fixed/bytes/varint wire
forms scanned without interpreting raw nested content. Late malformed tail
aborts whole result despite earlier flags/parameters/IDs.

## Structural and packed errors (wire kinds unchanged)

| literal | error@offset |
| --- | --- |
| 08 / 12 / f8 / f801 / fa01 / 80 | Truncated@len |
| 00 /03 /07 separately | InvalidFieldNumber0 |
| 0b /0c /13 /14 /fb01 /fc01 | UnsupportedGroup0 |
| 0e /0f /16 /17 /fe01 /ff01 | InvalidWireType0 |
| 1202ff | Truncated3 |
| 0d010203 | Truncated4 |
| 0901020304050607 | Truncated8 |
| 128080808008 | NegativeLength1 |
| fa018080808008 | NegativeLength2 |
| 12ffffffff07 | Truncated6 |
| 08 +ten80 | MalformedVarint10 |
| ten80 | MalformedVarint9 |
| fa0101800800 | Truncated4 (packed end; following0800 cannot rescue) |
| fa010201800800 | Truncated5 (first ID1 provisional, whole call fails) |
| fa010a +ten80 +0800 | MalformedVarint12 (packed tenth continuation) |

Prefix each malformed row with0806 and separately12021005; expected kind unchanged,
offset adds prefix length; no return of partial selected data. Packed values
1..9 continuation80 with matching lengths =>Truncated at packed end, even with
following0800; ten continuations=>Malformed at tenth. Short outer packed payload
fails during outer wire read before dispatch/items/reservations. Nested parameter
contents alone never cause those errors, because no nested scan occurs.

## Exact budget/error precedence

All fixture exact N/records/work/B and each+1 succeed. bytesN-1 =>Limit(bytes)0
FIRST even records/work/owned0. One-less records: selected@16,duplicates@22,
opaque@18,pair@18. One-less work: selected@16(integer work),duplicates@22(last
ID capacity),opaque@18(dispatch),pair@18(dispatch). One-less owned (nonzero B):
selected@14(second body),duplicates@22(ID growth),pair@10(second body).
Empty all0 succeeds; nonempty bytes0 fails bytes0. usize::MAX clamps.

0800: records1/work4/owned0 exact. work0/1=>work0;2=>work1;3=>work0(dispatch).
records0/work0=>records0. 08: work2=>work1(failed value read);work3=>Truncated1.
1200: records1,work4+S,ownedS. work3=>work0(dispatch),work4/ownedS=>work0(growth),
work4/owned0=>owned_bytes0 (owned precedes capacity work).
1201ff: work4=>work0(dispatch),work5/ownedS=>work0(capacity),
work5/owned0=>owned_bytes0; exact work5+S.
1202ff: work3/4=>work2(short bulk debit remaining+1);work5=>Truncated3.

f80100: N3/F1/V1 work6+I,records1,ownedI. work4=>work0(dispatch),
work5/owned0=>work0(integer before owned),work6/owned0=>owned_bytes0,
work6/ownedI=>work0(capacity). All limit offsets0.
fa0100: N3/F1,records1,work5,owned0;work4=>work0(dispatch).
fa010100: N4/F1/K1/L1/V1=>records2,work9+I,ownedI.
records1=>records3; work5=>work0(dispatch);work6=>work3(packed record);
work7=>work3(reread);work8=>work3(integer);work9/owned0=>owned_bytes3;
work9/ownedI=>work3(capacity); exact9+I success.
fa0101800800: records2,work8=>work4(failed packed read),work9=>Truncated4;
records1/work6=>records3;work6=>work3(record);work7=>work3(reread);
work8=>work4(failed read). No allocation before failure, following fields unscanned.

Growth at retained lengths0/1/2/4 tested for BOTH vectors. Five empty parameters
(1200)*5: N10,F5,records5,owned8S,work20+8S; one-less owned/work=>@8.
Five unpacked IDs(f80100)*5: N15,F5,V5,records5,owned8I,work30+8I;
one-less owned/work=>@12. Five packed IDs fa01050000000000:
N8,F1,K5,L5,V5,records6,owned8I,work25+8I; one-less owned/work=>@7.
Mixed vectors:1200 f80100 1200 fa01020000 =>N12,F4,K2,L2,V3;
records6,owned2S+4I,work27+2S+4I. One-less owned/work=>@11 (third ID growth).
No per-list/per-parameter/packed-occurrence reset; test shared work/owned exhaustion.

Hard records witness: (0800)*65535 N131070,records65535,work262140,owned0
succeeds; append0800=>records131070. Packed record ceiling includes outer fields:
fa01fdff03 followed65533 zero bytes =>N65538,F1,K65533,V65533,records65534;
append f80100=>records65535 succeeds if other limits sufficient, next unpacked
ID fails records at its tag. Default owned262144 for ID vector within hard16MiB.
ByteMAX+1 =>bytes0 before all checks. No independent hard-work success witness;
static clamping plus exact reachable work edges. No forced allocator exhaustion;
static inspect try_reserve mapping, checked arithmetic and helper unchangedness.

Finite hostile probes: all65536 two-byte literals with bytes32/records8/work256/
owned1024; paired equality, success raw/body pointer identity/spans<=len,
error offset<=len. Repeat each byte0..255 length0..31. Also literal packed
boundary probes above and paired complete prefixes. No new broad fuzz harness,
all-input/CPU/timing/RSS/stack or official compatibility claim. Runtime tests
remain FUTURE and require coordinator's source/CI grants.

# Independent literal expectations v1 — frozen before product source/runtime

All offsets d1-relative unless explicitly stated. R=size_of::<string_table::Record>(),
M=size_of::<package_member_names::Member>(), S=size_of::<&[u8]>(). These are portable
logical-layout quantities, not observed compiler/native outputs. Byte literals are
authored/transcribed without running any ktlsp/JVM/compiler/encoder/decoder.

## Genuine KIND2, original source bytes

fixtures/KTypeParameterBaseKt.class.bin is the unmodified 368-byte archive member
kotlin/jvm/internal/KTypeParameterBaseKt.class in kotlin-stdlib:2.3.20, original JAR
SHA2560ae12504a5040ebaf37703908483420d1a5624dd1d93f357665f8c77c848a01e.
Member SHA256ddbf21c6cea2ad2f18d809e57a8b5b1e59a42d55a3f622b6e4b1295684d692af.
Origin/local archive path/full hashes/notices in PROVENANCE.json. Annotation k2,
mv[2,3,0],xi48; raw d1 units begin0 followed by the 26 exact byte values below;
d2 units [[84,121,112,101,80,97,114,97,109,101,116,101,114,67,111,110,116,97,105,
110,101,114],[],[107,111,116,108,105,110,45,115,116,100,108,105,98]]. bv/xs/pn
absent, use existing header defaults/presence; no fabricated compiler provenance.

Decoded d1 (26 bytes), SHA2569ff8bb7084c7999234ee5745121782679818c75e165e0669f0e4c50fe9c3d15f:
08 0a 00 0a 02 10 00 0a 00 2a 0c 08 00 10 00 22 02 30 01 32 02 30 01 a8 06 02
Prefix0..1, table1..9, Package9..26. Compact table records:
- body table-relative2..2 (d1 3..3): all optionals absent => range1.
- body table-relative4..6 (d1 5..7): predefined_indexSome0, other optionals absent.
- body table-relative8..8 (d1 9..9): all optionals absent => range1.
local_name/unknown empty. Record1 predefined0 is pinned kotlin/Any; it is NOT the
queried member name. Index0 queries record0 => d2[0].

Expected ONE member: TypeAlias ordinal0 span9..23 body11..23,
name[84,121,112,101,80,97,114,97,109,101,116,101,114,67,111,110,116,97,105,110,101,114]
("TypeParameterContainer"), flagsSome0,old/getter/setterNone.
Raw alias body080010002202300132023001 has name0, flagsSome0, underlying/expanded
IDsNone, one inline body3001 each. Field101 ordinal1 span23..26 raw a80602 opaque.
Do not infer alias expansion, Any type identity, container/interface symbols or
semantic package/facade assembly from this data.

Future integration MUST parse_class -> annotations::decode -> metadata::decode ->
NEW package_member_names::decode and assert literal header/d1/table/name/raw spans.
No repeated manual traversal in place of the new API. Exact operation limits:
bytes96 (26 d1+70 d2 bytes),records11,nodes6,depth2,entries6,range_slots3,
queries1,output_units22,owned4R+2S+M+44,work211+4R+2S+M.
Manual work: framed table24+4R, Package node1, resolver constructor76,
outer scan21,alias node1,alias header20+2S,name query68,output pushM.
One-less work=>Wire Limit(work)23 (last extension dispatch);
records10=>Wire Limit(records)23; owned one-less=>Wire Limit(owned_bytes)9
(member output push after name). These are equations, not executed results.

## Authored MIXED fixture

fixtures/mixed.hex, 55 bytes; d2=[[102],[112],[97],[102]] =>8 input bytes.
04 0a 02 08 04 08 00 1a 06 08 00 10 00 48 06 22 0a 08 00 10 01 38 00 40 01
58 00 2a 0c 08 00 10 02 22 00 22 01 00 32 01 00 1a 02 10 03 18 00 f2 01
01 ff a2 06 01 00
Prefix0..1/table1..5/Package5..55. One compact Record body table2..4 (d1 3..5),
rangeSome4. Every other optional/list/unknown absent/empty.

Literal ordered outputs (kind,ordinal,span,body,name,F/O/G/S):
Function,1,7..15,9..15,[102],Some6/Some0/None/None
Property,2,15..27,17..27,[112],Some0/Some0/Some0/Some1
TypeAlias,3,27..41,29..41,[97],Some0/None/None/None
Function,4,41..45,43..45,[102],None/None/None/None

Other fields: ordinal0 unknown0800 at5..7; ordinal5 wrongwire Function1800
at45..47; ordinal6 opaque type_table f20101ff at47..51 (body ff invalid but opaque);
ordinal7 unknown a2060100 at51..55. Alias raw080010022200220100320100 retains
underlying inline empty then00 (fields33..35/body35..35,35..38/body37..38),
expanded00 (field38..41/body40..41). All malformed inline Types stay opaque.
Duplicate f names remain distinct; no symbol identity/overload discrimination.
Identical raw outer records: P 1a021000 1a021000 (P=040a020804),same d2 =>
two Function records ordinal0/1 span5..9/9..13 body7..9/11..13, both name[102],
all F/O/G/S None. No byte-level or name-level deduplication.

Exact mixed limits: bytes63,records25,nodes7,depth2,entries5,range_slots4,queries4,
output_units4,owned R+3S+4M+8,work176+R+3S+4M. Exact and +1 each succeed.
Records=envelope1+table2+outer8+selected headers(3+5+5+1)=25.
Nodes=table2+Package1+members4=7. Work=payload14+R,Package node1,
resolver constructor13,outer66 (50 wire bytes+8 records+8 dispatch),member nodes4,
headers58+3S (12+20+22+4 plus alias vector allocation work),queries20,
member capacity4M. Alias transient vectors consume3S even after they are dropped;
no refund and no per-member Budget resets. Names consume8 owned bytes.

One-less controls with other fields exact:
bytes62=>Wire Limit(bytes)0; records24=>Wire Limit(records)51;
work175+R+3S+4M=>Wire Limit(work)51 (last dispatch);
owned R+3S+4M+7=>Name Limit(owned_bytes),indexSome3/recordSome0,offset43;
nodes6=>Wire Limit(nodes)43; depth1=>Wire Limit(depth)3;
entries4=>Name Limit(entries),indexNone/recordNone,offset1;
entries3=>Name Limit(entries),indexNone/recordNone,offset0 (before bytes0/records0/work0);
range_slots3=>Name Limit(range_slots),indexNone/recordSome0,offset1;
queries3=>Name Limit(queries),indexSome3/recordNone,offset43;
output_units3=>Name Limit(output_units),indexSome3/recordSome0,offset43.

Mixed owned R+3=>Wire Limit(owned_bytes)7 (first Member push after first name),
owned R+1=>Name Limit(owned_bytes),indexSome0/recordSome0,offset9.
First query pre-output-byte work consumed: payload14+R,Package1,resolver13,
unknown4,first outer10,member1,header12,query1+record1+source1 =58+R.
Thus work R+59 with owned R+1 => Name Limit(work)0/0 at9 (output-byte debit2
before owned debit); work R+60 same owned => Name Limit(owned_bytes)0/0 at9.
Every-vector capacity0/1/2/4 growth retains existing formula; alias multiple-list
allocations must remain charged before alias query/output. Standalone alias tests
retain frozen equations; new shared helper must not alter any outcomes.

Every mixed byte cut n=0..55:
- n=0..4 => Wire Truncated at n (envelope requires entire table).
- success n5 or7 =>[]; n15=>first1; n27=>first2; n41=>first3;
  n45/47/51/55=>all4. Successful records retain exact spans/names/flags above.
- every other n => Wire Truncated at n from outer Package framing.
Complete unframed field-boundary prefixes legitimately succeed; no completeness
claim. Finite cut expectations are literal, not taken from the new implementation.

## Empty Package and container controls

minimal d1=00,d2=[] =>empty Vec. Exact bytes1,records1,nodes2,depth1,work4,owned0,
entries/range_slots/queries/output_units0. records0=>Wire Limit(records)0;
work3=>Wire Limit(work)1 (Package node); nodes1=>Wire Limit(nodes)1;
depth0=>Wire Limit(depth)1 (typed table root). bytes0=>Wire Limit(bytes)0 before
records/work/nodes. Empty d1=>Wire Truncated0 with default limits. minimal cuts:
n0=>Wire Truncated0,n1=>success[]. d1 00 18 00 20 00 28 00 =>empty (wrongwire3/4/5).
d1 00 00 =>Wire InvalidFieldNumber1; 00 0b =>Wire UnsupportedGroup1.
Each supported input works at k2 AND k5. k1/3/4/negative or wrong mv, Absent,
Unsupported or forged Decoded with wrong kind/version => UnsupportedContainer0
before all zero limits. Supported caller-constructed Decoded values traverse normal
preflight and decoding; the guard makes no authenticity claim.
d2 MAX_RECORDS+1 empty slices with usize::MAX limits=>Name Limit(entries)None/None0
before any per-entry traversal. All usize::MAX caller values clamp; no arithmetic
wrap or raising native hard limits. Combined input MAX_BYTES+1=>bytes0.

## Selected malformed/header/name/error-order controls

Use prefix P=04 0a02 0804, Package starts5, d2 as MIXED, default limits.
For each selector t=1a(Function),22(Property),2a(TypeAlias):
P t 00 => matching typed MissingName at7.
P t 01 80 => matching typed Wire Truncated at8.
P t 01 00 => matching typed Wire InvalidFieldNumber at7.
P t 02 10 04 => Name InvalidIndex,indexSome4/recordNone,offset7.
P t 06 10 ff ff ff ff 0f => Name InvalidIndex,indexSome(-1)/recordNone,offset7.
P t 0b 10 ff ff ff ff ff ff ff ff ff 01 => same signed -1 Name error.
P t 03 10 00 0b => matching typed Wire UnsupportedGroup9 (before query).
P t 03 10 00 00 => matching typed Wire InvalidFieldNumber9.
P t 00 00 => matching typed MissingName7 BEFORE later outer invalid tag7.
P t 02 10 04 00 => Name InvalidIndex4/None7 BEFORE later outer invalid tag9.
P t 02 10 00 00 => outer Wire InvalidFieldNumber9 AFTER earlier successful name.
P t 04 10 00 1a 01 => matching typed Wire Truncated11 (selected later wire framing).
P t 04 10 00 1a 00 =>success one member, opaque nested field3 bodyempty.

Duplicate selected scalar control body10 03 10 00 08 01 08 06 => name0,last flags6
for Alias; Function old_flags6/flagsNone; Property old_flags6/flagsNone. Body10 00
12 00 => required name0 remains after wrongwire name. Body12 00=>MissingName atEOF.
Body10 80 80 80 80 10 =>low32 name0, accepted. Body10 80 00 =>nonminimal name0.
Flags rawnegative/nonminimal retain signed low32 per existing readers; no defaults.

UTF16 d2 variants with table range4 and one selector/name0 retain []/[0]/[55296]
exactly. Query output and owned limits must use each actual emitted unit count.
Selected malformed protobuf UTF8 d1=05 0a03 3201ff t02 1000,d2=[] =>
Name UnsupportedUtf8,indexSome0/recordSome0,offset8. Same table alone (first6bytes)
=>empty success: invalid UTF8 remains unqueried. Existing resolver emits transformations
without name validation; no lenient replacement is added.

Unknown enum table case d1=0a 0a08 0801 1801 187f 3200 t02 1000,d2=[]:
Record rangeSome1,operationSomeInternalToClassId (recognized1 retained over unknown127),
stringSomeempty,unknown187f. Selected name resolves []; unknown enum never clears
recognized operation. Four bytes 1801 then187f are not an unknown selector exception.
Duplicate/range/substring/replace/predefined rules reuse existing pinned resolver.

Finite future probes: for every a,b byte decode [00,a,b] twice with supported k2/5,
d2=[],limits bytes32/records8/nodes4/depth2/work256/owned1024, compare exact Results,
ensure every success span/body is ordered/inbounds and selectors/names match literal
selected framing. Repeat n0..31 of each byte and mixed fixture every-cut table.
These are future tests, never run here; finite probes do not prove universal
CPU/RSS/stack/JVM compatibility. Allocator OOM is reviewed via try_reserve error
propagation; no forced allocation exhaustion experiment is promised.

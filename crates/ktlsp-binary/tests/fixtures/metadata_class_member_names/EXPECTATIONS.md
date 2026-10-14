# Independent expectations frozen before source

Contract v3 SHA2566da02f889ef6cc539040a8012352004cd751704d22bfa4d1c42defa0c78279fe
was independently approved by architect/correctness/adversarial before implementation.
No native runtime provided expected outputs.


MIXED d1 (43 bytes), d2 units [[67],[102],[112],[102]] (8 bytes):
04 0a 02 08 04 18 00 4a 06 08 00 10 01 48 06 52 0a 08 00 10 02 38 00 40 01
58 00 4a 02 10 03 42 01 ff 5a 01 80 48 00 a2 06 01 00
table1..5 has one compact record body3..5, rangeSome4. Class5..43.
Literal ordered records (kind,ordinal,field span,body span,name,F/O/G/S):
Function,1,7..15,9..15,[102],Some6/Some0/None/None
Property,2,15..27,17..27,[112],Some0/Some0/Some0/Some1
Function,3,27..31,29..31,[102],None/None/None/None
Remaining Class fields: fq_name ordinal0 5..7; constructor ordinal4 31..34;
alias ordinal5 34..37; wrongwire Function ordinal6 37..39; unknown ordinal7 39..43.
No semantic overload discrimination; the two f records MUST both remain.

Let R=size_of::<string_table::Record>() and M=size_of::<Member>(). These are
platform layout quantities, never compiler-output-derived expectations. Logical
table capacity R, member vector capacity4M, name bytes6. Exact mixed requirements:
bytes51, records28 (envelope1+table2+Class scans16+members9), nodes6 (table2+Class1+
members3), depth2, entries5, range_slots4, queries3, output_units3,
owned_bytes R+4M+6; work190+R+4M.
Manual work derivation: payload14+R; Class scans108; resolver constructor13;
selected header scans36; queries15 (each query1+record1+source1+outputbytes2);
Class/member nodes4; output vector4M. Exact and +1 succeed; below:
bytes50=>Wire Limit(bytes)0 (including records/work0 precedence),
records27=>Wire Limit(records)39 (last second-scan unknown),
work189+R+4M=>Wire Limit(work)39 (last second-scan dispatch),
owned R+4M+5=>Wire Limit(owned_bytes)27 (third output growth),
owned R+1=>Name Limit(owned_bytes),indexSome1/recordSome0 at9 (first name BEFORE
try_reserve_exact, not the later output Vec push),
owned R+1 AND work R+113=>Name Limit(work),indexSome1/recordSome0 at9 (first name
output-byte work debit BEFORE owned debit); workR+114 with same owned limit gives
the Name Limit(owned_bytes) above. These are manual partial-work sums through the
first member, not thresholds discovered by implementation.
nodes5=>Wire Limit(nodes)29 (third selected node), depth1=>Wire Limit(depth)3,
entries4=>Name Limit(entries),indexNone/recordNone at1,
entries3=>Name Limit(entries),indexNone/recordNone at0 (O(1) d2 preflight, even
with bytes0/records0/work0); d2 lenMAX_RECORDS+1 empty slices fails identically
with all caller limitsusize::MAX, before any d2 iteration,
range_slots3=>Name Limit(range_slots),indexNone/recordSome0 at1,
queries2=>Name Limit(queries),indexSome3/recordNone at29,
output_units2=>Name Limit(output_units),indexSome3/recordSome0 at29.

MINIMAL d1=00 18 00, d2=[], legitimate empty result after complete required-Class
validation. Exact bytes3,records3,nodes2,depth1,work12,owned0,entries0/range0/
queries0/output0. work11=>Wire Limit(work)1 (second fq dispatch); records2=>Wire
Limit(records)1. bytes2=>Wire Limit(bytes)0. nodes1=>Wire Limit(nodes)1.
00 alone=>Class MissingFqName1; 00 08 06=>Class MissingFqName3.
00 18 00 00=>Class Wire InvalidFieldNumber3. Empty d1=>Wire Truncated0.
00 18 00 48 00 50 00=>empty; wrong-wire member tags do not select.

Selected failures with table/d2 from MIXED (prefix04 0a02 0804; Class begins5):
- Class18 00 4a 00 => Function MissingName9.
- Class18 00 52 00 => Property MissingName9.
- Class18 00 4a 01 80 => Function Wire Truncated10.
- Class18 00 52 01 00 => Property Wire InvalidFieldNumber9.
- Class18 00 4a 02 10 04 => Name InvalidIndex,indexSome4,recordNone,offset9.
- Class18 00 4a 06 10 ff ff ff ff 0f => Name InvalidIndex,indexSome(-1),recordNone,offset9.
- Class18 00 4a 04 10 01 0b => outer truncated member length fails Class Wire
  Truncated12; use length03 instead => Function Wire UnsupportedGroup11.
- Class18 00 4a 02 10 01 00 => Class Wire InvalidFieldNumber11 BEFORE member query.
Add matching Property bad index/malformed/group cases. Present name never skips
later structural failures. Unselected malformed nested bytes pass.

UnsupportedUTF8 selected literal d1=05 0a 03 32 01 ff 18 00 4a 02 10 00,d2=[]:
Name UnsupportedUtf8,indexSome0,recordSome0,offset10. Changing Class tail to1800
only => empty success (bad protobuf string bytes unselected). Separate d2 lone
surrogate, NUL and empty-name variants retain [55296], [0], [] exactly; name
canonicality and semantic identifier validity are intentionally outside scope.
Duplicate scalar and wrong-wire-name checks reuse existing literal header fixtures;
no names/flags derived from the new implementation as expected output.

Unit.class integration MUST call parse_class -> annotations::decode -> metadata::
decode -> NEW class_member_names::decode, rather than repeat existing manual
traversal. Existing licensed Unit.class781 bytes SHA256
896588212a98f36148f2fbe1ea2878d6ba23c6376c6346c765cec5ac0c5716ce.
Expect ONE Function ordinal4, field37..49/body39..49,name UTF16
[116,111,83,116,114,105,110,103],FSome65558/ONone/GNone/SNone. Genuine Function
body literal10031a02300448968004; Class19..52. Unit Class fields flags ordinal0,
fq ordinal1, supertype ordinal2, constructor ordinal3, Function ordinal4,
extension ordinal5. No new compiler/JVM/oracle/artifact download.

Every byte cut of MIXED n0..43 is frozen here. Envelope Wire Truncated at n for
n=[0,1,2,3,4]. n5=>Class MissingFqName5. Class Wire Truncated at n for
n=[6,8,9,10,11,12,13,14,16,17,18,19,20,21,22,23,24,25,26,28,29,30,32,33,35,36,
38,40,41,42]. Success cuts: n7=>[]; n15=>first record; n27=>first2;
n31,34,37,39,43=>all3. Every successful record retains the above absolute spans
and exact flag/name values. Complete field-boundary prefixes may succeed without
claiming completeness of an unframed message. MINIMAL n0=>Wire Truncated0;
n1=>Class MissingFqName1; n2=>Class Wire Truncated2; n3=>[].

Finite adversarial tests: these every-byte cuts, selected low32/nonminimal/negative
indices and flags, wrong-wire member/names,
malformed outer tails and selected bodies, opaque nested bodies, aggregate budgets
across duplicate members and UTF16 names, bounded deterministic short-input probes.
No allocation from declared lengths. usize::MAX clamp checks reuse hard byte/record
limits; do not claim finite probes prove all-input/RSS/CPU/stack/JVM compatibility.

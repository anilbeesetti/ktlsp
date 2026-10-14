# Independent Type-header literal freeze, v1 — before source/runtime

Manually derived from pinned schema/generated sources and existing wire rules;
no decoder/compiler/encoder/oracle executed. Every success retains entire input
raw pointer/bytes identity, including field-boundary prefixes, duplicates,
wrong-wire/unknown fields. Unlisted scalar fields are None. All default outcomes
use wire::Limits::default(); errors are wire ErrorKind at Type-relative offset.

## Three eventual hex files

| file | literal bytes | exact selected values | N/F/work |
| --- | --- | --- | --- |
| selected.hex | 08 00 18 00 20 01 30 04 38 02 40 03 48 05 58 06 60 07 70 08 | flags0, nullable false, capabilities1, class4, parameter2, upper3, parameter_name5, outer6, alias7, abbreviated8, ALL Some | 20/10/40 |
| opaque.hex | 12 01 80 12 00 2a 01 80 2a 00 52 01 80 52 00 6a 01 80 6a 00 a2 06 01 80 a2 06 00 | ALL None; two raw occurrences EACH Argument2/upper5/outer10/abbreviated13/Annotation100 | 27/10/47 |
| unit_type.hex | 30 04 | class_name Some4 | 2/1/4 |

First two authored project data, third genuine existing licensed Unit.class inline
return Type. Malformed opaque contents80 and empty nested annotations succeed;
that does not establish nested initialization. No nested vector/merged object.

## Scalars, defaults, duplicates and wrong-wire matrix

[] -> all None, succeeds even bytes/records/work0. For EACH selected field number
1/3/4/6/7/8/9/11/12/14 independently, one recognized wire0 value0 -> Some0
(nullable Some false), then wire0 value1 -> Some1 (nullable true). Duplicates
0 then1 then0 -> Some0/false; other selected fields unchanged. Wrong-wire1/2/5
alone -> None; before/after a recognized field never erases or supplies it.
Wrong-wire bytes body80 deliberately malformed, fixed payloads arbitrary bits.
All10 selected are optional; classifier6/7/9/12 together succeed, no winner.

For each integer field, encode selected tag then literal value below:

| literal value varint | expected signed i32 |
| --- | --- |
| 00 / 80 00 | 0 |
| ff ff ff ff 0f | -1 |
| ff ff ff ff ff ff ff ff ff 01 | -1 |
| ff ff ff ff ff ff ff ff ff 7f | -1 |
| 80 80 80 80 08 | -2147483648 |
| ff ff ff ff 07 | 2147483647 |
| 80 80 80 80 10 | 0 |
| 80 80 80 80 80 80 80 80 80 02 | 0 |

For nullable field3 (tag18), values00/80 00 -> false;01/02/ff ff ff ff 0f/
80 80 80 80 08/80 80 80 80 10/ff ff ff ff ff ff ff ff ff 01/
ff ff ff ff ff ff ff ff ff 7f -> true. Literal ten-byte
80 80 80 80 80 80 80 80 80 02 -> false under inherited deterministic low64.
This pair separates full64 Boolean from low32 and literal-byte-nonzero guesses.
Bool duplicate true then false -> Some false, false then true -> Some true.
Nonminimal tag b0 00 value84 00 -> class Some4. Low32 aliased tag
b0 80 80 80 10 value04 -> class Some4. Never zigzag/index validation.

Unknown fields all supported wires: a0 06 01; a1 06 01 02 03 04 05 06 07 08;
a2 06 03 ff 00 0b; a5 06 01 02 03 04; reserved19000 c0 a3 09 01;
maximum536870911 f8 ff ff ff 0f 01 -> all None with exact raw. Unknown100 wire2
does not recursively parse invalid group/tag0/UTF8. Also append each to30 04,
expect class Some4. Nested number5/10/13 duplicates preserved raw, no merge.

## Structural errors, tail precedence

| bytes | exact error/offset |
| --- | --- |
| 30 / 80 | Truncated1 |
| 00 / 03 / 07 | InvalidFieldNumber0 |
| 0b / 0c / 1b / 1c / 33 / 34 | UnsupportedGroup0 |
| 0e / 0f / 1e / 1f / 36 / 37 | InvalidWireType0 |
| 12 02 ff | Truncated3 |
| 0d 01 02 03 | Truncated4 |
| 09 01 02 03 04 05 06 07 | Truncated8 |
| 12 80 80 80 80 08 | NegativeLength1 |
| 12 ff ff ff ff 07 | Truncated6 |
| ten80 | MalformedVarint9 |
| 30 then ten80 | MalformedVarint10 |
| 12 then ten80 | MalformedVarint10 |

For EACH row also prepend30 04, expect same kind offset+2, never early success.
For EACH selected field tag and continuation count1..9: truncated value ->
Truncated supplied length; count10 -> Malformed at tenth continuation. A full
opaque length-delimited body containing these bytes succeeds: body is not parsed.

## Every byte cut, exact partition (52 cuts)

For each eventual file, cut input at n0..=N. T list -> Truncated at n.
Other cuts are success with raw equal that prefix. Lists partition ALL cuts.
- selected length20: T[1,3,5,7,9,11,13,15,17,19]. Success n=0,2,4,6,8,10,
  12,14,16,18,20: fields from first n/2 pairs present with exact table values;
  all later fields None. Empty classifier prefixes are valid.
- opaque length27: T[1,2,4,6,7,9,11,12,14,16,17,19,21,22,23,25,26].
  Success[0,3,5,8,10,13,15,18,20,24,27], all scalars None.
- unit_type length2: T[1]; success0 all None and2 class Some4.
Count21+28+3=52. Success at a field boundary does not prove complete Type graph.

## Exact budgets and first-error offsets

Three files pass exact N/F/work above and each+1. bytes N-1 -> Limit(bytes)0,
even with records0/work0. records F-1 -> Limit(records) at last tag:
selected18, opaque24, unit0. work total-1 -> Limit(work) same last tag(dispatch).
Empty all0 success. Unit30 04: records0/work0 -> records0; records1 work0/1 ->
work0; work2 -> work1; work3 -> work0(dispatch); work4 succeeds.
30 with records1/work2 -> work1; work3 -> Truncated1.
30 04 80 with records1/work4 -> records2; records2/work6 -> work3;
records2/work7 -> Truncated3. 30 04 00 with work6 -> InvalidFieldNumber2.
12 00 with work3 -> work0(dispatch); work4 succeeds ALL None.
12 02 ff with work4 -> work2(short bulk debit2); work5 -> Truncated3.
30 04 12 01 80 with work8 -> work2(dispatch); work9 succeeds class4.
30 04 12 03 ff ff ff with work9 -> work4(bulk3); work10 -> work2(dispatch);
work11 succeeds class4. No owned-capacity/node/depth charge for any body.
12 80 08 +1024ff -> bytes1027/fields1/work1029, all None; work1028 ->work0
at final dispatch, exact1029 succeeds, no body allocation or nested validation.

usize::MAX limits clamp. MAX_BYTES+1 bytes -> bytes0 before records/work;
65,535 copies30 00 -> N131070/F65535/W262140, class Some0 succeeds;
one extra30 00 -> records131070 even unlimited caller limits. Hard work64Mi
is unreachable on successful flat scans bounded by bytes16Mi/fields65,535;
no false hard-work witness. Finite probes under bytes32/records8/work256:
all a,b0..255, decode[a,b] and[30,04,a,b] twice; each a repeated n0..31 twice.
Assert identical complete Results, raw identity on success, offset<=len on error.
139264 inputs/278528 calls, future original native CI only. No timing/RSS claim.

## Real consumer, frozen before execution

Unmodified existing crates/ktlsp-binary/fixtures/t017c2/Unit.class:781bytes,
SHA256896588212a98f36148f2fbe1ea2878d6ba23c6376c6346c765cec5ac0c5716ce,
kotlin-stdlib2.3.20 JAR SHA256
0ae12504a5040ebaf37703908483420d1a5624dd1d93f357665f8c77c848a01e;
distribution2.3.20-release-208, compiler provenance UNVERIFIED. No regeneration.
Existing independent Function EXPECTATIONS/UNIT_EXTRACTION.json establish d1
52 bytes, Class19..52, Function field37..49/body39..49, Function-relative return
field2..6, tag/length1a02, inline Type body4..6=d1[43..45]=30 04.

Future test invokes actual parse_class -> annotations::decode -> metadata::decode;
assert k1/mv[2,3,0]; payload::decode -> actual class_header::decode(payload),
assert flagsSome326/fq_name0, Class raw=&d1[19..52]. Existing separate
wire::decode(full d1) selects sole payload field9, verifies span37..49 and literal
body10 03 1a 02 30 04 48 96 80 04. Assert selected body is within decoded Class
raw at Class-relative20..30. Invoke function_header::decode(actual Function body),
assert name3/flagsSome65558/old_flagsNone and full raw identity. Use fixed fixture
offsets only AFTER asserting complete literal Function and raw identity: assert
function.raw[2..4]=1a02, get function.raw[4..6], assert pointer=d1[43..45] and
literal unit_type.hex. This is fixture-specific extraction, no general walker.
Type decode bytes2/records1/work4 -> class_nameSome4, ALL other scalars None.
Separate Resolver::new(framed.string_table,header.d2) -> get_string(4) gives exact
UTF16[107,111,116,108,105,110,47,83,116,114,105,110,103] = kotlin/String.
Reason: compact records predefined2,predefined0,range2,predefined14,empty;
index4 selects predefined14. Name expectation from existing resolver source/table,
not raw d2[4], not implementation-produced expected value. Every API budget
separate; this test changes no production caller or table API.

Notice copies match metadata_wire LICENSE.txt/NOTICE.txt/PROTOBUF_LICENSE.txt.
Existing artifact notices/provenance stay authoritative. No fresh JVM/compiler,
product/parser/server/test/oracle/benchmark execution in preparation.

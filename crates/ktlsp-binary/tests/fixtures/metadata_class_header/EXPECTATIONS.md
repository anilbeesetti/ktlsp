# Independently authored Class-header expectations before source/runtime

All bytes/expectations manually derived from pinned schema/generated parser and
existing wire charging rules. No decoder/encoder/compiler execution. Offsets are
relative to unframed supplied Class. W(X) means ErrorKind::Wire(X), Missing means
MissingFqName. Every success retains pointer-identical raw bytes, including ignored
wrongwire/unknown/duplicates; no field collection or copy. flags F means Option value;
F=None has effective6 without becoming Some6. fq_name Q remains signed index data.

## Authored baseline files

- minimal.hex `18 00`: F=None,Q=0; bytes2,fields1,work4.
- explicit.hex `08 06 18 00`: F=Some6,Q=0; bytes4,fields2,work8.
- duplicates.hex `08 01 18 01 08 06 18 00 0a 01 ff 1a 01 80`:
  F=Some6,Q=0; last recognized flags tag4/fq tag6; wrongwire flags tag8 and
  fq tag11 do not clear/replace scalars. bytes14,fields6,work26.
- opaque.hex `18 00 2a 00 4a 01 80 12 01 80 a2 06 01 00`:
  F=None,Q=0. Empty TypeParameter, malformed Function, malformed packed supertype
  ids and unknown extension bytes remain opaque. bytes14,fields5,work24.
  This is deliberately NOT full Kotlin Class initialization/schema validation.

## Scalars, presence, duplicates and wrongwire

| bytes | result at default limits |
| --- | --- |
| empty | Missing0, also with records0/work0/bytes0 |
| 08 06 | Missing2 (flags alone cannot supply fq_name) |
| 18 00 | F=None,Q=0 (present zero differs from absent) |
| 08 00 18 00 | F=Some0,Q=0 |
| 08 06 18 00 | F=Some6,Q=0 |
| 18 ac 02 08 02 | F=Some2,Q=300; reversed order |
| 18 01 18 02 | F=None,Q=2 |
| 08 01 08 02 18 00 | F=Some2,Q=0 |
| 1a 00 | Missing2, wrongwire alone |
| 1a 01 80 18 00 | F=None,Q=0; wrongwire body not parsed |
| 18 01 1a 00 | F=None,Q=1; wrongwire does not clear |
| 0a 00 18 00 | F=None,Q=0 |
| 08 01 0a 00 18 00 | F=Some1,Q=0 |
| 1d 00 00 00 00 18 00 | F=None,Q=0; wrongwire fixed32 |
| 19 00 00 00 00 00 00 00 00 18 00 | F=None,Q=0; wrongwire fixed64 |
| 98 00 80 00 | F=None,Q=0; nonminimal tag/value |
| 98 80 80 80 10 00 | F=None,Q=0; low32 tag alias24 |
| 18 ff ff ff ff 0f | F=None,Q=-1; five-byte signed low32 |
| 18 ff ff ff ff ff ff ff ff ff 01 | F=None,Q=-1; ten-byte negative |
| 18 ff ff ff ff ff ff ff ff ff 7f | F=None,Q=-1; raw64 truncation rule |
| 08 80 80 80 80 08 18 00 | F=Some(i32::MIN),Q=0 |
| 18 ff ff ff ff 07 | F=None,Q=i32::MAX |
| 18 80 80 80 80 10 | F=None,Q=0; high32 bits ignored |
| 18 80 80 80 80 80 80 80 80 80 02 | F=None,Q=0; tenth payload high bits ignored |

Repeat low32/negative/nonminimal values for BOTH selected fields in eventual tests.
Unknown field19000 (`c0 a3 09 01`) and max536870911 (`f8 ff ff ff 0f 01`)
with `18 00` succeed preserving raw. All opaque wire0/1/2/5 forms require coverage;
unknown/member tags never become inferred declarations. Test zero-length opaque
body, arbitrary ff/00/group bytes INSIDE it, and long body tight-work debit.

## Structural errors and required-field precedence

| bytes | default error |
| --- | --- |
| 18 | W(Truncated)1 |
| 80 | W(Truncated)1 |
| 00 | W(InvalidFieldNumber)0 |
| 03 or 07 | W(InvalidFieldNumber)0, before group/wire |
| 0b or 0c or 1b or 1c | W(UnsupportedGroup)0 |
| 0e or 0f | W(InvalidWireType)0 |
| 18 00 00 | W(InvalidFieldNumber)2 |
| 18 00 0b | W(UnsupportedGroup)2 |
| 18 00 0f | W(InvalidWireType)2 |
| 18 00 80 | W(Truncated)3; never early success |
| 08 06 80 | W(Truncated)3; before Missing |
| 1a 02 ff | W(Truncated)3 |
| 0d 01 02 03 | W(Truncated)4, fixed32 short |
| 09 01 02 03 04 05 06 07 | W(Truncated)8, fixed64 short |
| 1a 80 80 80 80 08 | W(NegativeLength)1 |
| 1a ff ff ff ff 07 | W(Truncated)6; no declared-length allocation |
| 18 followed ten80 | W(MalformedVarint)10 |
| ten80 | W(MalformedVarint)9 |
| 18 00 1a followed ten80 | W(MalformedVarint)12 |

Every-byte cuts REQUIRED:
minimal n0=>Missing0,n1=>Truncated1,n2=>success.
explicit n0=>Missing0,n1=>Truncated1,n2=>Missing2,n3=>Truncated3,n4=>success.
duplicates n0=>Missing0; n1,3,5,7,9,10,12,13=>Truncated n;
n2=>Missing2; n4=>F1/Q1; n6=>F6/Q1; n8,11,14=>F6/Q0.
opaque n0=>Missing0; n1,3,5,6,8,9,11,12,13=>Truncated n;
n2,4,7,10,14=>FNone/Q0 success (complete opaque fields only).
Every cut reports its own sliced raw on success. Nonminimal ten-byte sequences
cuts1..9 continuation bytes=>Truncated at cut; tenth continuation=>Malformed at9.
Append valid fields then every structural-error tail to prove scan cannot stop early.

## Exact bounds and error order, manual equations

Complete scan work=N+2F, F records; bytes ceiling includes all opaque bytes.
No envelope/root/node/owned/depth charges. Exact scalar/fixture totals above pass;
one-higher passes; one-lower bytes fails W(Limit(bytes))0 before every other error.
For minimal bytes2/records1/work4: records0=>records0, work0=>work0(record),
work1=>work0(tag), work2=>work1(value), work3=>work0(dispatch), work4=>success.
Malformed `18`: work2=>work1,work3=>Truncated1; records0=>records0.
`18 00 80`: records1=>records2; work6=>work3,work7=>Truncated3.
`18 00 00`: work6=>InvalidFieldNumber2 (no dispatch for invalid field).
`1a 00`: work3=>work0(dispatch),work4=>Missing2.
`1a 02 ff`: work4=>work2(bulk short debit2),work5=>Truncated3.
`18 00 2a 01 80`: work8=>work2(dispatch),work9=>FNone/Q0.
`18 00 2a 03 ff ff ff`: work9=>work4(payload bulk debit3),work10=>work2(dispatch),
work11=>success. These distinguish bulk start from first missing/dispatch offset.
duplicates records5=>records11;work25=>work11(final dispatch).
opaque records4=>records10;work23=>work10(final dispatch).

Hard clamps: supplied limits usize::MAX clamp to defaults; MAX_BYTES+1 bytes fails0
before records/work/structure. Default records65,535 accepts `18 00` +65,534 copies
`08 00` (work262,140,bytes131,070); next field fails records at131,070.
All are scalars/no proportional untrusted allocation. For standalone work ceiling,
caller cannot raise64Mi; under16MiB bytes and65,535 fields maximum successful
work<=16MiB+131,070, so work hard ceiling cannot be independently reached on a
successful flat scan. Test clamping via byte/record ceiling and explicit small work
thresholds; do not claim an unreachable work-hard-ceiling runtime witness.

Future deterministic probes: all a,b0..255 decode [a,b] and [18,00,a,b] twice with
bytes32/records8/work256; compare complete Results and raw pointer/bounds on success.
For n0..31 repeat each a with same limits. Add selected truncated-varint, large
length/negative/group tails. Native tests only after source/CI admission; finite
probes do not prove all-input compatibility, CPU, stack or RSS limits.

## Existing licensed data-only Unit integration

Reuse crates/ktlsp-binary/fixtures/t017c2/Unit.class unchanged, SHA256
896588212a98f36148f2fbe1ea2878d6ba23c6376c6346c765cec5ac0c5716ce,size781.
Its provenance.json names org.jetbrains.kotlin:kotlin-stdlib:2.3.20 JAR SHA256
0ae12504a5040ebaf37703908483420d1a5624dd1d93f357665f8c77c848a01e, existing
Apache LICENSE.txt/NOTICE.txt hashes. No compiler provenance inferred/no tool run.
Existing tests/metadata_header.rs independently transcribed decoded d1, and
tests/metadata_wire.rs transcribed payload19..52 fields. Reuse as baseline data:
header kind1,mv[2,3,0]; payload::decode table1..19,Class19..52. Supplied Class raw:
`08 c6 02 18 00 32 02 30 01 42 07 08 42 a2 06 02 08 02 4a 0a 10 03 1a 02
30 04 48 96 80 04 a8 06 05`. F=Some326,Q=0,fields6,bytes33,work45.
Byte offsets relative Class: selected flags0..3,fq3..5; opaque fields5..9,
9..18,18..30,30..33. raw identity exactly &decoded_d1[19..52]. Separate caller
may resolve Q0 to UTF16 `kotlin/Unit` using existing resolver, with independent
budget; class decoder does no such lookup. Negative Q synthetic success must NOT
index any table here. Payload's old authored class.hex tail0801 has no fq_name:
this new decoder returns Missing2 while payload::decode remains successful.

No precompiled artifact copied/added. Future fixture notices copy existing
tests/fixtures/metadata_wire/{LICENSE.txt,NOTICE.txt,PROTOBUF_LICENSE.txt} unchanged.
New hex authored project data; schema/generated source is reference only.

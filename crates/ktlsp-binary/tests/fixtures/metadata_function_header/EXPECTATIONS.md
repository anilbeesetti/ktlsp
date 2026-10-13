# Independent Function-header fixture freeze before implementation

All expected Results/offsets manually derived from pinned schema/generated Java
and frozen wire rules, not an implementation/oracle output. F=flags Option<i32>,
O=old_flags Option<i32>, N=name signed int32. W(X) wraps wire ErrorKind. Missing
means MissingName. Every success must retain pointer-identical entire supplied raw
slice, including field-boundary prefixes, duplicates and wrong-wire/unknown fields.
No tests have run; these are future acceptance assertions.

## Five concrete files

| file | exact bytes | F/O/N | bytes/fields/work |
| --- | --- | --- | --- |
| minimal.hex | 10 00 | None/None/0 | 2/1/4 |
| explicit.hex | 08 06 10 00 48 06 | Some6/Some6/0 | 6/3/12 |
| duplicates.hex | 08 01 48 02 10 01 08 06 48 00 10 00 0a 01 ff 4a 01 80 12 01 00 | Some0/Some6/0 | 21/9/39 |
| opaque.hex | 10 00 1a 01 80 22 00 32 00 5a 01 80 a2 06 01 00 | None/None/0 | 16/6/28 |
| unit_function.hex | 10 03 1a 02 30 04 48 96 80 04 | Some65558/None/3 | 10/3/16 |

First four are authored project data. Fifth is genuine unmodified Function bytes
from existing licensed Unit.class, independently extracted as data below.
duplicates has recognized scalar tag starts0/2/4/6/8/10 and wrong-wire old/flags/name
starts12/15/18. Wrong-wire bodies must not erase the prior values. opaque contains
malformed nested return_type, empty TypeParameter/ValueParameter lacking required
fields, malformed packed context-receiver IDs and unknown extension100. It MUST
succeed as selected header while NOT establishing protobuf Function initialization.

## Presence, independent flags, order and wrong wire

| bytes | default result F/O/N or error |
| --- | --- |
| empty | Missing0; also bytes0/records0/work0 |
| 08 06 | Missing2 |
| 48 06 | Missing2 |
| 08 06 48 06 | Missing4 |
| 12 00 | Missing2 |
| 10 00 | None/None/0 |
| 08 00 10 00 | None/Some0/0 |
| 48 00 10 00 | Some0/None/0 |
| 08 06 10 00 | None/Some6/0 |
| 48 06 10 00 | Some6/None/0 |
| 48 02 08 01 10 ac 02 | Some2/Some1/300 |
| 08 01 10 00 48 02 | Some2/Some1/0 |
| 48 02 10 00 08 01 | Some2/Some1/0 |
| 10 01 10 02 | None/None/2 |
| 08 01 08 02 10 00 | None/Some2/0 |
| 48 01 48 02 10 00 | Some2/None/0 |
| 0a 01 80 10 00 | None/None/0 |
| 4a 01 80 10 00 | None/None/0 |
| 12 01 80 10 00 | None/None/0 |
| 08 01 0a 00 10 00 | None/Some1/0 |
| 48 01 4a 00 10 00 | Some1/None/0 |
| 10 01 12 00 | None/None/1 |
| 15 00 00 00 00 10 00 | None/None/0 |
| 11 00 00 00 00 00 00 00 00 10 00 | None/None/0 |
| 90 00 80 00 | None/None/0; nonminimal tag/value |
| 90 80 80 80 10 00 | None/None/0; low32 tag alias16 |

Future tests cover wrong-wire1/2/5 and recognized0 independently for ALL THREE
selected field numbers. Wrong-wire alone supplies no scalar presence, wrong-wire
before/after a recognized scalar does not change it, and cross-field order never
changes another selected scalar. No conversion of O bits into F or vice versa.

## Signed/nonminimal values (repeat independently for all three fields)

For each row V below: `10 V` => FNone/ONone/Nexpected;
`08 V 10 00` => FNone/OSome(expected)/N0;
`48 V 10 00` => FSome(expected)/ONone/N0. Repeat duplicates to prove last wins
for negative as well as positive values, without affecting the other two fields.

| V | expected int32 |
| --- | --- |
| ff ff ff ff 0f | -1 |
| ff ff ff ff ff ff ff ff ff 01 | -1 |
| ff ff ff ff ff ff ff ff ff 7f | -1 |
| 80 80 80 80 08 | -2147483648 |
| ff ff ff ff 07 | 2147483647 |
| 80 80 80 80 10 | 0 |
| 80 80 80 80 80 80 80 80 80 02 | 0 |
| 80 00 | 0 |

Negative indices succeed as data; no resolver/indexing in this decoder.
Raw64 slow-path caveat in CONTRACT.md applies; no universal JVM fast-path claim.

## Unknown/opaque forms and large bodies

Append each following to `10 00`, expect FNone/ONone/N0 with raw identity:
`c0 a3 09 01` reserved19000; `f8 ff ff ff 0f 01` maximum536870911;
`a0 06 01`, `a1 06 01 02 03 04 05 06 07 08`, `a2 06 00`,
`a2 06 03 ff 00 0b`, `a5 06 01 02 03 04` (unknown100 all supported wire forms).
Nested group/tag0/bad-wire/invalidUTF8 bytes inside an opaque body are not errors.
`10 00 32 80 08` +1024ff: bytes1029/fields2/work1033; work1032 fails
W(Limit(work))2 at final dispatch, work1033 succeeds. Payload bytes are debited
without parsing or allocating their declared body.

## Structural wire failures precede present AND absent name

| bytes | default error/offset |
| --- | --- |
| 10 | W(Truncated)1 |
| 80 | W(Truncated)1 |
| 00, 03, 07 (separate cases) | W(InvalidFieldNumber)0 |
| 0b,0c,13,14,4b,4c (separate cases) | W(UnsupportedGroup)0 |
| 0e,0f,16,17,4e,4f (separate cases) | W(InvalidWireType)0 |
| 12 02 ff | W(Truncated)3 |
| 0d 01 02 03 | W(Truncated)4 |
| 09 01 02 03 04 05 06 07 | W(Truncated)8 |
| 12 80 80 80 80 08 | W(NegativeLength)1 |
| 12 ff ff ff ff 07 | W(Truncated)6 |
| 10 followed ten80 | W(MalformedVarint)10 |
| ten80 | W(MalformedVarint)9 |
| 12 followed ten80 | W(MalformedVarint)10 |

For EVERY row, prepend independently `10 00` (present), `08 06` (absent),
`48 06` (absent), or `08 06 10 00 48 06` (all present): SAME wire kind and
offset+prefix.len(). Never early success or Missing over later structural error.
Also truncate each selected tag8/16/72 value after1..9 continuation bytes:
Truncated at supplied length; ten continuation bytes=>Malformed at tenth byte.

## EVERY byte cut of all five files (60 exact cases)

For each fixture, cut at n0..=len. T list => W(Truncated) at n. M list => Missing
at n. S list gives exact successful (n,F,O,N); raw is that prefix only.
Lists partition every n once (including empty and full input).

- minimal length2: T[1], M[0], S[(2,None,None,0)].
- explicit length6: T[1,3,5], M[0,2],
  S[(4,None,Some6,0),(6,Some6,Some6,0)].
- duplicates length21: T[1,3,5,7,9,11,13,14,16,17,19,20], M[0,2,4],
  S[(6,Some2,Some1,1),(8,Some2,Some6,1),(10,Some0,Some6,1),
  (12,Some0,Some6,0),(15,Some0,Some6,0),(18,Some0,Some6,0),(21,Some0,Some6,0)].
- opaque length16: T[1,3,4,6,8,10,11,13,14,15], M[0],
  S[(2,None,None,0),(5,None,None,0),(7,None,None,0),(9,None,None,0),
  (12,None,None,0),(16,None,None,0)].
- unit_function length10: T[1,3,4,5,7,8,9], M[0],
  S[(2,None,None,3),(6,None,None,3),(10,Some65558,None,3)].

Correction to aggregate counting is mechanical only: lengths2+6+21+16+10 plus
five empty cuts =60. Complete field-boundary prefixes may succeed; this does not
establish full Function validity or identify truncation of an unframed container.

## Exact limits and precedence

All five files pass exact bytes/records/work totals above and each+1. bytes len-1
fails W(Limit(bytes))0 even with records0/work0. Each fixture records F-1 fails at
last tag start: minimal0, explicit4, duplicates18, opaque12, genuine6. Work total-1
fails dispatch at SAME last tag start. No envelope/root record.

For `10 00`, records1 and work0/1=>W(Limit(work))0, work2=>W(Limit(work))1,
work3=>W(Limit(work))0 (dispatch), work4=>success. records0/work0=>records0.
Empty input all0=>Missing0. `10`: records1/work2=>work1; work3=>Truncated1.
`10 00 80`: records1/work4=>records2; records2/work6=>work3;
records2/work7=>Truncated3. `10 00 00` work6=>InvalidFieldNumber2.
`12 00` work3=>work0(dispatch); work4=>Missing2.
`12 02 ff` work4=>work2(bulk short debit2); work5=>Truncated3.
`10 00 32 01 80` work8=>work2(dispatch), work9=>success.
`10 00 32 03 ff ff ff` work9=>work4(bulk debit3), work10=>work2(dispatch),
work11=>success. These distinguish cursor/bulk-start/tag-start offsets.

usize::MAX limits clamp, not disable. MAX_BYTES+1 input fails bytes0 before any
wire/records/work check. `10 00`+65,534 copies `08 00` has bytes131070/F65535/
work262140, succeeds OSome0/FNone/N0. Appending one `48 00` fails records131070
under usize::MAX limits. At maximum16MiB/65535 fields a flat successful scan
costs<=16MiB+131070, so the64Mi work hard ceiling is unreachable independently on
success. Do not invent a hard-work-ceiling runtime witness; use small-work exact
thresholds and byte/record clamp checks. Large data allocated in future tests only
after runtime admission, never on unchecked declared lengths in product code.

## Deterministic hostile probes (future native CI only)

For all a,b0..255 decode [a,b] and [10,00,a,b] twice under bytes32/records8/work256.
For every a repeat length n0..31. Assert identical full Results, success raw pointer/
bytes identity, and all error offsets<=input.len(). This defines139264 input cases
and278528 decode calls (131072 paired inputs+8192 repeats). Known exact fixtures,
cuts/limits/errors above supply meaningful behavior assertions beyond determinism.
No timing/stack/RSS/full protobuf/all-input correctness claim from finite probes.

## Genuine licensed Function and separately bounded composition

Existing crates/ktlsp-binary/fixtures/t017c2/Unit.class, unchanged781 bytes, SHA256
896588212a98f36148f2fbe1ea2878d6ba23c6376c6346c765cec5ac0c5716ce.
Original provenance.json: org.jetbrains.kotlin:kotlin-stdlib:2.3.20, JAR SHA256
0ae12504a5040ebaf37703908483420d1a5624dd1d93f357665f8c77c848a01e,
distribution2.3.20-release-208, compiler provenance UNVERIFIED. No new download,
compiler/JVM/oracle/Gradle/Maven or product execution. extract_unit.py reads the
actual constant-pool Utf8 entry31 classfile[290..352], decodes its fixture-local
one/two-byte MUTF8 units, verifies initial NUL+low8 d1 and entire independent52-byte
transcription, then literal d1[37..39]=4a0a and body[39..49]. UNIT_EXTRACTION.json
records actual hashes/spans; Python data extraction is not native decode evidence.

Future integration: existing parse_class/annotations/header as separately bounded
operations -> assert supported kind1/mv[2,3,0] -> payload::decode -> Class payload
d1[19..52]. Use existing wire::decode in a separately bounded call on full d1, select
the one field9 Bytes and assert span37..49/body39..49/literal bytes; pass that body
to function_header with exact bytes10/records3/work16. Expect FSome65558/ONone/N3,
pointer raw=&d1[39..49]. Separate Resolver::new and get_string(3) => UTF16
[116,111,83,116,114,105,110,103] (toString), as existing header transcription.
No generic container walker, new helper/public API or combined aggregate budget.
No reinterpretation of full Class bytes as a Function body. Synthesized Package
envelope not needed; genuine container composition covers this selected header.

Copy unchanged metadata_wire LICENSE.txt/NOTICE.txt/PROTOBUF_LICENSE.txt into the
future fixture directory. Existing third-party provenance remains authoritative;
new authored bytes are project data, genuine snippet derives from existing artifact.
Never ship generated Java sources or upstream runtime. #68/#18 remain OPEN.

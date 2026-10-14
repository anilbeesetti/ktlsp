# Literal preimplementation freeze — revision 2

No decoder/encoder under test, Rust/native/JVM/compiler/oracle executed. Each row
is ONE complete caller-selected parent bytes. Signed numbers use low32. `S` is
sizeof(SignatureBody), C(n) logical next_power_of_two capacity; spans parent-relative.
Raw equals complete input; all body raw slices pointer-identical to supplied raw.

|ID|parent hex|merged(name,desc)|body spans|records|work|owned|
|---|---|---|---|---|---|---|
|V0|empty|absent|[]|0|0|0|
|V1|a2 06 00|(None,None)|[3..3]|1|5+S|S|
|V2|a2 06 04 08 00 10 00|(Some0,Some0)|[3..7]|3|17+S|S|
|V3|a2 06 04 08 07 10 08|(Some7,Some8)|[3..7]|3|17+S|S|
|V4|a2 06 02 08 07 a2 06 02 10 08 a2 06 00|(Some7,Some8)|[3..5,8..10,13..13]|5|27+4S|4S|
|V5|a2 06 06 08 01 08 02 10 03 a2 06 02 08 04|(Some4,Some3)|[3..9,12..14]|6|34+2S|2S|
|V6|a0 06 07|absent|[]|1|5|0|
|V7|a2 06 04 0a 00 10 00|(None,Some0)|[3..7]|3|17+S|S|
|V8|a2 06 06 08 07 10 08 18 63|(Some7,Some8)|[3..9]|4|23+S|S|
|V9|a2 06 04 08 07 12 00 a0 06 01|(Some7,None)|[3..7]|4|22+S|S|
|V10|aa 06 01 00|absent|[]|1|6|0|
|V11|a2 06 06 08 ff ff ff ff 0f|(Some-1,None)|[3..9]|2|19+S|S|
|V12|a2 06 06 08 80 80 80 80 08|(Some-2147483648,None)|[3..9]|2|19+S|S|
|V13|a2 06 06 08 80 80 80 80 10|(Some0,None)|[3..9]|2|19+S|S|
|V14|a2 06 03 08 87 00|(Some7,None)|[3..6]|2|13+S|S|
|V15|a2 06 00 a2 06 00 a2 06 00 a2 06 00 a2 06 00|(None,None)|[3..3,6..6,9..9,12..12,15..15]|5|25+8S|8S|
|V16|a2 06 02 0a 00|(None,None)|[3..5]|2|11+S|S|
|V17|a2 06 03 1a 01 00|(None,None)|[3..6]|2|13+S|S|
|V18|a2 06 00 10 00|(None,None)|[3..3]|2|9+S|S|
|V19|a2 06 02 08 07 a2 06 02 0a 00|(Some7,None)|[3..5,8..10]|4|22+2S|2S|
|V20|a2 86 00 84 00 08 07 10 08|(Some7,Some8)|[5..9]|3|19+S|S|
|V21|a2 86 80 80 10 04 08 07 10 08|(Some7,Some8)|[6..10]|3|20+S|S|
|V22|a2 06 0b 08 ff ff ff ff ff ff ff ff ff 01|(Some-1,None)|[3..14]|2|29+S|S|
|P1|08 46 12 06 10 05 1a 02 38 00 12 06 10 06 1a 02 38 01 a2 06 04 08 07 10 08|(Some7,Some8)|[21..25]|6|41+S|S|

V8 field3=99 is unknown; JvmMethodSignature has NO enum. V17 wrong-wire child3
bytes00 opaque despite malformed nested wire. V10 parent101 bytes00 opaque.
V18 parent Function.name0 ignored by this API; V0 succeeds even as uninitialized
Function data. V4 proves later absent scalar and empty message cannot erase data;
V5 proves last present scalar wins across messages without clearing desc.
V19 later wrong-wire name preserves earlier name. V20 accepts nonminimal tag/length;
V21 tag retains low32, V22 ten-byte negative scalar retains signedlow32.
V19 records3=>Limit(records)@8,owned2S-1=>Limit(owned_bytes)@5,
work22+2S-1=>Limit(work)@5; other limits default.

V3 every-prefix freeze: cut0=>success absent; cuts1,2,3,4,5,6=>Truncated@cut;
cut7=>V3 success. Parent must complete outer body framing before child scan.
V4 prefix freeze: cut0 absent;1..4 Truncated@cut;5 success(Some7,None),onebody;
6..9 Truncated@cut;10 success(Some7,Some8),twobodies;11,12 Truncated@cut;
13 full V4. P1 cuts0,2,10,18 succeed absent with no bodies;cut25 full P1;
all other cuts1..24=>Truncated@cut. V15 cuts0,3,6,9,12,15 succeed
with that many complete empty occurrences (cut/3);all other cuts Truncated@cut.
Complete prefixes are independently valid messages, not partial output.

Malformed rows use default budgets, exact error kind and PARENT offset:
|ID|hex|error|
|---|---|---|
|M1|00|InvalidFieldNumber@0|
|M2|0e|InvalidWireType@0|
|M3|0b|UnsupportedGroup@0|
|M4|a2 06 01 00|InvalidFieldNumber@3|
|M5|a2 06 01 08|Truncated@4|
|M6|a2 06 02 08 80|Truncated@5|
|M7|a2 06 01 0e|InvalidWireType@3|
|M8|a2 06 01 0b|UnsupportedGroup@3|
|M9|a2 06 03 0a 02 00 10 01|Truncated@6|
|M10|a2 06 00 00|InvalidFieldNumber@3|
|M11|a2 06 02 08 07 a2 06 01 08|Truncated@9|
|M12|a2 06 ff ff ff ff 0f|NegativeLength@2|
|M13|a2 06 06 0a ff ff ff ff 0f|NegativeLength@4|
|M14|a2 06 0b 08 80 80 80 80 80 80 80 80 80 80|MalformedVarint@13|

M9 selected body's incomplete bytes field may not consume following parent10 01.
M11 proves a valid earlier body cannot hide malformed later body. M13 wrong-wire
name's invalid length still fails wire. Unknown bytes bodies are opaque, but their
outer framing is validated. Append is after child validation, so M4 with owned0
still returns InvalidFieldNumber@3 (provided work/records sufficient).

Resource rows independently tighten one limit (others default) unless specified:
- For every valid row, exact bytes/records/work/owned from table succeeds; raising
  all four to usize::MAX also succeeds under clamping. all0 empty succeeds.
- V3 bytes6=>Limit(bytes)@0; records0=>Limit(records)@0;
  records1=>Limit(records)@3;records2=>Limit(records)@5.
- V3 work0=>Limit(work)@0 (record-work);work1=>Limit(work)@0(tagbyte);
  work3=>Limit(work)@2(lengthbyte);work7=>Limit(work)@3(bulkbody);
  work8=>Limit(work)@0(parentdispatch);work9=>Limit(work)@3(childrecordwork);
  work12=>Limit(work)@3(childdispatch);work13=>Limit(work)@5(childrecordwork);
  work16=>Limit(work)@5(childdispatch);work17+S-1=>Limit(work)@0(capacitywork).
- V3 ownedS-1=>Limit(owned_bytes)@0 after child; work17 and ownedS-1 gives
  Limit(owned_bytes)@0 before capacity-work. ownedS and work17+S succeed.
- V4 records4=>Limit(records)@10 (third parent, despite empty body);
  owned2S=>Limit(owned_bytes)@10 (third push);owned4S-1 same;
  work27+4S-1=>Limit(work)@10 (third capacity charge); exact4S succeeds.
- V5 aggregate records5=>Limit(records)@12(last child); later scalar overwrite is
  still scanned/charged. Exact34+2S succeeds; onebelow=>Limit(work)@9(second push).
- V15 records4=>Limit(records)@12;owned4S=>Limit(owned_bytes)@12;
  work25+8S-1=>Limit(work)@12. All five empty bodies retained/charged.
- M4 bytes3=>Limit(bytes)@0 before structural error; records1=>Limit(records)@3
  before child invalid field; owned0=>InvalidFieldNumber@3, not owned failure.
- M5 work7=>Limit(work)@3(childtag);work8=>Limit(work)@4 before truncated
  child value;work9=>Truncated@4.
- V10 exact work6,records1,owned0 succeeds despite opaque00 payload.
- P1 exact25/6/41+S/S succeeds; separate parent/header/table/resolver budgets
  remain separate. bytes24=>Limit(bytes)@0;records5=>Limit(records)@23;
  work41+S-1=>Limit(work)@18;ownedS-1=>Limit(owned_bytes)@18.

Future bounded-hostile tests: deterministic all65536 two-byte inputs and uniform
length0..31 bytes, limits32bytes/8records/256work/1024owned; deterministic result,
no panic, errors<=input length, raw/pointers/spans valid, signatureNone iff bodies
empty. This finite corpus is not all-input proof. Allocation failure static only.

Hard-cap probes (planned literals, not executed):
- vec![0;MAX_BYTES+1], all limits usize::MAX => Limit(bytes)@0 before field0.
- Repeat literal a2 06 00 exactly65535 times (N196605), all limitsusize::MAX:
  success Some(None,None),65535 ordered empty bodies at[3i+3..3i+3] for
  i0..65534;records65535,work327675+65536S,owned65536S.
- Same literal65536times => Limit(records)@196605, despite raised records.
- Assert Default fields exactly MAX_BYTES/MAX_RECORDS/MAX_VALIDATION_UNITS/
  MAX_BYTES. Existing Budget clamps work/owned statically; these shallow-input
  fixtures do not dynamically reach64Mi work or16MiB owned ceilings. No dynamic
  work/owned hard-ceiling proof is claimed.

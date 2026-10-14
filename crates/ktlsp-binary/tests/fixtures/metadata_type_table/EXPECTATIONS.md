# Independently frozen TypeTable literals and arithmetic

Preparation before decoder source. All literals independently authored from pinned
schema and unchanged wire code, no first-decoder output. Let S=size_of::<&[u8]>()
(16 on64bit,8 on32bit); capacity C is next_power_of_two(k), C0=0. No source runtime.

## Complete fixtures

|fixture|literal bytes N|fields F|types body ranges|first_nullable|C|work N+2F+C*S|last field|
|--|--:|--:|--|--|--:|--|--:|
|empty|0|0|[]|None|0|0|none|
|ordered|11|4|[2..2,4..6,8..9]|Some(1)|4|19+4S|9|
|duplicates|15|5|[15..15]|Some(2)|1|25+S|13|
|opaque|24|5|[23..24]|None|1|34+S|21|

ordered first body empty, second0800, third80; opaque last body80 malformed nested
varint intentionally accepted. duplicates scalar values 0 then wrong-wire bytes80
(no overwrite), then -1 then2; type empty retained. opaque field1 wrong-wire0/5/1,
unknown field3/wire2 malformed body ff000b, then selected malformed Type80.
Each decoded raw/body points into exact supplied slice; empty body's as_ptr matches
input pointer at range start, including one-past-end. No owned body copy.

At exact limits success; at +1 success; bytes N-1 -> Limit(bytes)@0 (N>0).
records F-1 -> Limit(records)@lastfield. work total-1 -> Limit(work)@lastfield.
owned C*S-1 -> Limit(owned_bytes) at last growth: ordered6,duplicates13,opaque21.
Empty all zero succeeds; even limits.usizeMAX clamp to hard bounds.

## Every fixture cut, inclusive zero/full

All listed boundaries succeed; every other cut n returns Truncated@n with default
limits. This covers 1+12+16+25=54 cuts (including duplicate empty prefixes).

ordered: n0(types[],None),2([empty],None),6([empty,0800],None),
9([empty,0800,80],None),11([empty,0800,80],Some1).
duplicates: n0([],None),2([],Some0),5([],Some0),11([],Some-1),
13([],Some2),15([empty],Some2).
opaque: n0/2/7/16/21([],None),24([80],None).
empty: n0([],None).

## Scalar conversion and duplicates

Prefix10 followed by any value literal:
00->0; 01->1; 02->2; ff ff ff ff 0f->-1;
fe ff ff ff 0f->-2; ff ff ff ff ff ff ff ff ff 01->-1;
80 80 80 80 08->i32MIN; ff ff ff ff 07->i32MAX;
80 80 80 80 10->0; 80 00->0;
80 80 80 80 80 80 80 80 80 02->0;
ff ff ff ff ff ff ff ff ff 7f->-1 (unchanged wire slow-path).
For k2 empty types, retain all scalar values including0/1/2/3/-1/-2 unchanged;
no nullability/index application. Absent remainsNone regardless of entries.
'10 01 10 02' -> Some2. '10 02 10 01' -> Some1.
Each accepted wrong-wire field2 variant '11 0000000000000000', '12 01 80',
'15 00000000' alone givesNone; before/after10 01 stillSome1.
Selected Type field1 wrong wire0/1/5 does not append, with/without valid entries.
Nonminimal '8a 00 80 00' gives one empty type; '90 00 80 00' givesSome0.
Low32-tag '8a 80 80 80 10 00' gives one empty Type;
'90 80 80 80 10 00' givesSome0. Unknown field3 wires0/1/2/5 stay raw.

## Structural and read offsets

Each standalone literal below has exact default-limit error; prefixing '0a 00'
adds2 to error offset, preserving structural error despite retained selected type.

|literal|kind|offset|
|--|--|--:|
|80|Truncated|1|
|0a|Truncated|1|
|10|Truncated|1|
|00/03/07|InvalidFieldNumber|0|
|0b/0c/13/14/1b/1c|UnsupportedGroup|0|
|0e/0f/16/17|InvalidWireType|0|
|0a 02 ff|Truncated|3|
|0d 01 02 03|Truncated|4|
|09 01 02 03 04 05 06 07|Truncated|8|
|0a 80 80 80 80 08|NegativeLength|1|
|0a ff ff ff ff 07|Truncated|6|
|10 + ten80|MalformedVarint|10|
|ten80 tag|MalformedVarint|9|
|0a + ten80|MalformedVarint|10|

For prefix10 plus n continuation80 bytes n1..9 -> Truncated@(n+1),
n10 -> MalformedVarint@10. Same with prefix0a (length varint).
No nested/body error for '0a 01 80', '0a 03 ff 00 0b', or empty type.

## Aggregate growth / admission precedence

k repeated '0a 00' entries k1..5 have (N,F,C)=(2k,k,1/2/4/4/8).
Exact work4k+C*S and ownedC*S succeed with correct ordered empty bodies.
Owned one-short fails at offsets0/2/4/4/8 respectively; work total-one-short
fails at0/2/4/6/8 respectively. This distinguishes logical capacity from k*S
and cumulative from per-entry admission. No allocation occurs before owned/work.

Literal '0a 00', record1:
work0 -> Limit(work)@0(record charge);
work1 -> Limit(work)@0(tag);
work2 -> Limit(work)@1(length);
work3 -> Limit(work)@0(dispatch);
work4,owned0 -> Limit(owned_bytes)@0;
work4,ownedS -> Limit(work)@0(capacity);
work4+S,ownedS -> success.
records0,work0,owned0 -> Limit(records)@0.
bytes1,records0,work0,owned0 -> Limit(bytes)@0.

'0a 02 ff', records1: work4 -> Limit(work)@2; work5 -> Truncated@3.
'10', records1: work2 -> Limit(work)@1; work3 -> Truncated@1.
'0a 00 80', records1 -> Limit(records)@2;
records2/work6+S -> Limit(work)@3; work7+S -> Truncated@3.
'0a 00 00', records2/work6+S -> InvalidFieldNumber@2.

Late unknown scalar '18 00' after k2 entries needs records3/work12+2S;
one-short work fails dispatch@4, records2 fails@4, proving no early stop.
1024-byte opaque Type: '0a 80 08' +1024 ff costs1029+S work,F1,C1;
1028+S fails capacity-work@0,1029+S succeeds; no nested scan.

Hard clamps: byte input16MiB+1 rejects at0 with usizeMAX limits;
65535 repeated empty types at N131070,C65536 exact work262140+65536S and
owned65536S succeed; append one more field -> Limit(records)@131070.
Work hard64Mi cannot be reached by this flat scan under16MiB/65535/S<=16;
clamping checked by source inspection rather than impossible expectation.
Allocation failure mapping static only. Deterministic all65536 two-byte combinations
and selected valid-prefix tails under bytes32/records8/work256/owned128 must repeat
same success/error; errors<=len; successful outputs borrow input with length bounded
by records and empty/scalar presence consistency. No semantic oracle executed.

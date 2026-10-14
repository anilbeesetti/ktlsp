# Independent literal acceptance freeze v2

Authored from pinned schema/generated source and existing wire/budget rules before
TypeParameter implementation, encoder or runtime. Hex separated bytes. B means
`08 00 10 00` (4 bytes); B+ means literal concatenation. S=size_of::<UpperBound>()
in the target Rust ABI (normally32 on64-bit), not an assumed universal byte count.
All successful results borrow whole raw pointer-identically; selected bound bodies
borrow exact input subranges, including zero-length slices. No unknown vector.

## Committed hex fixtures proposed

minimal.hex: `08 00 10 00`
-> id0,name0,reifiedNone,varianceNone,empty bounds/IDs; work8,records2,owned0.

mixed.hex: `08 00 10 00 30 07 32 02 08 09 32 00 30 0a 2a 00 2a 02 0b ff`
-> id0,name0,None,None; IDs[7,8,9,10]; bounds rawempty span16..16, raw0bff
span18..20. Field spans0..2,2..4,4..6,6..10,10..12,12..14,14..16,16..20.
N20,F8,P2,L2,U2,C16+2S => records10,work60+2S,owned16+2S.
0bff is deliberately invalid nested protobuf; accepted as opaque.

scalars.hex: `08 ff ff ff ff 0f 10 fe ff ff ff 0f 18 00 20 02 30 ff ff ff ff 0f`
-> id-1,name-2,reifiedSome(false),varianceSome(Inv),IDs[-1],empty bounds.
N22,F5,U1,C4 => records5,work37,owned4.

## Additional successful literals in dedicated tests

| Hex | Expected selected result |
| --- | --- |
| B+`18 00 20 02` | Some(false),Some(Inv), presence differs from None |
| B+`18 02 20 00` | Some(true),Some(In) |
| B+`18 01 18 00 20 00 20 01 20 03` | reifiedSome(false),varianceSome(Out); unknown3 retained only raw |
| B+`20 03` | varianceNone, effective defaultInv |
| B+`20 ff ff ff ff 0f` | varianceNone; unknown signed-1 |
| `08 01 10 02 08 03 10 04` | id3,name4, duplicates full raw |
| B+`0a 00 12 00 1a 00 22 00 28 01 35 00 00 00 00` | unchanged id/name/None/None/empty repeateds; wrong-wire fields opaque |
| B+`18 80 80 80 80 10` | reifiedSome(true), full raw64=2^32 |
| B+`18 80 80 80 80 80 80 80 80 80 02` | reifiedSome(false), retained slow-path low64=0 |
| `88 00 80 00 90 00 81 00` | nonminimal tags/values id0,name1 |
| `08 80 80 80 80 10 10 00` | id0,name0, low32 int32 truncation |
| B+`30 01 32 02 02 03 30 04 32 00` | IDs[1,2,3,4], mixed/empty packed |
| B+`32 01 07 32 01 08` | IDs[7,8], split packed segments |
| B+`32 05 ff ff ff ff 0f` | IDs[-1], packed signed interpretation |
| B+`32 02 80 00` | IDs[0], nonminimal packed varint |
| B+`2a 01 80 2a 00` | bound bodies80 at6..7; empty at9..9 |
| B+`a2 06 01 00 aa 06 01 80` | annotation100/extension101 wholly opaque; empty selected lists |
| B+`79 01 02 03 04 05 06 07 08 85 01 78 56 34 12` | unknown fixed64#15/fixed32#16 raw retained |

## Error literals (default limits unless specified)

Wire(k) means ErrorKind::Wire(wire::ErrorKind::k), offsets message-relative.
All errors return no partial record. MissingId wins only at clean full EOF.

| Hex | Exact kind @offset |
| --- | --- |
| empty | MissingId @0 |
| `08 00` | MissingName @2 |
| `10 00` | MissingId @2 |
| `0a 00 10 00` | MissingId @4 (wrong-wire ID) |
| `08 00 12 00` | MissingName @4 (wrong-wire name) |
| `08 00 80` | Wire(Truncated) @3, before missing-name |
| B+`00` | Wire(InvalidFieldNumber) @4 |
| B+`07` | Wire(InvalidFieldNumber) @4, before invalid-wire |
| B+`0b` or B+`0c` | Wire(UnsupportedGroup) @4 |
| B+`0e` or B+`0f` | Wire(InvalidWireType) @4 |
| B+`30` | Wire(Truncated) @5 |
| B+`32 02 01` | Wire(Truncated) @7, outer length bound |
| B+`32 01 80` | Wire(Truncated) @7, packed entry bound |
| B+`32 01 80 30 01` | Wire(Truncated) @7, does not consume sibling field |
| B+`2a 02 01` | Wire(Truncated) @7, outer bound before opaque acceptance |
| B+`32 80 80 80 80 08` | Wire(NegativeLength) @5 |
| B+`30 80 80 80 80 80 80 80 80 80 80` | Wire(MalformedVarint) @14 |
| B+`32 0a 80 80 80 80 80 80 80 80 80 80` | Wire(MalformedVarint) @15 |
| B+`79 00` | Wire(Truncated) @6, short fixed64 |

Cut prefixes of minimal/scalars/mixed are asserted against independent boundaries:
minimal lengths0 MissingId0,1 Truncated1,2 MissingName2,3 Truncated3,4 success.
Mixed prefix0 MissingId0, prefix2 MissingName2; prefix4 succeeds empty;
prefix6 IDs[7]; prefix10/prefix12 IDs[7,8,9];
prefix14 IDs[7,8,9,10]; prefix16 adds empty bound16..16; prefix20 full success.
All other positive mixed cuts fail Truncated at that cut. Full scalar
prefixes6 -> MissingName6;12 -> required fields success;
14 -> reifiedfalse;16 -> varianceInv;22 -> IDs[-1]. Other positive scalar cuts
Truncated at cut. No expected values obtained from a passing decoder.

## Exact limit boundaries

Use generous unmentioned limits; pair at boundary with immediately below. The
minimal byte/record/work cases also run with missing required fields when specified.

| Literal / limits | Expected |
| --- | --- |
| empty / bytes0,records0,work0,owned0 | MissingId @0 (no root-node charge) |
| minimal / bytes4,records2,work8,owned0 | success |
| minimal / bytes3 | Wire(Limit("bytes")) @0 |
| minimal / records1 | Wire(Limit("records")) @2 |
| minimal / work0 or work1 | Wire(Limit("work")) @0 |
| minimal / work7 | Wire(Limit("work")) @2, dispatch |
| B+`30 07` / records3,work17,owned4 | IDs[7] success |
| B+`30 07` / owned3 | Wire(Limit("owned_bytes")) @4 |
| B+`30 07` / work16 | Wire(Limit("work")) @4, reservation precharge |
| B+`32 02 07 08` / records5,work28,owned8 | IDs[7,8] success |
| B+`32 02 07 08` / records4 | Wire(Limit("records")) @7 |
| B+`32 02 07 08` / owned7 | Wire(Limit("owned_bytes")) @7 |
| B+`32 02 07 08` / work27 | Wire(Limit("work")) @7, second reservation |
| B+`32 01 07 32 01 08` / records6,work32,owned8 | IDs[7,8] success |
| B+`32 01 07 32 01 08` / records5 | Wire(Limit("records")) @9, proves aggregate |
| B+`32 01 07 32 01 08` / owned7 or work31 | corresponding Wire Limit @9 |
| B+`32 00` / records3,work12,owned0 | success empty IDs |
| B+`2a 00` / records3,work12+S,ownedS | empty bound6..6 success |
| B+`2a 00` / ownedS-1 or work11+S | corresponding Wire Limit @4 |
| mixed / records10,work60+2S,owned16+2S | success |
| mixed / owned15+2S or work59+2S | corresponding Wire Limit @16, joint vectors |
| B+`32 01 80 30 01` / records3 | Wire(Limit("records")) @6 before packed truncation |
| B+`32 01 80 30 01` / work14 | Wire(Limit("work")) @6 before packed truncation |

Hard-clamp evidence is separated by reachability: unchanged native CI exercises
usize::MAX byte/record clamps; constructor inspection verifies min(MAX_*) for
64Mi work/16Mi owned ceilings. No exhaustion of those larger ceilings is claimed:
the byte/record limits may make them unreachable here. Caller-tightened work/owned
boundaries above do execute. Byte overage returns Limit bytes at0. Build65,535-field
stream: required B plus65,533 scalar unknown
`38 00` fields; len131,070, records65,535, work262,140,owned0 succeeds. Append
`38 00` => records limit at131,070 with caller usize::MAX. No unbounded benchmark.
All2-byte hostile pairs alone and following B, plus repeated-byte slices0..31,
use tight bytes32/records8/work256/owned256: deterministic result/error and all
successful borrowed spans within source; errors offset<=len. Runtime only Linux CI.

## Review correction provenance

An initial adversarial source note incorrectly inferred bool used low32 and gave
one opaque body start5; exact pinned readBool inspection and byte transcription
corrected these to full raw64 and start6 before this v1 freeze. No implementation
or runtime supplied the corrections. Preserve reviewer messages in review receipt.
All three v1 reviewers found mixed prefix2 was incorrectly included in a truncation
catch-all; v2 explicitly sets MissingName@2. Architect requested executable clamp
evidence be distinguished from constructor inspection; v2 does so above.

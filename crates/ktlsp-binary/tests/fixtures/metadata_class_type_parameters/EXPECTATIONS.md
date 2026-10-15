# Literal expectations — independently authored before execution

All tuples are prospective, source-derived requirements, never observed outputs.
Generous limits unless a limit is specified. S=SelectedTypeParameter slot size,
B=UpperBound slot size. Raw/borrowed pointer identities must accompany every
success: header.raw pointer equals input; each parameter.raw pointer equals
input[span].as_ptr(); each opaque bound pointer equals parameter.raw[bound.span].
No strings/names/types reconstructed. Default parent flags=None and fq_name=0
unless stated. Default child reified=None/variance=None and bound lists empty.

## Frozen fixture literals and successful tuples

minimal.hex: `18 00`
Result children[]; (bytes,records,nodes,depth,work,owned)=(2,2,1,1,9,0).

selected.hex: `18 00 2a 04 08 00 10 00`
Result [(span4..8,id0,name0)]; exact (8,6,2,2,34+S,S).

duplicates.hex: `18 00 2a 04 08 00 10 00 2a 04 08 00 10 00`
Result [(4..8,0,0),(10..14,0,0)] separately, exact(14,10,3,2,59+2S,2S).

three.hex: `18 00 2a 04 08 00 10 00 2a 04 08 00 10 00 2a 04 08 00 10 00`
Result [(4..8,0,0),(10..14,0,0),(16..20,0,0)]; exact(20,14,4,2,84+4S,4S).
Third outer logical growth adds2S, not1S; no speculative directory capacity.

scalars.hex: `18 00 2a 16 08 ff ff ff ff 0f 10 fe ff ff ff 0f 18 00 20 02 30 ff ff ff ff 0f`
Result [(4..26,id-1,name-2,reifiedSome(false),varianceSome(Inv),boundIDs[-1])];
exact(26,9,2,2,99+S,4+S). This authored scalar positive is not compiler evidence.

presence.hex: `18 00 2a 0e 08 00 10 00 18 01 18 00 20 00 20 01 20 03`
Result [(4..18,0,0,reifiedSome(false),varianceSome(Out))]; unknown enum3 does not
replace Out, exact(18,11,2,2,74+S,S). None/default remains distinct from Some.

wrong_wire.hex: `18 00 28 01 2d 00 00 00 00`
Result[]; field5/wire0 and field5/wire5 are unselected raw; exact(9,6,1,1,31,0).
Inline child wrongwire control:
`18 00 2a 0e 08 00 10 00 0a 00 12 00 1a 00 22 00 28 01`
Result [(4..18,0,0,None,None)] exact(18,11,2,2,74+S,S); wrongwire scalar/bound
fields do not set or clear presence and field5/wire0 is not a bound.

opaque.hex: `18 00 2a 14 08 00 10 00 30 07 32 02 08 09 32 00 30 0a 2a 00 2a 02 0b ff`
Result [(4..24,0,0,None,None,boundIDs[7,8,9,10],bounds[(16..16,[]),(18..20,[0b,ff])])].
Bounds span16..16 and18..20 are CHILD-relative; their Class positions20..20 and
22..24 are for witness comparison only, never stored in raw UpperBound.span.
Exact(24,14,2,2,118+2B+S,16+2B+S). Nested group/ff bytes are opaque, not errors.
Unselected malformed body control `18 00 32 01 ff 4a 01 80` succeeds[];
exact(8,6,1,1,29,0). Only selected TypeParameter bodies are inspected.

nonminimal.hex: `98 00 80 00 aa 00 84 00 88 00 80 00 90 00 81 00`
Result [(8..16,0,1)] exact(16,6,2,2,54+S,S). Existing low32 tag/length rules and
nonminimal scalar varints apply. No new canonical-protobuf restriction.
Signed parent inline `08 ff ff ff ff 0f 18 fe ff ff ff 0f 2a 04 08 00 10 00`
Result parent(flagsSome(-1),fq_name-2),[(14..18,0,0)]; exact(18,8,2,2,58+S,S).
Last-win child inline `18 00 2a 08 08 01 10 02 08 03 10 04`
Result [(4..12,3,4)]; exact(12,8,2,2,50+S,S).

pair_class.hex is EXACT existing decoded_d1[47..285], transcribed in
PAIR_WITNESS.md; genuine result parent(flagsSome1031,fq_name0),
[(7..13,id0,name1,None,SomeOut),(15..21,id1,name2,None,SomeOut)].
Both bound vectors empty. Exact(238,36,3,2,563+2S,2S), parentF15/childF3 each.

## Error tuples (phase, kind, Class offset; consumed R/Q/W/O)

Here R/Q/W/O count successful records/nodes/work/owned deductions through failure;
no counter introspection API is proposed. They are static accounting expectations.
Parent Class(...) wraps ClassHeader errors; TP(...) wraps child TypeParameter
errors; Wire(...) is container selection/node/growth error. Bounded errors can
precede these generous-budget structural errors.

|Literal Class|Expected error|R/Q/W/O|
|---|---|---|
|empty|Class MissingFqName@0|0/1/1/0|
|08 06|Class MissingFqName@2|1/1/5/0|
|1a 00|Class MissingFqName@2|1/1/5/0|
|2a 01 00|Class MissingFqName@3|1/1/6/0|
|18 00 2a 00|TP MissingId@4|4/2/18/0|
|18 00 2a 02 08 00|TP MissingName@6|5/2/26/0|
|18 00 2a 02 10 00|TP MissingId@6|5/2/26/0|
|18 00 2a 01 00|TP Wire InvalidFieldNumber@4|5/2/22/0|
|18 00 2a 01 80|TP Wire Truncated@5|5/2/23/0|
|18 00 2a 01 0b|TP Wire UnsupportedGroup@4|5/2/22/0|
|18 00 2a 01 0e|TP Wire InvalidWireType@4|5/2/22/0|
|18 00 2a 01 00 00|Class Wire InvalidFieldNumber@5|3/1/12/0|
|18 00 2a 00 00|Class Wire InvalidFieldNumber@4|3/1/11/0|
|18 00 2b|Class Wire UnsupportedGroup@2|2/1/7/0|
|18 00 2e|Class Wire InvalidWireType@2|2/1/7/0|
|18 00 2a 80|Class Wire Truncated@4|2/1/9/0|
|18 00 2a ff ff ff ff 0f|Class Wire NegativeLength@3|2/1/12/0|
|18 00 2a 80 80 80 80 80 80 80 80 80 80|Class Wire MalformedVarint@12|2/1/17/0|

Child-required precedence must include allocating-before-missing-ID:
`18 00 2a 02 30 07` => TP MissingId@6, R5/Q2/W31/O4.
This preserves the successful bound-ID storage charge, never refunds it.
Opaque child bound then invalid tail:
`18 00 2a 07 08 00 10 00 2a 00 00` => TP Wire InvalidFieldNumber@10,
R8/Q2/W46+B/O B. No outer S was charged, existing bound body span6..6.

Parent required-field/tail errors above win even with child depth1/nodes1/owned0
when parent node and full parent scan fit. Example invalid-child-plus-parent-tail
with depth1 still returns Class InvalidFieldNumber@5, not depth@4. Same malformed
child without tail and with depth1 returns container Wire Limit(depth)@4.

## Exact whole-budget boundaries and monotonic failure charges

All unspecified limits equal generous defaults. Exact and +1 each succeed; byte
limit is not a charge. For selected.hex, full exact requirements listed above:

|Tight limit|Error|Consumed R/Q/W/O|
|---|---|---|
|bytes7 (also records0/work0)|Wire Limit(bytes)@0|0/0/0/0|
|nodes0|Wire Limit(nodes)@0|0/0/0/0|
|depth0 with nodes0|Wire Limit(depth)@0|0/0/0/0|
|work0|Wire Limit(work)@0|0/1/0/0|
|nodes1|Wire Limit(nodes)@4|4/1/25/0|
|depth1|Wire Limit(depth)@4|4/1/25/0|
|records5|TP Wire Limit(records)@6|5/2/30/0|
|owned S-1|Wire Limit(owned_bytes)@2|6/2/34/0|
|work33+S|Wire Limit(work)@2|6/2/34/S|
|records0/work1|Class Wire Limit(records)@0|0/1/1/0|

The final row means records0/work1: node succeeds, record fails before its work.
Separate work1/records6 => Class Wire Limit(work)@0, R1/Q1/W1/O0:
parent record deduction persists even though its following work charge fails.
Selected work25 => container Wire Limit(work)@4, R4/Q2/W25/O0: child node
deduction persists before its work fails. Empty-child work17 => Wire Limit(work)@4,
R4/Q2/W17/O0, before MissingId; exact18 reaches MissingId.

duplicates: owned2S-1 => Wire Limit(owned_bytes)@8 after R10/Q3/W59+S/O S.
three: owned4S-1 => Wire Limit(owned_bytes)@14 after R14/Q4/W84+2S/O2S;
work83+4S => Wire Limit(work)@14 after R14/Q4/W84+2S/O4S.
Each child is decoded first; no later retained slot charged before validation.
opaque: owned(16+2B+S)-1 fails final outer push@2, R14/Q2/W118+2B/O16+2B.
pair: records35 => Wire Limit(records)@235 during SECOND scan,
R35/Q3/W558+2S/O2S; work562+2S => Wire Limit(work)@235 during second dispatch,
R36/Q3/W562+2S/O2S; owned2S-1 => Wire Limit(owned_bytes)@13 after
R25/Q3/W324+S/O S. Arithmetic: parent node+scan269, second-scan first4 fields29,
two child nodes2, two child scans24, first outer growthS.

Hard clamp requirements (future authored inputs, no allocation/execution now):
input lenMAX_BYTES+1 and all limitsusize::MAX => Wire Limit(bytes)@0 before node.
Class `18 00` plus32,767 repeats of`20 00` has N65,536/F32,768/K0; all limits
usize::MAX => Wire Limit(records)@65,534 on last second-scan field (65,536th
attempt), consumed R65,535/Q1/W262,141/O0. Depthusize::MAX still clamps2,
nodesusize::MAX clamps65,535, owned/work hard clamps as defined by existing Budget;
no non-independent test requires force allocator failure or RSS measurement.

## Every byte-prefix expectations

selected.hex cuts n0..8: n0 Class MissingFqName@0; n1 Class Wire Truncated@1;
n2 success[]; n3..7 Class Wire Truncated@n; n8 exact selected success. For n3,
parent record2/work8; n4..7 parent record2/work(n+5), all Q1/O0; n1 R1/Q1/W4/O0.
duplicates n0..14: same n0..8; n9..13 Class Wire Truncated@n; n14 both success.
At n8 first success only; complete field-boundary prefixes are valid unframed
messages. For n9 parent R3/Q1/W16/O0; n10..13 parent R3/Q1/W(n+7)/O0.
three n0..20: same n0..14; n15..19 Class Wire Truncated@n; n20 all3.
At n15 parent R4/Q1/W24/O0; n16..19 parent R4/Q1/W(n+9)/O0.
opaque n0..24: n0 MissingFqName; n1 Truncated; n2 success[]; n3..23 parent
Truncated@n; n24 exact opaque success. n3 R2/Q1/W8/O0; n4..23 R2/Q1/W(n+5)/O0.
Every parent truncation prevents child parsing/storage even if bytes of a valid
child prefix are present. Empty child literal n0..4 uses same n0..3 as selected,
n4 TP MissingId@4, never success. All error phases/offsets above are Class-relative.

The remaining proposed fixtures have the following COMPLETE prefix partitions;
all unlisted cuts in the inclusive0..N domain are Class Wire Truncated@n.
For any parent-truncating prefix, let q be the count of complete outer fields
strictly before the unfinished field. The literal field-boundary lists below
freeze q: consumed R=q+1,Q1,W=n+2q+3,O0. No child work occurs on those errors.
This includes EOF inside tag, length, value or opaque body. On missing-fq_name
complete prefixes R=F,Q1,W=1+n+2F,O0. Successful prefixes use the exact success
formula with their listed fields/children, no unencoded later fields counted.

minimal N2: n0 Class MissingFqName@0; n2 success[] exact minimal tuple.
Outer boundaries[2].
scalars N26: n0 Class MissingFqName@0; n2 success[] exact minimal tuple;
n26 exact scalar success. Outer boundaries[2,26].
presence N18: n0 Class MissingFqName@0; n2 success[] exact minimal tuple;
n18 exact presence success. Outer boundaries[2,18].
wrong_wire N9: n0 Class MissingFqName@0; n2 success[] tuple(2,2,1,1,9,0);
n4 success[] tuple(4,4,1,1,17,0); n9 exact wrongwire success.
Outer boundaries[2,4,9]. At n4 the field5/wire0 remains unselected.
nonminimal N16: n0 Class MissingFqName@0; n4 success[] tuple(4,2,1,1,13,0);
n16 exact nonminimal success. Outer boundaries[4,16].
pair_class N238: n0 Class MissingFqName@0; n3 Class MissingFqName@3;
success cuts and (F,K,R,Q,W,O) tuples:

|n|F|K|R|Q|W|O|
|---|---|---|---|---|---|---|
|5|2|0|4|1|19|0|
|13|3|1|9|2|52+S|S|
|21|4|2|14|3|85+2S|2S|
|29|5|2|16|3|105+2S|2S|
|56|6|2|18|3|163+2S|2S|
|68|7|2|20|3|191+2S|2S|
|85|8|2|22|3|229+2S|2S|
|102|9|2|24|3|267+2S|2S|
|151|10|2|26|3|369+2S|2S|
|173|11|2|28|3|417+2S|2S|
|185|12|2|30|3|445+2S|2S|
|210|13|2|32|3|499+2S|2S|
|235|14|2|34|3|553+2S|2S|
|238|15|2|36|3|563+2S|2S|

Pair complete outer boundaries[3,5,13,21,29,56,68,85,102,151,173,185,210,235,238].
Only n13 retains first selected child (7..13,0,1,None,SomeOut); n21 and every
later successful cut retain both (second15..21,1,2,None,SomeOut); n5 retains[];
parent flagsSome1031/fq_name0 at every success. Depth1 for n5, otherwise2.
Remaining cuts[1,2,4,6..12,14..20,22..28,30..55,57..67,69..84,86..101,
103..150,152..172,174..184,186..209,211..234,236,237] are parent Truncated@n.
At n3 MissingFqName consumes R1/Q1/W6/O0; no child spans exist yet.

All optional inline controls also have full prefix partitions by the same rule:
child wrongwire N18, last-win N12, and empty-packed-ID N10 have boundaries[2,N],
n0 MissingFqName, n2 minimal success[], nN their literal full result; other cuts
parent Truncated@n. Signed-parent N18 boundaries[6,12,18]: n0 MissingFqName@0,
n6 MissingFqName@6 (R1/Q1/W9/O0), n12 success(flagsSome-1,fq_name-2,[]) with
tuple(12,4,1,1,33,0), n18 exact signed-parent child result; other cuts truncated.
Unselected-body N8 boundaries[2,5,8]: n0 MissingFqName, n2 minimal success[],
n5 success[] tuple(5,4,1,1,19,0), n8 exact unselected-body success; others truncated.

Finite hostile deterministic corpus, independent literal expected tuples:
`00`=>Class InvalidFieldNumber@0 R1/Q1/W3/O0;
`0e`=>Class InvalidWireType@0 R1/Q1/W3/O0;
`0b`=>Class UnsupportedGroup@0 R1/Q1/W3/O0;
`18 00 2a 01 ff`=>TP Truncated@5 R5/Q2/W23/O0;
`18 00 2a 02 08 80`=>TP Truncated@6 R5/Q2/W26/O0;
`18 00 2a 06 08 00 10 00 32 00`=>success [(4..10,0,0,boundIDs[])],
exact(10,7,2,2,42+S,S).
No random decoder-output oracle, panic-catching pass claim or finite-probe
all-input/stack/RSS/JVM-equivalence claim. More tests require independent literals.

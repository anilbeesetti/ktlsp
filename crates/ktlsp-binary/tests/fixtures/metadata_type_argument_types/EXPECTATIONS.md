# Independent preimplementation freeze

Original contract SHA256027a11442a9e8ec35dc96d6daefe6ee15b8771953012628d85e40a870510a4f7.
Original explicit12case/98prefix/13error JSON SHA25686425365e8d8270e6f4b35137566bfdfb023dccfcf2a4ed139d993051f7c5c92.
No implementation or native decoder was executed to obtain these expectations.
S=size_of::<type_argument::InlineType>(), T=size_of::<SelectedType>().

## Frozen independent successful literals

Unspecified scalars None. Full raw/pointer/span equality applies to every result.
Use repository input literals unchanged for selected/duplicates; copy bytes only
as independently frozen new fixtures. Genuine Pair fixture/notices stay readonly.

| name | bytes | parent | ordered child span/header | R,Q,D,O,W |
| --- | --- | --- | --- | --- |
| empty | empty | None,None | [] | 0,1,1,0,1 |
| empty_inline | 12 00 | None,None | 2..2/all None | 1,2,2,S+T,6+S+T |
| pair0 | 12 02 38 00 | None,None | 2..4/type_parameter0 | 2,2,2,S+T,12+S+T |
| pair1 | 12 02 38 01 | None,None | 2..4/type_parameter1 | 2,2,2,S+T,12+S+T |
| selected | 08 02 12 02 30 04 18 00 | Inv,Some0 | 4..6/class_name4 | 4,2,2,S+T,20+S+T |
| duplicates | 08 00 08 63 12 02 30 04 12 02 18 01 12 00 18 01 18 ff ff ff ff 0f 08 03 08 ff ff ff ff 0f | Star,Some(-1) | 6..8/class_name4;10..12/nullabletrue;14..14/all None | 11,4,2,4(S+T),60+4(S+T) |
| repeated | 12 02 38 00 12 02 38 01 | None,None | 2..4/parameter0;6..8/parameter1 | 4,3,2,2(S+T),23+2(S+T) |
| wrong_wire | 0a 01 80 10 01 1a 01 80 | None,None | [] | 3,1,1,0,15 |
| opaque_nested | 12 04 12 02 00 80 | None,None | 2..6/all None | 2,2,2,S+T,16+S+T |
| opaque_extension | 12 05 a2 06 02 00 80 | None,None | 2..7/all None | 2,2,2,S+T,18+S+T |
| child_wrong_wire | 12 03 3a 01 80 | None,None | 2..5/all None | 2,2,2,S+T,14+S+T |
| nonminimal_empty | 92 00 80 00 | None,None | 4..4/all None | 1,2,2,S+T,8+S+T |

Complete per-prefix partition frozen below; all absent prefixes return defaults.
T means Truncated@prefix length unless an earlier fully selected child error is
specified (none in these successful literals). At complete field boundaries,
selected values are exactly those of fully retained fields; never initialization.
- empty N0 success[0].
- empty_inline N2 success[0,2], T[1].
- pair0/pair1 N4 success[0,4], T[1,2,3].
- selected N8 success[0,2,6,8], T[1,3,4,5,7].
- duplicates N30 success[0,2,4,8,12,14,16,22,24,30], all other cuts T.
- repeated N8 success[0,4,8], all other cuts T.
- wrong_wire N8 success[0,3,5,8], all other cuts T.
- opaque_nested N6 success[0,6], all other cuts T.
- opaque_extension N7 success[0,7], all other cuts T.
- child_wrong_wire N5 success[0,5], all other cuts T.
- nonminimal_empty N4 success[0,4], all other cuts T.
Every cut0..=N must be asserted; exact full expected successes/errors for all98
cuts are independently frozen in temporary EXPECTATIONS.json before source.

## Error seeds and exact coordinates under adequate budgets

| literal | ErrorKind@Argument-relative offset |
| --- | --- |
| 12 01 80 | Truncated@3 |
| 12 01 00 | InvalidFieldNumber@2 |
| 12 01 0b | UnsupportedGroup@2 |
| 12 01 0e | InvalidWireType@2 |
| 12 01 80 00 | InvalidFieldNumber@3 (parent, before child) |
| 12 01 80 08 | Truncated@4 (parent, before child) |
| 12 00 12 01 80 | Truncated@5 |
| 92 00 81 00 80 | Truncated@5 |
| 12 02 38 00 12 02 08 80 | Truncated@8 |
| 12 06 12 80 80 80 80 08 | NegativeLength@3 |
| 12 0a 80 80 80 80 80 80 80 80 80 80 | MalformedVarint@11 |
| 12 02 ff | Truncated@3 (parent bulk) |
| 12 80 80 80 80 08 | NegativeLength@1 (parent) |

Also inherited parent malformed rows00/03/07 invalidField0;0b/0c group0;
0e/0f invalidWire0;09+7bytes truncated8;0d+3bytes truncated4;ten80 malformed9.
Prepend complete pair0 => same parent error offset+4. Wrap same malformed bodies
in selected inline Type => child error offset+body.start, never double rebase.

## Presence, scalar conversion and opaque controls

For parent projection known00/01/02/03 => In/Out/Inv/Star. For both projection and
parent type_id use signed low32 varints. Unknown projection values never erase
known; verify known/unknown/known and unknown/known/unknown in separate literals.
Type integer fields1/4/6/7/8/9/11/12/14 use signed low32 independently, last-win
within EACH occurrence; child nullable uses full raw64!=0, not low32. Cases:
00,80 00 =>0; ff ff ff ff 0f and ff ff ff ff ff ff ff ff ff 01 =>-1;
80 80 80 80 08 =>MIN; ff ff ff ff 07 =>MAX;80 80 80 80 10=>0.
Bool00/80 00 false,01/02/80 80 80 80 10 true. Raw64 slow-path inherited limit
must stay documented, not universal JVM fast-path compatibility. Duplicate Type
bodies with different scalars MUST NOT merge or clear earlier occurrences.
For EACH parent known field1/2/3 and child selected scalar, wrong wires1/2/5
(or wire0 for parent field2) before/after recognized values retain presence;
length bodies80 remain opaque if field is wrong-wire or unselected. Empty child
all None distinct from explicit scalar zero; parent Inv with no type and Star
with inline type/id both structurally valid, no initialization validation.
Unknown child length fields2/5/10/13/100/101..199 with malformed interior00/80/
0b are opaque framing successes, not recursive errors/depth3. Unknown supported
wire0/1/2/5 fields and reserved/max field numbers remain exact in borrowed raw.

## Exact tightened limits, growth and hard clamps

All table minima and each minimum+1 pass; independently lower each resource by1.
For selected: records3=>records@4;nodes1=>nodes@4;depth1=>depth@4;
owned(S+T)-1=>owned_bytes@4;work(20+S+T)-1=>work@4(result reserve).
For duplicates: records10=>records@10;nodes3=>nodes@14;depth1=>depth@6;
owned4(S+T)-1=>owned_bytes@14;work60+4(S+T)-1=>work@14.
For repeated: records3=>records@6;nodes2=>nodes@6;owned2(S+T)-1/work23+2(S+T)-1
=> respective limit@6. Pair records1/nodes1/depth1/ownedO-1/workW-1 => respective
limit@2;bytes3=>bytes@0 even all other limits0. Empty bytes/records/owned0,
nodes1/depth1/work1 pass; all0=>depth@0;depth1/nodes0=>nodes@0;work0=>work@0.
For12 00: work0(root)@0;work1(record)@0;work2(tag)@0;work3(length)@1;
work4(dispatch)@0;work5(staging-capacity)@0;work5+S(child-node)@2;
work6+S(result-capacity)@2; exact6+S+T passes. With ownedS-1 and work5,
owned_bytes@0 wins before staging capacity-work. With ownedS and work6+S,
owned_bytes@2 wins before result capacity-work. records0/depth1=>records@0;
records1/depth1=>depth@2 AFTER complete staging. depth0/records0=>depth@0.

k1..5 copies12 00: C=[1,2,4,4,8],R=k,Q=1+k,D2,O=C(S+T),W=1+5k+O.
ownedO-1 errors at body[2,4,6,6,10]; workW-1 at body[2,4,6,8,10].
Staging-only limitC*S-1 fails tag[0,2,4,4,8] before ANY child; nodes1 + that
staging deficit must still fail owned rather than child nodes. Prove no refund
by ownedC*S: staging can finish but first typed reserve fails body2. ownedC*T
alone also cannot satisfy aggregate (unless it incidentally exceeds aggregate,
which it cannot for positive S); do not infer target sizes outside Rust size_of.
Opaque_extension and nested exact budgets prove grandchildren unscanned/depth2.
Use caller usize::MAX for each clamps; MAX_BYTES+1=>bytes@0.65535 copies18 00
fits R65535/Q1/O0/W262141 then one extra=>records@131070 with callerusizeMAX.
Empty children65535 pass Q65536? NO: nodes hard65535 fails last child body131070,
provided layout-dependent owned/work hard ceilings permit reaching it; do not
invent that independent hard-node witness. Use tightened node limits instead.
Independent hard-work/owned exhaustion not claimed; allocation failure/overflow
mapping static review only, no deterministic OOM or huge allocation test.

Finite hostile probe eventual test: all a,b0..255 for[a,b] and[12,02,a,b], plus
all a repeated n0..31;139264 inputs, twice=>278528 calls. Limits bytes36/records8/
nodes8/depth2/work1024/owned1024. Equal complete Results, success raw/pointer/
ordered span assertions, errors offset<=len. No local execution admission.


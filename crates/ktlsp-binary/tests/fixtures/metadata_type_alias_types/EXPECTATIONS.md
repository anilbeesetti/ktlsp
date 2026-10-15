# Independent literal expectations v1 — before source/runtime

All fixture bytes are independently authored; no decoder/encoder/compiler/oracle
execution supplies expected outcomes. P=size_of::<&[u8]>(); S=size_of::<SelectedType>();
Q defined in CONTRACT.md. Errors below are existing TypeAlias Error {offset,kind};
wire failures use ErrorKind::Wire(kind). All success budgets are symbolic ABI sizes.
Every success checks raw bytes AND pointer identity for parent and every child,
exact field number/order/body span/header tuple, bounds, and no output staging
vectors. Child raw equals parent[span] and its pointer, even for empty EOF slices.
Header tuple order: flags, nullable projected bool->0/1, flexible_capabilities,
class_name, type_parameter, flexible_upper_id, parameter_name, outer_id, alias_name,
abbreviated_id. '-' means None; numeric entries mean Some(value). Z is ten Nones.
Parent tuple=(flags,name,underlying_id,expanded_id); '-' is None, name is required.
No defaults filled in; no output merging/ID precedence. Every error has no partial
result and offset<=input.len(). All ordinary errors default sufficient other limits.

## Complete tuples and exact aggregate minima

| fixture | parent tuple | types in wire order: (field,body span,header tuple) |
|---|---|---|
| minimal | (-,0,-,-) | [] |
| explicit | (6,0,0,0) | [] |
| selected | (0,5,7,8) | (4,6..10,[-,1,-,2,-,-,-,-,-,-]); (6,12..14,[-,-,-,-,3,-,-,-,-,-]) |
| interleaved | (-,2,-,-) | (6,4..6,[-,-,-,-,1,-,-,-,-,-]); (4,8..10,[-,-,-,2,-,-,-,-,-,-]); (6,12..12,Z); (4,14..18,[-,-,-,4,-,-,-,-,-,-]) |
| empties | (-,0,-,-) | (4,4..4,Z); (6,6..6,Z); (4,8..8,Z); (6,10..10,Z); (4,12..12,Z) |
| opaque | (-,5,-,-) | (4,4..16,Z) |
| wrong_wire | (-,5,-,-) | [] |
| nonminimal | (-,0,-,-) | (6,6..6,Z); (4,13..15,[-,-,-,4,-,-,-,-,-,-]) |
| all_scalars | (-,0,-,-) | (4,4..24,[0,0,1,4,2,3,5,6,7,8]) |
| signed_parent | (-1,-2147483648,2147483647,0) | [] |

| fixture | N/F/K/M/C/H | records/nodes/depth | owned | work |
|---|---|---|---|---|
| minimal | 2/1/0/0/0/0 | 1/1/1 | 0 | 5 |
| explicit | 8/4/0/0/0/0 | 4/1/1 | 0 | 17 |
| selected | 18/6/2/6/3/1 | 9/3/2 | 2P+2S | 48+2P+2S |
| interleaved | 20/6/4/8/4/3 | 10/5/2 | 4P+4S | 60+4P+4S |
| empties | 12/6/5/0/0/4 | 6/6/2 | 6P+8S | 39+6P+8S |
| opaque | 22/4/1/12/4/0 | 8/2/2 | P+S | 53+P+S |
| wrong_wire | 23/6/0/0/0/0 | 6/1/1 | 0 | 36 |
| nonminimal | 15/3/2/2/1/1 | 4/3/2 | 2P+2S | 31+2P+2S |
| all_scalars | 24/2/1/20/10/0 | 12/2/2 | P+S | 71+P+S |
| signed_parent | 24/4/0/0/0/0 | 4/1/1 | 0 | 33 |

opaque Type fields2/5/10/13 contain malformed opaque80/00/ff/0b. Parent fields3/8
contain opaque80/00, untouched. Header scalars all None, no nested traversal.
wrong_wire fields4/6 varint/fixed32/fixed64 do not create selected occurrences;
wrong-wire name2 body80 does not clear name5. nonminimal field6 tag b2 00 and
length80 00 is empty at6; low32-aliased field4 tag a2 80 80 80 10 and length82 00
produce body13..15=30 04. Never search for matching/empty bytes to derive offsets.

Exact minima and ALL minima+1 succeed. Also tighten one limit at a time to exact
minimum, others default; usize::MAX limits clamp. bytes=N-1 with other limits0
fails bytes0 first. One-less each nonzero limit has these first errors:

| fixture | records-1 | nodes-1 | depth-1 | work-1 | owned-1 |
|---|---|---|---|---|---|
| minimal | records0 | nodes0 | depth0 | work0 | N/A |
| explicit | records6 | nodes0 | depth0 | work0 | N/A |
| selected | records12 | nodes12 | depth6 | work12 | owned_bytes12 |
| interleaved | records16 | nodes14 | depth4 | work16 | owned_bytes12 |
| empties | records10 | nodes12 | depth4 | work12 | owned_bytes12 |
| opaque | records13 | nodes4 | depth4 | work4 | owned_bytes4 |
| wrong_wire | records20 | nodes0 | depth0 | work0 | N/A |
| nonminimal | records13 | nodes13 | depth6 | work13 | owned_bytes13 |
| all_scalars | records22 | nodes4 | depth4 | work4 | owned_bytes4 |
| signed_parent | records18 | nodes0 | depth0 | work0 | N/A |

The number appended to each label is the parent-relative offset. No-child depth0
fails root depth0. Interleaved owned-1 fails third typed push at empty span12;
last child has no typed growth, so work-1 fails its last scalar dispatch at16.

## Every byte cut (all fixtures)

For cut n0..=N, the following parent field-boundary lists are exhaustive; every
other cut is parent Wire(Truncated) at n (including cuts inside opaque bodies).
At listed boundary0 =>MissingName0. Before first name, explicit and selected
boundary2 and signed_parent boundary6 =>MissingName at that boundary. All other listed boundaries
succeed with the literal parent state/types accumulated below; raw is exact prefix.
No malformed child hides behind an accepted boundary in these ten fixtures.

minimal: [0,2]; n2 full.
explicit: [0,2,4,6,8]; n4 flags6/name0/IDsNone, n6 underlying0, n8 expanded0.
selected: [0,2,4,10,14,16,18]; n2 MissingName2; n4 flags0/name5;
n10 first Type tuple, n14 both, n16 underlying7, n18 expanded8.
interleaved: [0,2,6,10,12,18,20]; n2 name1; n6/n10/n12/n18 retain first
1/2/3/4 Type tuples respectively and name1; n20 name2.
empties: [0,2,4,6,8,10,12]; n2 name0 then exactly first(n-2)/2 empty tuples.
opaque: [0,2,16,19,22]; n2 name5; n16 adds one typed tuple; n19/n22 same tuple.
wrong_wire: [0,2,4,6,11,20,23]; all nonzero boundaries name5/noTypes/scalarsNone.
nonminimal: [0,2,6,15]; n2 name0; n6 first empty field6; n15 both tuples.
all_scalars: [0,2,24]; n2 name0/noTypes; n24 full header tuple.
signed_parent: [0,6,12,18,24]; n12 flags-1/nameMIN/IDsNone; n18 underlyingMAX;
n24 expanded0. Boundary6 MissingName6, not a node/depth error.
For every successful cut use defaults, repeat raw/header pointer/span checks;
full fixture exact budget checks remain above, not recalculated from decoder output.

## Parent compatibility, first-error precedence and exact rebasing

Empty input =>MissingName0 even ALL limits0 (byte check passes, no parent fields).
`22 01 80` =>MissingName3 even nodes0/depth0, sufficient other limits; child never
visited. `22 00` owned0 =>owned_bytes0 before MissingName/root. `12 00` =>MissingName2.
`10 00` records0/work0/nodes0/depth0 =>records0, not composition depth.
`10 00 22 01 80 00` =>PARENT InvalidFieldNumber5 before childTruncated5.
`22 01 80 00` =>PARENT InvalidFieldNumber3 before MissingName/child.
`10 00 22 02 30 04 80` =>PARENT Truncated7 before child visit.
`10 00 22 02 ff` =>PARENT Truncated5; child not visited.
`10 00 22 01 80` =>child Truncated5 (body4..5, local1).
`10 00 22 01 80 28 00` =>child Truncated5; no following-container bleed.
`10 00 32 01 00 22 01 80` =>first child InvalidFieldNumber4, not secondTruncated8.
`10 00 22 01 80 32 01 00` =>first child Truncated5, not laterInvalidFieldNumber7.
`10 00 22 02 30 04 32 01 80` =>SECOND child Truncated9.
`10 00 32 02 30 04 22 01 80` =>SECOND child Truncated9 (opposite field order).
`10 00 22 00 32 01 80` =>second child Truncated7 after first empty child.
For every selected-child case substitute field4/6 and vice versa; same kind/offset.
For each malformed-child case separately prepend parent `08 00` and `a0 06 00`:
parent remains valid, same kind and exact offset+2/+3; never double-rebase.
Existing raw public type_alias::decode still accepts each well-framed opaque child
case (including raw inline.hex), with unchanged scalar/opaque lists and limits.
The new wrapper can reject such bodies without changing raw reader behavior.

Freeze child structural table: wrap body B separately as `10 00 22 L B` and
`10 00 32 L B`, where literal one-byte L=len(B), all listed lengths<128. Body
start4. Expected child kind/localoffset as below; final error offset=4+local.

| literal B | kind | localoffset |
|---|---|---|
| 30 / 80 | Truncated | 1 |
| 00 / 03 / 07 | InvalidFieldNumber | 0 |
| 0b / 0c / 23 / 24 / 33 / 34 | UnsupportedGroup | 0 |
| 0e / 0f / 26 / 27 / 36 / 37 | InvalidWireType | 0 |
| 30 80 | Truncated | 2 |
| 12 02 ff | Truncated | 3 |
| 0d 01 02 03 | Truncated | 4 |
| 09 01 02 03 04 05 06 07 | Truncated | 8 |
| 12 80 80 80 80 08 | NegativeLength | 1 |
| 12 ff ff ff ff 07 | Truncated | 6 |
| ten80 | MalformedVarint | 9 |
| 30 then ten80 / 12 then ten80 | MalformedVarint | 10 |

Also apply literal structural table directly as parent tails, without child wrap,
after `10 00`: same kind/localoffset+2. Parent field0 outranks wrongwire. Parent
late structural error outranks earlier malformed selected child. Wrong-wire scalar
body bytes remain opaque, not reinterpreted as Type. No catch_unwind, no panic/OOM
forcing or expectation generation from the implementation.

## Full raw scalar/opaque compatibility matrices

For each parent scalar tag08/10/28/38 independently use literal values:
00 / 80 00=>0; ff ff ff ff 0f=>-1; ff ff ff ff ff ff ff ff ff 01=>-1;
ff ff ff ff ff ff ff ff ff 7f=>-1; 80 80 80 80 08=>MIN;
ff ff ff ff 07=>MAX; 80 80 80 80 10=>0;
80 80 80 80 80 80 80 80 80 02=>0.
Ensure a separate name `10 00` for non-name scalars; name cases use the scalar
itself. Include duplicate0/1/0, and wrong-wire1/2/5 before+after recognized fields:
only recognized last value persists; wrong-wire-only name =>MissingName(len).
Parent inline tags22/32 wrongwire0/1/5 alone/before/after valid body produce no
occurrence; supported unknown fields wires0/1/2/5 never decoded internally.
Selected scalar IDs and inline bodies coexist in both orders, e.g.
`10 00 28 01 22 00 38 02 32 00` and
`10 00 32 00 38 02 22 00 28 01`: name0/IDs1,2; first types field4span6..6,
field6span10..10; second field6span4..4,field4span8..8; both headersZ.
Duplicate IDENTICAL bodies `10 00 22 02 30 04 32 02 30 04 22 02 30 04`
=>field4span4..6,field6span8..10,field4span12..14, each class4, distinct pointers.
Duplicate singular children remain separate; no collapsed/merged record.

For each child scalar tag08/18/20/30/38/40/48/58/60/70 use inherited literal
TypeHeader cases in NEW wrapper-focused tests without changing existing fixtures.
Integer values above are low32 signed. Bool18 uses full inherited low64!=0:
80 80 80 80 10=>true (low32zero but low64nonzero);
80 80 80 80 80 80 80 80 80 02=>false; 02=>true, 00=>false.
Duplicate0/1/0, wrongwire1/2/5 before/after don't clear selected presence.
Classifiers6/7/9/12 all coexist without a winner. Existing all_scalars literal
checks every selected Header field; unselected nested field2/5/10/13 remain opaque.
No alias/type/name index/UTF8/semantic validation introduced.

## Tight charge trace and aggregate growth

`10 00 22 00`: parentrecords2/nodes2/depth2/ownedP+S/work11+P+S.
After parent, spent8+P; rootnode adds1 at0; occurrence adds1 at4; childnode adds1
at4; empty child has no wire charge; typed reservation addsS at4.
With other limits sufficient: work8+P=>work0(root); work9+P=>work4(occurrence);
work10+P=>work4(childnode); work11+P=>work4(typedreservation).
ownedP-1=>owned_bytes2(PARENT staging); ownedP=>owned_bytes4(TYPED growth).
ownedP/work11+P=>owned_bytes4 before work4. records1=>records2(parent);
nodes0=>nodes0(root); nodes1=>nodes4(child); depth0=>depth0(root);
depth1=>depth4(child). records2/nodes1=>nodes4, not child record failure.
Root/merge/node/push errors already parent-relative, never rebased again.

`10 00 22 00 32 00`: exactrecords3/nodes3/depth2/owned2P+2S/work18+2P+2S.
Parent spent12+2P. root@0; comparison@4; occurrence@4; childnode@4; pushS@4;
then occurrence@6; childnode@6; pushS@6. Comparison is charged even though both
bodies empty; no comparison once one list exhausted. Work13+2P=>work4(compare),
work14+2P=>work4(occurrence), work15+2P=>work4(node),
work16+2P=>work4(push). owned2P+S=>owned_bytes6(second typed growth).
The same-field `10 00 22 00 22 00` has H0/work17+2P+2S, proves no blanket K-1
charge. Both fields separately obey same one-sided rule.

For K in1,2,3,4,5,8,9, parent `10 00` followed by K literal `22 00` (separately
K literal `32 00`): output K empty headers, spans(4+2i)..same for i0..K-1.
N2+2K/F1+K/records1+K/nodes1+K/depth2/H0;
owned Q(K)*(P+S); work5+6K+Q(K)*(P+S).
Exact and one-less budgets; staging growth1/2/3/5/9 occurs at parent tag2+2i,
typed growth same index body4+2i. Work-1 fails last body2+2K. Owned-1 fails typed
last-growth body4 forK1, else4+2*(Q(K)/2). No refund enables typed reservation.

Also for k in1,2,3,4,5,8,9 construct literal block4^k then6^k, and reverse6^k4^k:
K2k,n4=n6=k,Hk, output encoded block order; spans4+2i; owned2Q(k)P+Q(2k)S;
records1+2k/nodes1+2k/depth2/work5+13k+owned.
This reaches BOTH staging directory growth boundaries with one combined typed
vector. Unequal blocks4^9 then6^5 have K14/H9/owned24P+16S/work98+owned;
reverse6^5 then4^9 has K14/H5/work94+owned. All-body tuplesZ/spans4+2i.
Alternating field4/6 for totalK in1,2,3,4,5,8,9,17,18 checks
H=max(K-1,0); owned[Q(ceilK/2)+Q(floorK/2)]P+Q(K)S;
work5+6K+H+owned, nodes1+K/records1+K, spans4+2i. All data fixed from literal
repetition/field identities, never expected from a decoder-produced directory.
No length-derived preallocation permitted to alter frozen growth/failure semantics.

## Hard caps and finite hostile cases — FUTURE only

MAX_BYTES+1 bytes fail bytes0 before other0 limits. Caller usize::MAX cannot raise
caps. 65535 copies literal `10 00`: N131070/F65535/noTypes/name0,
records65535/nodes1/depth1/owned0/work262141, exact succeeds; append one more
`10 00`: parent records failure at131070 before root. This witnesses record clamp
without storage/node dominance. Hard depth remains2; smaller0/1 tests above.
Nodes ceiling65535 and hard work64Mi/owned16Mi are statically clamped by unchanged
Budget; no independent runtime witness is promised where bytes/records/storage
or node bounds dominate. Small exact/minus-one witnesses test operational debit
semantics. No manufactured hard-work overflow, allocator-failure/OOM or RSS claim.

Prospective deterministic hostile tests with Limits bytes32/records16/nodes8/
depth2/work256/owned4096: all a,b0..255, test `[a,b]` and BOTH valid-name selected
wrappers `[10,00,22,02,a,b]`, `[10,00,32,02,a,b]`; every a0..255 repeated n0..15
as unwrapped body. 200704 finite inputs, twice each=401408 calls. Equal complete
Results; success full raw/pointer identity, bounded ordered spans, field4/6 only,
header.raw within parent; errors offset<=len. Wrapped child failures lie4..6;
parent staging allocation failure offset2 and typed allocation failure offset4
are permitted without forcing either. Exhaustive pair expectations are bounded
invariants fixed BEFORE implementation, not implementation-derived golden output.
No timing, RSS, fuzz framework, panic catcher, JVM/compiler/oracle or local runtime.
Execution requires later root admission and exact original hosted Linux CI.

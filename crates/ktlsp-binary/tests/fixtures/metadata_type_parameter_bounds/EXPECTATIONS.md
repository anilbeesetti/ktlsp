# Frozen literal expectations v1 — authored before source/runtime

All hex fixtures are authored, not compiler/oracle observations. None is produced
by an encoder/decoder under test. None establishes genuine positive coverage.
All expected values below were derived from source contract and bytes, before any
implementation. Later tests assert COMPLETE structs/tuples, not lengths alone.
N=None. O(v)=Some(v). Parent tuple order is
(id,name,reified,variance,upper_bound_id,upper_bound). Type tuple order follows
existing TypeHeader exactly:
(flags,nullable,flexible_type_capabilities_id,class_name,type_parameter,
 flexible_upper_bound_id,type_parameter_name,outer_type_id,type_alias_name,
 abbreviated_type_id).
Z=(N,N,N,N,N,N,N,N,N,N). A span is half-open BODY offsets in original parent.

## Positive literals

| fixture | length | complete parent scalars/IDs | ALL ordered bounds: span -> full Type tuple |
|---|---:|---|---|
| minimal.hex | 4 | (0,0,N,N,[]) | [] |
| selected.hex | 59 | (-1,-2,O(true),O(Out),[-1,7,0]) | [18..46 -> (O(-1),O(true),O(3),O(4),O(5),O(6),O(7),O(8),O(9),O(10)), 48..48 -> Z] |
| duplicates.hex | 28 | (1,2,O(false),O(Out),[]) | [16..24 -> (N,O(false),N,O(2),N,N,N,N,N,N), 26..28 -> (N,N,N,O(3),N,N,N,N,N,N)] |
| opaque.hex | 18 | (0,0,N,N,[]) | [6..18 -> Z] |
| wrong_wire.hex | 25 | (0,0,N,N,[]) | [21..25 -> Z] |

Raw output equals ENTIRE parent bytes and out.raw.as_ptr()==bytes.as_ptr().
Every header.raw equals bytes[span] and its pointer equals bytes[span].as_ptr(),
including selected's empty body at48. Assert every span start<=end<=len; no copies,
rebasing to d1, merging, default synthesis or omission of empty bodies.
For complete struct comparisons construct expected header.raw from literal slice;
expected scalars above are literals, never computed with a product decoder.

selected child is precisely:
`08 ff ff ff ff 0f 18 80 80 80 80 10 20 03 30 04 38 05 40 06 48 07 58 08 60 09 70 0a`.
Its full64 nullable value is2^32, true even though low32=0. Parent signed IDs use
low32 (not zigzag); child classifiers independently coexist. Empty second child
is ALL None and is retained separately. Parent packed ID bytes `07 80 00` append
7,0 AFTER unpacked-1. Unknown variance3 leaves preceding Out. duplicates keeps
class2 despite wrong-wire field6 `32 00`, and separate class3 stays separate.
opaque has malformed deeper field2,5,10,13 bodies80,00,ff,80: all stay opaque,
with raw bytes preserved and no child traversal/depth charge. wrong_wire parent
id/name/reified/variance wrong-wire2 plus field5/wire0 and field6/wire5 do not
alter data or append IDs; child field6/wire2 and field3/wire2 keep all None.

Additional finite positive literals (all have base `08 00 10 00` unless stated):
- base + `28 01`: no bound/IDs; parent(0,0,N,N), raw full6 bytes.
- base + `30 01 32 02 02 03 30 04 32 00`: IDs[1,2,3,4], no inline bounds;
  every other parent scalar as base. No default TypeHeader synthesized.
- base + `32 05 ff ff ff ff 0f`: IDs[-1], no bounds.
- base + `2a 03 12 01 80`: single span6..9 -> Z; deeper invalid80 opaque.
- base + `2a 02 30 01`: single span6..8 -> (N,N,N,O(1),N,N,N,N,N,N).
- parent `08 80 80 80 80 10 10 00`: (0,0,N,N,[],[]), low32 id0.
- parent `88 00 80 00 90 00 81 00`: (0,1,N,N,[],[]), nonminimal tags/values.
- base + `18 80 80 80 80 10`: reified O(true), all other fields base.
- base + `18 80 80 80 80 80 80 80 80 80 02`: reified O(false), raw64 inherited slow path.
- base + `2a 0b 18 80 80 80 80 80 80 80 80 80 02`: span6..17,
  (N,O(false),N,N,N,N,N,N,N,N), inherited raw64 slow path.

## Default-budget errors — exact parent-relative offsets

Error kind is existing type_parameter::ErrorKind. `W(x)` means Wire(x).
No catch-all prefix/error inference; following entries are literal expectations.

| complete hex | kind | offset |
|---|---|---:|
| empty | MissingId | 0 |
| 08 00 | MissingName | 2 |
| 10 00 | MissingId | 2 |
| 2a 01 80 | MissingId | 3 |
| 08 00 2a 01 80 | MissingName | 5 |
| 10 00 2a 01 80 | MissingId | 5 |
| 08 00 10 00 2a 01 80 | W(Truncated) | 7 |
| 08 00 10 00 2a 02 30 01 2a 01 80 | W(Truncated) | 11 |
| 08 00 10 00 2a 01 80 00 | W(InvalidFieldNumber) | 7 |
| 08 00 10 00 2a 01 80 30 | W(Truncated) | 8 |
| 08 00 10 00 2a 01 80 32 01 80 | W(Truncated) | 10 |
| 08 00 10 00 2a 01 80 0b | W(UnsupportedGroup) | 7 |
| 08 00 10 00 2a 02 01 | W(Truncated) | 7 |
| 08 00 10 00 2a 01 00 | W(InvalidFieldNumber) | 6 |
| 08 00 10 00 2a 01 07 | W(InvalidFieldNumber) | 6 |
| 08 00 10 00 2a 01 0b | W(UnsupportedGroup) | 6 |
| 08 00 10 00 2a 01 0c | W(UnsupportedGroup) | 6 |
| 08 00 10 00 2a 01 0e | W(InvalidWireType) | 6 |
| 08 00 10 00 2a 01 0f | W(InvalidWireType) | 6 |
| 08 00 10 00 2a 01 30 | W(Truncated) | 7 |
| 08 00 10 00 2a 03 32 02 01 | W(Truncated) | 9 |
| 08 00 10 00 2a 06 32 80 80 80 80 08 | W(NegativeLength) | 7 |
| 08 00 10 00 2a 0b 30 80 80 80 80 80 80 80 80 80 80 | W(MalformedVarint) | 16 |
| 08 00 10 00 2a 01 80 2a 01 00 | W(Truncated) | 7 |

These distinguish complete-parent-tail-before-child, packed-parent faults, required
id-before-name, once-only child offset rebase, first child wins and no following
container/body bleed. Last malformed varint offset is sixth body-start6+tenth
continuation index10=16. Existing raw TypeParameter::decode still accepts parent
base+`2a 01 80` with opaque body/span6..7; new composition rejects it at7.

Exact prefixes of minimal `08 00 10 00`: len0 MissingId@0; len1 W(Truncated)@1;
len2 MissingName@2; len3 W(Truncated)@3; len4 full minimal success.
Exact prefixes of base+`2a 00 2a 00`:
len0..4 as minimal; len5 W(Truncated)@5; len6 success (one bound6..6 -> Z);
len7 W(Truncated)@7; len8 success (bounds6..6,8..8 -> Z). Parent fields unchanged.
Every proper prefix of child-malformed vector base+`2a 01 80`: len0..4 as above;
len5 W(Truncated)@5; len6 W(Truncated)@6; len7 W(Truncated)@7.

## Aggregate minima and one-less limits

S=size_of::<type_parameter::UpperBound>(), T=size_of::<SelectedUpperBound>(),
I=size_of::<i32>()=4. No numeric host struct-size measurement; later tests use
these two sizes solely for expected capacity arithmetic. All other counts fixed.
Minima are tuples(bytes,records,nodes,depth,work,owned_bytes). Setting all exact
minima simultaneously MUST succeed with full values and pointers above.

| vector | exact minima |
|---|---|
| minimal | (4,2,1,1,9,0) |
| base+2a00 | (6,3,2,2,14+S+T,S+T) |
| base+2a023001 | (8,4,2,2,20+S+T,S+T) |
| selected | (59,20,3,2,134+2S+2T+16,2S+2T+16) |
| duplicates | (28,14,3,2,69+2S+2T,2S+2T) |
| opaque | (18,7,2,2,46+S+T,S+T) |
| wrong_wire | (25,11,2,2,53+S+T,S+T) |
| base + five 2a00 | (14,7,6,2,34+8S+8T,8S+8T) |
| base + five 3000 | (14,7,1,1,66,32) |

Derivations: selected parent N59/F8/P2/B3/U1, child C28/H10, nodes3;
owned2S+2T+16 => work59+16+3+4+1+28+20+3+owned=134+owned,
records8+2+10=20. duplicates N28/F9/C10/H5/nodes3 =>69+owned.
opaque N18/F3/C12/H4/nodes2 =>18+6+12+8+2=46 before capacity.
wrong_wire N25/F9/C4/H2/nodes2 =>25+18+4+4+2=53.
Five empty bounds stage logical slots1+1+2+4=8, typed same, no child fields.
Five IDs consume capacity8*4, U5:14+14+5+1+32=66.

Single-axis cuts below use otherwise DEFAULT limits, unless explicitly stated.
All return W(Limit(label)) at the listed offset. `min-1` lowers only named axis.

| vector | cut | label | offset |
|---|---|---|---:|
| minimal | bytes3 | bytes | 0 |
| minimal | records1 | records | 2 |
| minimal | nodes0 | nodes | 0 |
| minimal | depth0 | depth | 0 |
| minimal | work8 | work | 2 |
| minimal | work0 | work | 0 |
| base+2a00 | records2 | records | 4 |
| base+2a00 | nodes1 | nodes | 6 |
| base+2a00 | depth1 | depth | 6 |
| base+2a00 | owned S-1 | owned_bytes | 4 |
| base+2a00 | owned S+T-1 | owned_bytes | 6 |
| base+2a00 | work13+S+T | work | 6 |
| base+2a023001 | records3 | records | 6 |
| selected | bytes58 | bytes | 0 |
| selected | records19 | records | 44 |
| selected | nodes2 | nodes | 48 |
| selected | depth1 | depth | 18 |
| selected | owned 2S+2T+15 | owned_bytes | 48 |
| selected | work133+2S+2T+16 | work | 48 |
| duplicates | records13 | records | 26 |
| duplicates | nodes2 | nodes | 26 |
| duplicates | owned2S+2T-1 | owned_bytes | 26 |
| duplicates | work68+2S+2T | work | 26 |
| opaque | records6 | records | 15 |
| opaque | work45+S+T | work | 6 |
| opaque | ownedS+T-1 | owned_bytes | 6 |
| wrong_wire | records10 | records | 23 |
| wrong_wire | work52+S+T | work | 21 |
| wrong_wire | ownedS+T-1 | owned_bytes | 21 |
| five empty bounds | records6 | records | 12 |
| five empty bounds | nodes5 | nodes | 14 |
| five empty bounds | owned8S+8T-1 | owned_bytes | 14 |
| five empty bounds | work33+8S+8T | work | 14 |
| five IDs | owned31 | owned_bytes | 12 |
| five IDs | work65 | work | 12 |

Node depth wins nodes/work: base+2a00 with depth1,nodes1,work>=14+S+T returns
depth@6. With default depth, nodes1 and ample work returns nodes@6. Empty parent
with nodes0 returns nodes@0 before MissingId; default limits returns MissingId@0.
Byte limit0 on nonempty base wins depth0/nodes0/work0 at0. Parent stage S-1 storage
fault precedes any malformed child in base+2a0180, at parent tag4. No reset at
child or packed-ID boundary is allowed; aggregate exact record minima test it.

Growth with first2,3,5 empty bounds checks capacities2,4,8 for BOTH directories:
base+2*(2a00): N8/F4/nodes3/owned2S+2T/work19+owned;
base+3*(2a00): N10/F5/nodes4/owned4S+4T/work24+owned;
base+5*(2a00): as table. Parent staging is cumulatively charged even when freed;
final returned directory is typed only. IDs capacities2,4,8 checked analogously;
no implementation vector capacity is used as an expected budget oracle.

## Hard clamps and finite hostile scope (future native tests only)

Limits all usize::MAX: minimal/selected still return full expected tuples;
depth is clamped2 and selected succeeds, no deeper traversal. MAX_BYTES+1 literal
zero bytes must Limit(bytes)@0 before node. Base plus65,533 field7/wire0 `38 00`
unknown fields has bytes131070,F65535,records65535,nodes1,depth1,owned0,
work262141 exactly; full parent0/0/N/N,[] IDs and[]bounds. Append one more3800:
caller usize::MAX still Limit(records)@131070. This is finite hard-record witness.
Hard nodes65,535 cannot be isolated on successful inputs (parent must spend at
least n+2 records and nodes=1+n); hard work/owned ceilings are inspected in the
unchanged Budget constructor, with tightened work/owned runtime witnesses above.
Do not fabricate independent hard-cap successes/failures for unreachable axes.
No allocation-failure/OOM/32-bit-platform runtime witness claimed.

Later deterministic hostile test limits(bytes32,records8,nodes4,depth2,work256,
owned512): each ordered a,b in0..=255, check [a,b] and base+[a,b]; each a and
n in0..32 check n repeats of a. Compare two same-call results for determinism;
success MUST assert raw equality/pointer and every header span/raw/pointer;
error MUST have offset<=input.len, never panic. This finite set is not fuzzing
completeness/crash-proof/performance evidence. No product/runtime execution now.

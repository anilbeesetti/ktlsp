# Frozen acceptance details — UNEXECUTED

All JSON literals have complete field-boundary prefix partitions. For each prefix
not a parent-field boundary, Truncated@prefix_length (these success literals use
valid tags and bounded lengths). At complete boundaries before recognized name,
MissingName@length; all remaining listed boundaries succeed with parent scalars
last-complete-field wins and selected children whose full fields have completed.
Check actual literal child headers (all unlisted Type scalars None), fieldnumber,
body spans, ordering and raw pointer identity, including empty bodies. Prefix
expectations are literals, never decoder-produced. Exact and independently +1
limits succeed; bytes L-1 fails Wire(Limit(bytes))@0. Each JSON deficit changes only
that one limit from exact. No-child owned0 succeeds; depth0/nodes0 fail at0.

## Small resource sequencing

Literal `10 00 1a 00`: L4,F2,K1,M0,G0; R4,Q2,D2,O=S,W18+S.
Default other bounds: bytes3 with all0 -> bytes@0; depth0 ->depth@0;
depth1/nodes0 ->nodes@0; depth2/nodes1 ->nodes@4;
records0 ->records@0 first pass; records1 ->records@2 first pass;
records2 ->records@0 rescan; records3 ->records@2 rescan;
records3/depth1 ->records@2 (rescan before childdepth);
work0 ->work@0 root; work1 ->work@0 first record;
work2 ->work@0 first tag; work3 ->work@1 first value;
work4 ->work@0 first dispatch; work5 ->work@2 first-pass second record;
work6 ->work@2 second tag; work7 ->work@3 length;
work8 ->work@2 dispatch; work9 ->work@0 rescan name record;
work10 ->work@0 name tag; work11 ->work@1 value;
work12 ->work@0 dispatch; work13 ->work@2 selected record;
work14 ->work@2 selected tag; work15 ->work@3 length;
work16 ->work@2 dispatch; work17 ->work@4 child node;
work18 and ownedS ->work@2 push growth; work18+S succeeds.
work18 and ownedS-1 ->owned_bytes@2 (owned before growthwork).
Malformed selected child `10 00 1a 01 00` with owned0/depth2 ->
InvalidFieldNumber@4; depth1 ->depth@4. Parent `10 00 1a 01 00 00`
->InvalidFieldNumber@5 before child, regardless owned0. Missing name body
`1a 01 00` ->MissingName@3 even owned0/depth1 (parent finishes first).

## Shared budget and growth

For k=1..5, literal name `10 00` followed by k copies `1a 00`:
N=2+2k,F=1+k,R=2+2k,Q=1+k,D2,O=C(k)S,W=9+9k+C(k)S.
O-1 fails owned_bytes at tag [2,4,6,6,10] for k1..5.
W-1 with exact O fails work at [2,4,6,10,10]; k4 ends at child node@10.
Q-1 fails last childbody 2+2k; R-1 fails last rescan tag2k.
Interleave tags1a/2a to prove ONE vector/balance/growth, not per-field lists.
Use k=3 exact O4S, owned3S fails at third growth@6; no payload copies.

Canonical distinguishing witness Unit: parent separately R3/work16, child
separately R1/work4 would pass, but composition R3 fails records@0 during rescan;
aggregate R7,Q2,D2,O=S,W38+S passes. Aggregate records6 fails final flags@6;
work37+S fails final flags dispatch@6 AFTER child storage, ownedS-1 fails@2.
No per-child cap reset can satisfy these assertions.

For no-child minimal `10 00`: R2,Q1,D1,O0,W9. All zero limits =>bytes@0;
bytes2/depth0=>depth@0; bytes2/depth1/nodes0=>nodes@0;
bytes2/depth1/nodes1/work0=>work@0. Empty with permissive bytes/depth/nodes/work
=>MissingName@0; empty all0=>depth@0. Error precedence is explicit.

## Hostile and compatibility families

JSON malformed cases are literal default-bound errors with exact Function offsets.
For EACH selected tag1a/2a repeat child00,0b,0e,30,negative length, ten continuation
bytes, and nonminimal wrapper cases; preserve selector identity on success only.
For each child fault, prepend `10 00 1a 00` and shift child errors by4 (not twice).
Then append parent00: parent InvalidFieldNumber at original full length wins.
Remove name or use wrong-wire name12 00: MissingName at complete-parent length.
Structural parent tag/length/read failure wins regardless present/missing name.

For EACH selected field3/5, wire0/1/5 is wrong-wire opaque, neither an occurrence
nor a scalar default. Test before AND after wire2 empty and valid nonempty child.
Wire2 malformed child is examined; malformed bytes in all unselected wire2 parent
fields stay opaque. Type nested Argument/Type bodies00/80/0b remain opaque at
depth2, without depth3 or required nested-field checks. Empty children are nodes
and vector elements, never skipped. Child scalar duplicates last-win only within
that occurrence; separate Type bodies are never merged into each other.

Nonminimal/low32 parent aliases: `10 00 9a 80 80 80 10 80 80 80 80 10`
-> one field3 empty child span12..12, R4,Q2,D2,O=S,W34+S;
replace first9a withaa ->field5, same. Lowered records3 fails@2;
ownedS-1 fails@2; work33+S fails@2. Nonminimal length/body starts are computed
from full field end, never tag_start+2. Inherited varints that terminate at tenth
byte, negative signed indices and raw boolean full-u64 behavior are already pinned
by standalone helpers; compose those frozen Type literals in a selected wrapper
and assert the same exact scalar presence without a new wire policy.

Every successful fixture, all proper prefix errors, all malformed defaults, exact
minima/+1/one-less limits, storage boundaries and hostile families must become
meaningful integration assertions on later source grant. Deterministic small hostile
sweep: for each byte b=0..255 use input `[10,00,t,01,b]`, t=1a and2a. Before tests,
freeze outcomes by wire classes: b>>3=0 ->InvalidFieldNumber@4; otherwise
wire3/4 ->UnsupportedGroup@4, wire6/7 ->InvalidWireType@4; remaining wires0/1/2/5
with b<128 ->Truncated@5 (missing value/length/fixed payload); b>=128 ->
Truncated@5 (continuing tag) regardless low bits. IMPORTANT precedence:
continuing tag is read before field/wire validation, so b>=128 ALWAYS Truncated@5.
Raw error outputs never exceed input len. Follow each failure with healthy Unit
input to prove no persistent budget or output state. No random fuzz runtime now.

## Hard-limit clamping (later hosted tests only)

All usize::MAX limits behave as defaults; nodes/depth0 root errors unaffected by
huge other limits. Prefix a name then append 32767 unselected `18 00` fields:
F32768,L65536; two scans R65536 exceed hard65535, fail records at last rescan tag
65534 even requested records=usize::MAX; F32767 (32766 suffixes) succeeds.
Use unselected field6/wire2 (tag32) body of MAX_BYTES-8 zeros: input name2+tag1+length4+body =
MAX_BYTES-1, two flat fields, no selected nodes/storage, huge/default bytes pass;
append two scalar bytes -> MAX_BYTES+1 fails bytes@0 even usize::MAX. Child
contents opaque, no actual metadata/compatibility claim. Do not claim hard-work
boundary independently reachable when other hard caps stop it. Existing Budget
constructor min/checked-sub proves nodes/depth/work/owned hard-clamp wiring; test
small caller cuts and symbolic S storage-growth, do not invent ABI/platform limits.

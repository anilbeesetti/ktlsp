# Independent acceptance freeze — all product assertions UNEXECUTED

JSON literals are authored before source. All unlisted Type scalars None. Assert
full parent/child tuples, exact body spans/order, raw byte equality AND pointer
identity for parent and every child, including empties at exact starts. Prefixes
0..L partition into literal successes, MissingFqName boundaries, or Truncated@n.
At successful prefixes retain only complete selected fields; flags/fq_name reflect
last complete recognized fields. In duplicates, fq_name0 through prefix16 then2
at18; flagsNone until16 thenSome7. In Unit flagsSome326 for all successful
prefixes; fq_name0. Other successful prefixes have each row's parent tuple.
For every success assert exact limits and independently +1 limits succeed, bytes
L-1 ->Wire(Limit(bytes))@0; each one-less resource changes ONLY that limit from
exact, with JSON's kind/offset. owned0 for minimal succeeds (no negative deficit).
No budget output state survives return; follow each failed call with healthy Unit.

## Genuine held-artifact integration, prospective native assertion

The genuine test must consume actual unchanged
`include_bytes!("../fixtures/t017c2/Unit.class")` via existing Rust
`ktlsp_binary::parse_class`, `annotations::decode`, `metadata::decode` and
`payload::decode`, each with its own existing bounds. Require
`Outcome::Decoded` header k1/mv[2,3,0], decoded d1 length52 and the frozen bytes;
require actual payload_span19..52 and payload bytes equal unit_class.hex.
Invoke the NEW `class_supertypes::decode` on that ACTUAL borrowed payload,
with exact L33/R13/Q2/D2/O=S/W96+S. Assert actual header flagsSome326/fq_name0,
header.raw pointer-identical to actual decoded d1[19..52]; one selected child
span7..9, class_nameSome1/all other Type scalarsNone, raw30 01 and pointer-identical
to actual Class[7..9] / actual d1[26..28]. Assert full tuples and bytes as well
as pointers. Retain parsed Class/annotations/d1 owners until all assertions finish.
Never replace this pipeline with the hex literal or a manufactured header.
The 33-byte literal test is a separate small exact-resource control; it cannot
by itself establish genuine artifact consumer coverage. No resolver/name meaning,
inheritance, ID reconciliation, compiler/JVM/oracle invocation or whole-pipeline
aggregate memory claim. This native assertion remains UNEXECUTED in preparation.

## Empty-child sequencing and competing failures

Literal `18 00 32 00`: L4,F2,K1,M0,G0; R4,Q2,D2,O=S,W18+S.
Default other limits: bytes3/all0 ->bytes@0; depth0 ->depth@0;
depth1/nodes0 ->nodes@0; depth2/nodes1 ->nodes@4;
records0 ->records@0; records1 ->records@2 first scan;
records2 ->records@0 rescan; records3 ->records@2 rescan;
records3/depth1 ->records@2 BEFORE childdepth.
work0 ->work@0 root; work1 ->work@0 first record;
work2 ->work@0 first tag; work3 ->work@1 first value;
work4 ->work@0 first dispatch; work5 ->work@2 second record;
work6 ->work@2 second tag; work7 ->work@3 length;
work8 ->work@2 dispatch; work9 ->work@0 rescan fq record;
work10 ->work@0 tag; work11 ->work@1 value; work12 ->work@0 dispatch;
work13 ->work@2 selected record; work14 ->work@2 tag;
work15 ->work@3 length; work16 ->work@2 dispatch;
work17 ->work@4 child node; work18/ownedS ->work@2 growth;
work18/ownedS-1 ->owned_bytes@2 (owned before growthwork).
work18+S/ownedS succeeds. All Limit labels wrap Class ErrorKind::Wire.

`18 00 32 01 00` with owned0/depth2 ->InvalidFieldNumber@4;
depth1 ->depth@4; node1 ->nodes@4 before child. Its parent scan costs9 plus
root1: records3 ->records@2 rescan before child, records4 ->records@4 child;
work20 ->work@4 child record, work21 ->work@4 child tag;
work22 with owned0 ->InvalidFieldNumber@4. First parent scan must complete.
`18 00 32 01 00 00` ->parent InvalidFieldNumber@5 even owned0/depth1.
`32 01 00` ->MissingFqName@3 even owned0/depth1; `1a 00 32 01 00` ->MissingFqName@5.
Root limits precede missing-fq check; missing-fq follows ALL parent wire errors.
Empty permissive input ->MissingFqName@0; empty all0 ->depth@0.
Minimal `18 00`:R2,Q1,D1,O0,W9. bytes1/all0 ->bytes@0;
bytes2/depth0 ->depth@0; bytes2/depth1/nodes0 ->nodes@0;
bytes2/depth1/nodes1/work0 ->work@0. depth1/owned0 otherwise exact succeeds.

## Aggregate witness, vector growth and clamps

Unit standalone parent R6/W45 and child R1/W4 are separately successful cuts,
but aggregate records6 fails records@0 rescan. Aggregate R13,Q2,D2,O=S,W96+S
succeeds. records12 fails final parent module tag30; work95+S fails its dispatch@30
AFTER child storage; ownedS-1 fails selected tag5; depth1/nodes1 fail childbody7.
No per-child budget reset satisfies this conjunction. Header/d1/payload extraction
remain separate budgets: no whole pipeline memory/performance claim.

For k1..5: `18 00` then k copies `32 00`. L2+2k,F1+k,R2+2k,Q1+k,D2,
O=C(k)S,W9+9k+C(k)S. Owned O-1 fails at tags[2,4,6,6,10]; W-1 with exactO
fails at[2,4,6,10,10], with k4 failing last child node10 (NO growth). NodesQ-1
fails last childbody2+2k; recordsR-1 last rescan tag2k. Assert k3 needs4S;
owned3S fails third growth@6; k4 uses same4S; k5 needs8S. This proves ONE vector,
logical1/1/2/4 growth and encoded order/empty bodies without layout guesses.

All usize::MAX requests clamp to defaults, verified later against small successful
cases and below hostile hard-record/bytes witnesses. Parent fq `18 00` followed
by32767 unselected `08 00`: F32768,L65536,R65536 exceeds hard65535; fail records
at last rescan tag65534, even requested usize::MAX. With32766 suffixes F32767
succeeds. Unselected field5/wire2 `2a` body MAX_BYTES-8 zero bytes with four-byte
length: fq2+tag1+length4+body =MAX_BYTES-1, F2,K0. Huge/default bytes pass;
append `08 00` ->MAX_BYTES+1 ->bytes@0 even usize::MAX. Body stays opaque.
Depthusize::MAX behaves as2 (nested opaque Type content does not require3).
Existing constructor min/checked-sub proves work/nodes/owned clamp wiring;
exercise caller-tightened limits and growth, do not invent independently reachable
hard-work/node/storage or RSS/CPU/ABI thresholds blocked by other hard caps.

## Hostile byte and scalar families

Every JSON malformed row has exact default-bound Class-relative kind/offset.
For each child fault prepend `18 00 32 00` to the child-containing parent and
shift every child offset+4 exactly ONCE; append parent00 and parent error at the
previous full length wins. Remove fq field or replace it by wrong-wire1a00:
MissingFqName@complete parent length, unless parent malformed first.

Selector field6 wires0/1/5 are opaque and never occurrences. Test each before/after
empty and valid wire2 bodies; no presence erasure. Class flags/fq wrong wires and
signed/duplicate cases preserve standalone semantics. IDs field2 wire0/2 coexist
with inline supertype; packed00/80/ff stay opaque even if malformed.
Unselected parent nested fields5/8/9 and selected Type nested2/5/10/13/100 bodies
00/80/0b/ff remain opaque at depth2, with no nested required-field validation.

Wrap existing Type-header frozen scalar matrix in field6/wire2, preserving its
literal outcomes: each selected scalar1/3/4/6/7/8/9/11/12/14 values0/1 and0->1->0;
wrong-wire1/2/5 alone/before/after recognized scalar leaves expected presence;
all four classifiers coexist. Integer values `ff ff ff ff 0f`, ten-byteff..01,
ten-byteff..7f ->-1; `80 80 80 80 08` ->i32MIN; `ff ff ff ff 07` ->i32MAX;
`80 80 80 80 10` and `80 80 80 80 80 80 80 80 80 02` ->0. Nullable full64
for first latter value is true, second latter false (low64), never low32 truth.
Signed Class fq/flags use same integer cases; parent fq required never defaulted.
Existing Type-header EXPECTATIONS.md is pinned by PROVENANCE.json; no new wire policy.

Nonminimal JSON body starts8 and low32-alias body starts12 prove span derives
from end-body.len, not tag_start+2. Nonminimal raw tags/lengths preserved exactly.
For selected parent tag32/child selected scalar30, continuation counts1..9 truncate
at supplied end; count10 MalformedVarint at tenth continuation. Unknown parent
wire0/1/2/5 fields retained, no unknown-field allocation. Field0 precedes wire
validation; continuing tag byte is read BEFORE field/wire checks.

Deterministic sweep before later runtime: input `[18,00,32,01,b]`, b0..255
(hex notation). If b>=128 ->Truncated@5 regardless low bits. Else if b>>3=0
->InvalidFieldNumber@4; else wires3/4 ->UnsupportedGroup@4, wires6/7
->InvalidWireType@4; remaining wires0/1/2/5 ->Truncated@5 (missing value/payload).
Repeat after empty sibling (+2 offsets), with later parent00 (parent wins), and
then healthy Unit input; deterministic results, no panic, offsets<=input.len.
All these are prospective meaningful native integration assertions, never a claim
of executed fuzzing, compiler compatibility, recursive/semantic inheritance or parity.

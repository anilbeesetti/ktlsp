# Frozen independent literal expectations (TEMP, unexecuted Rust)

EXPECTATIONS.json is the exact literal result/cut matrix. Do not derive expectations
from decoder output or a new encoder. All input/raw output slices must compare bytes
AND pointers, order, scalar Options and every span. Full tuple order is flags,name,
type_id,vararg_element_type_id. For empty input check raw pointer too.

B=sizeof(ValueParameterBody), T=sizeof(SelectedValueParameter), I=sizeof(i32).
All formula layout terms are eventual target size_of, not ABI promises. Existing
push logical capacity charges, including temporary opaque directory, are exact.

fixture       N  parentF packedK packedL IDs childN childF records owned                work
empty         0  0       0       0       0   0      0      0       0                    0
selected      21 4       2       3       3   8      4      10      B+4I+T               53+owned
duplicates    39 7       2       2       3   20     9      18      2B+4I+2T             98+owned
opaque        12 1       0       0       0   10     4      5       B+T                  32+owned
three         12 3       0       0       0   6      3      6       4B+4T                30+owned
pair          25 4       0       0       0   12     4      8       2B+2T                53+owned

For each row exact minima together SUCCEED. Tighten just one bound, keeping others
at exact minimum: bytes N-1 -> Wire(Limit(bytes))@0; records-1 -> Wire(Limit(records))
at selected10/duplicates27/opaque10/three10/pair14; owned-1 -> Wire(Limit(owned_bytes))
at selected4/duplicates25/opaque2/three10/pair12; work-1 -> Wire(Limit(work)) at same
last typed-push offset. Empty with allzero succeeds; no negative-bound test.

Growth: three opaque pushes len0/1/2 charge B,B,2B at parent tags0/4/8; typed pushes
len0/1/2 charge T,T,2T at body starts2/6/10. At last push owned 4B+4T-1 fails@10;
4B+2T fits exactly two typed entries but cannot admit third. Version vector in
selected charges I,I,2I at packed offsets15/16 and unpacked tag18. Parent ID charges
finish BEFORE any child; limits owned B+4I-1 fail@18 even with valid child; owned
B+4I passes parent then fails typed@4; parent work37+B+4I passes parent, next child
record-work at4 fails; records parent6 pass parent then child record at4 fails.
Pair records6 admits parent4+firstchild2 then second child record at12 fails;
records7 admits second name but next child field at14 fails. This demonstrates
shared counters rather than independent child budget resets.

Exact simple budget event grid: input12 02 10 00, N4/F1/childN2/childF1;
records2/ownedB+T/work10+B+T succeeds.
records0 => Wire(Limit(records))@0; records1 => Wire(Limit(records))@2.
owned0 => Wire(Limit(owned_bytes))@0 (parent push);
ownedB => Wire(Limit(owned_bytes))@2 (typed push after successful child).
work6+B => Wire(Limit(work))@2 (child record-work);
work7+B => Wire(Limit(work))@2 (child tag read);
work8+B => Wire(Limit(work))@3 (child value read);
work9+B => Wire(Limit(work))@2 (child dispatch);
work10+B => Wire(Limit(work))@2 (typed reservation work).
All other bounds default, except use ownedB+T for these work probes.

Precedence beyond default faults in JSON:
- 12 00 with ownedB and work>=4+B: MissingName@2 wins any prospective typed push.
- 12 01 00 with ownedB: Wire(InvalidFieldNumber)@2 wins typed-owned failure.
- 12 00 08 00 with default limits: MissingName@2, not parent EOF4.
- 12 00 08 00 with records1: Wire(Limit(records))@2 wins child MissingName.
- 12 00 00: parent InvalidFieldNumber@2 wins missing child name.
- 12 01 00 08: parent Truncated@4 wins malformed child InvalidFieldNumber@2.
- 12 00 fa 01 01 80: parent packed Truncated@6 wins MissingName@2.
- 12 02 10 00 12 00 with owned2B+T: MissingName@6 wins second typed push.
- 12 00 with bytes1/records0/work0/owned0 => bytes@0 first.
- 12 00 with bytes2/records0/work0/owned0 => records@0 first.
- 12 00 with records1/work0/owned0 => work@0 before reads.
No parent offsets are rebased. Child local offsets exactly once add body.start;
child required MissingName is end of child, not constructor EOF (except final child).

Hard clamps (future Rust tests, NONE run now):
Use Limits with every field usize::MAX, compare to hard-default outcomes; existing
constructor::Limits is reused. MAX_BYTES+1 bytes => Wire(Limit(bytes))@0; no scan.
65535 repetitions08 00 => flagsSome0/records65535/work262140, no owned capacity;
append08 00 => Wire(Limit(records))@131070 despite caller usize::MAX.
32767 repetitions12 02 10 00 =>65534 records, work327670+32768(B+T),
owned32768(B+T), exactly32767 typed entries; append another => records@131070
in last child after full parent has succeeded. Set work/owned explicitly large
and assert equality with default-clamped outcomes rather than bypass hard caps.
Root bytes/records caps and aggregate successful work bound make hard64Mi work
and16Mi owned EXHAUSTION unreachable on normal 32/64-bit layouts for this shallow
shape; do not claim an exercised exhaustion. Clamp implementation source review
plus usize::MAX/default equivalence and caller-tightened exact minima test those
axes. No oversized output or allocator/RSS evidence is implied. OOM/allocation
failure remains static review only; no injected allocator/new framework.

Finite hostile plan under bytes32/records16/work4096/owned4096:
all65536 two-byte parents; all65536 two-byte child bodies inside literal parent
12 02 XX YY; repeat decode assert deterministic Result, success borrow/span/tuple
invariants and error.offset<=input.len, no panic. Add length0..32 deterministic
LCG byte strings (seed1, a=1664525,c=1013904223 modulo2^32, highbyte,256 strings
per length); literals cover malformed-varint/group/negative-length paths.
Prefix matrix has115 cases, every constructor cuts parent-first; arbitrary data
probe does NOT establish all-input safety, CPU bounds, upstream runtime equivalence,
semantic coverage or allocator failure. No local Rust probe in this phase.

Genuine fixture: all exact Pair tuples in JSON come from manually inspected bytes,
checked independently against committed class Metadata annotation CP89 and direct
d1 decoding by Python standard-library data checks; see PAIR_DATA_VERIFICATION.
The genuine fixture covers2 absent-flags/IDs scalar tuples, no version IDs.
Author literals cover presence/signed/duplicate/IDs/wrong-wire/opaque shapes.
No compiler provenance inference from distribution2.3.20. No new archive search.

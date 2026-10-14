# Independently authored and genuine expectation freeze v3

This file is written before source or product execution. No encoder/decoder under
test supplies expected values. S=size_of::<SelectedType>(); symbolic ABI only.
For EVERY success: parent.raw and header.raw match supplied bytes AND pointer
identity, types preserve encoded order, body spans within parent input, field
identity exactly3/4. Every unspecified TypeHeader Option is None, no defaults.
No partial result on errors. Wire errors below use the existing ValueParameter
ErrorKind::Wire; MissingName is the existing distinct variant.

## Fixtures (literal bytes frozen in fixtures/*.hex)

minimal: `10 00`, N2/F1/K0/C0/M0. name0; flags/IDs None; types[].
records2/nodes1/depth1/owned0/work9.

selected: `08 00 10 05 1a 04 18 01 30 02 22 02 38 03 28 07 30 08`.
N18/F6/K2/C3/M6. flagsSome0/name5/type_idSome7/vararg_idSome8.
types[(3,6..10,nullableSome(true),class_nameSome2),
       (4,12..14,type_parameterSome3)]. Both IDs uninterpreted despite inline Types.
records15/nodes3/depth2/owned2S/work75+2S.

duplicates: `10 01 22 02 38 01 1a 02 30 02 22 00 1a 04 30 03 30 04 10 02`.
N20/F6/K4/C4/M8. name2; flags/IDs None. types in exact order:
[(4,4..6,type_parameterSome1),(3,8..10,class_nameSome2),
 (4,12..12,allNone),(3,14..18,class_nameSome4)]. No merging across occurrences.
records16/nodes5/depth2/owned4S/work85+4S. Empty body raw pointer at parent[12..].

opaque: `10 05 1a 0c 12 01 80 2a 01 00 52 01 ff 6a 01 0b 3a 01 80 42 01 00`.
N22/F4/K1/C4/M12. name5/flagsIDsNone; types[(3,4..16,allNone)].
Type fields2/5/10/13 contain malformed opaque80/00/ff/0b; parent annotation7 and
default8 contain opaque80/00. No deeper scan/initialization/semantic check.
records12/nodes2/depth2/ownedS/work82+S.

wrong_wire: `10 05 18 01 20 02 1d 00 00 00 00 21 00 00 00 00 00 00 00 00 12 01 80`.
N23/F6/K0/C0/M0. name5; flags/IDsNone; types[]. Wrong-wire3/4 do not set a type;
wrong-wire name2 byte payload does not clear recognized name. Its inner80 opaque.
records12/nodes1/depth1/owned0/work71.

genuine Pair parameters: bytes `10 05 1a 02 38 00` and `10 06 1a 02 38 01`.
Each N6/F2/K1/C1/M2; flags/IDsNone, names5/6. types[(3,4..6,type_parameterSome0/1)].
records5/nodes2/depth2/ownedS/work26+S. Complete provenance independently checked
in PROVENANCE.json: Pair class/JAR, d1, constructor and notice hashes match.
d1 parameter spans82..88/90..96, Type d1 spans86..88/94..96; constructor-relative
parameter spans4..10/12..18. These are field3 positives ONLY, no genuine vararg.

## Every-byte prefix cuts (default sufficient budget)

minimal: cut0 MissingName0;cut1 Truncated1;cut2 success as above.
selected: cut0 MissingName0,cut2 MissingName2; success cuts4/10/14/16/18:
parent flags0,name5 from4; Type3 from10; Type4 from14; type_id7 from16; vararg_id8
from18. Truncated@cut for[1,3,5,6,7,8,9,11,12,13,15,17].
duplicates: cut0 MissingName0; success[2,6,10,12,18,20]. name1 until20 then2;
types become available at6/10/12/18 as listed above. Other cuts[1,3,4,5,7,8,9,
11,13,14,15,16,17,19] Truncated@cut.
opaque: cut0 MissingName0; success[2,16,19,22], one type from16; all other cuts
[1,3,4,5,6,7,8,9,10,11,12,13,14,15,17,18,20,21] Truncated@cut.
wrong_wire: cut0 MissingName0; success[2,4,6,11,20,23] same selected result;
all other cuts[1,3,5,7,8,9,10,12,13,14,15,16,17,18,19,21,22] Truncated@cut.
each genuine Pair: cut0 MissingName0; success2 name5/6 no types;success6 full;
cuts1/3/4/5 Truncated@cut. Total count includes both Pair inputs: 3+19+21+23+24+14=104.
Unframed complete prefixes may succeed; no container completeness claim.

## Finite selected/opaque matrices

Field4-only `10 00 22 02 38 01`: name0,type4 span4..6,type_parameter1;
same exact budget as Pair. Late name `1a 02 38 00 10 05`: name5,type3 span2..4,
type_parameter0; records5/nodes2/work26+S/ownedS; required name checked first.
For each tag1a/22 empty body after1000: type span4..4/allNone,
records4/nodes2/depth2/work18+S/ownedS. Name-only succeeds at depth1.
Both empty `1000 1a00 2200`: spans4..4/6..6, identities3/4,
records6/nodes3/work27+2S/owned2S. Both nonempty `1000 1a023800 22023801`:
spans4..6/8..10,param0/1,records8/nodes3/work43+2S/owned2S.

For each3/4, wire0/1/5 before and after baseline10051a023800 do not produce or
clear types. Wire2 is ALWAYS selected; malformed bodies cannot become opaque.
For each3/4 repeated empty/duplicate/split fields retain order, no merge.
Type body `300138023003` has class_name3 AND type_parameter2 (conflict accepted),
body length6/C3; field3/4 under1000 yields N10/F2/K1/records7/nodes2/work42+S.
Parent signed IDs and name: existing scalar fixture encodings reused:0/-1 via
5 and10 byte forms/i32MIN/i32MAX/nonminimal0/low32 overflow=>0; types unaffected.
Child all nine integer fields/bool reuse existing TypeHeader literal rules without
new exhaustive framework. A child nullable `18 80 80 80 80 10` is true (full u64).
Unknown fields100/max field number and wrong-wire child scalars remain raw.
Parent7/8/100 opaque malformed payloads never become selected children.

## Frozen first errors and exact rebasing

`1a 01 80` =>MissingName3 before child Truncated3.
`10 05 1a 01 80 00` =>parent InvalidFieldNumber5 before child Truncated5.
`10 05 1a 02 38 00 80` =>parent Truncated7 after otherwise valid child.
`10 05 1a 01 80` =>child Truncated5 (body4..5, local1).
`10 05 1a 01 80 28 00` =>child Truncated5 despite a valid following type_id0
parent field; child scan cannot read across its body end. Field4 substitution
`10 05 22 01 80 28 00` has the same first error and offset.
`10 05 1a 01 00` =>child InvalidFieldNumber4 (local0).
`10 05 22 01 0b` =>child UnsupportedGroup4 (local0).
`10 05 22 01 0e` =>child InvalidWireType4 (local0).
`10 05 1a 02 30 80` =>child Truncated6 (local2).
`10 05 1a 03 12 02 ff` =>child Truncated7 (local3, outer child framing valid).
`10 05 1a 06 12 80 80 80 80 08` =>child NegativeLength5 (local1).
`10 05 1a 0b 08` followed ten80 =>child MalformedVarint14 (local10).
`10 05 1a 0a` followed ten80 =>child MalformedVarint13 (local9/tag).
`10 05 22 00 1a 01 80` =>second child Truncated7 despite earlier empty child.
`10 05 1a 02 ff` =>PARENT Truncated5; child never visited.
For selected malformed child cases prefix parent by `08 00` then separately
`a0 06 00`; kinds unchanged and offsets add2/3. No double rebase. Also substitute
field4 for every field3 error case and vice versa: identical offsets/kinds.
Missing name without malformed wire remains at len; zero field-number outranks
bad wire, groups3/4 unsupported, wire6/7 invalid, as existing wire decoder.

## Exact limits and competing first failures

For each successful fixture: exact bytes/records/nodes/depth/work/owned and each+1
succeed; bytesN-1 plus all other0 =>bytes0 FIRST. usize::MAX clamps unchanged.
One-less records: minimal@0(second scan), selected@16(last second-pass ID),
duplicates@18(last second-pass name), opaque@19(last second-pass default),
wrong_wire@20(last second-pass wrongname), Pair@4(child tag).
One-less nodes/depth for any child fixture fail at last body start /first body
start respectively: selected12/6,duplicates14/4,opaque4/4,Pair4/4.
One-less work: minimal@0,selected@16,duplicates@18,opaque@19,wrong_wire@20,Pair@2.
One-less owned: selected@10(second child),duplicates@10(third child),opaque@2,Pair@2.
For no-child fixtures owned0/depth1/nodes1 succeed; depth0 or nodes0 fail at0.

Pair exact work trace with sufficient other limits:work0=>work0(root);
work1=>work0(first record);work10=>work2(parent dispatch);work11=>work0(second
scan record);work20=>work2(second dispatch);work21=>work4(child node);
work22=>work4(child record);work23=>work4(child tag);work24=>work5(child value);
work25=>work4(child dispatch);work26=>work2(storage reservation).
work26+owned0=>owned_bytes2 (owned precedes reservation work);records0+work1=>
records0;nodes0+depth0=>depth0. Pair records4+nodes1=>nodes4; records4+work22=>
records4(child record outranks its work). Empty input default=>MissingName0;
empty all0=>depth0, distinct from old public decoder MissingName0.

Aggregate growth: parent1000 plus alternating empty1a00/2200 repeated K=1..5.
N2+2K/F1+K/C0/M0. nodes1+K/records2+2K/ownedQ*S/work9+9K+Q*S.
K3 uses4S;K5 uses8S. One-less owned at tag2+2*(Q/2) for K3/5 =>6/10;
one-less work fails last child node at body2K+2 when last occurrence does not grow,
or final push tag2K when it grows. Both identities share one vector/budget.

Hard-record witness parent1000 then32766 empty field3/4 =>F32767,N65534,K32766,
records65534,nodes32767,owned32768S,work294903+32768S (subject to16MiB storage).
Add one empty occurrence: first pass32768 records, second pass allows32767, last
record fails Limit(records) at tag65534. No child-level new budget reset.
Hard-node bound may be dominated by record cap; validate static clamp plus small
exact cases. ByteMAX+1 fails bytes0 before root. Hard work is clamped; no broad
benchmark/forced hard-work/OOM/RSS witness. Finite hostile probe in future test:
all65536 two-byte inputs and each repeated byte0..255 lengths0..15 with limits
bytes32/records16/nodes8/depth2/work256/owned4096; equal outcomes, errors<=len,
successful borrows/order/spans checked. No broad fuzz harness, timing or RSS claim.
The unwrapped parent probes cannot reach selected children; ALSO freeze all65536
two-byte pairs (a,b) wrapped separately as `10 00 1a 02 a b` and
`10 00 22 02 a b`, with the same finite limits/invariants. Every wrapped input has
a fully valid outer parent/name and exactly one selected child body at4..6;
success returns field identity3/4 and parent name0, exact child raw/pointer/span;
child-decoding failure offsets lie4..6 and retain child wire/limit kind; the
specified storage Wire(Limit(allocation)) at parent tag2 is also permitted, with
no forced-OOM test. This checks the new
child path including malformed content; expectations are invariants fixed now,
not an output-derived list of golden decoder results.

All execution above is FUTURE and requires root grant. Preparation data hash/
literal checks establish provenance and arithmetic only, not runtime behavior.

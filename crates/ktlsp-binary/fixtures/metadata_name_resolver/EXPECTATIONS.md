# Manual fixture freeze revision 2

Source-derived, not executed; table hex is UNFRAMED #125 input. d2 strings use
literal ASCII unless arrays of hex UTF16 units are shown. No Kotlin compiler,
JVM oracle, encoder or resolver was used. Each row is a separate resolver.

| ID | table hex | d2 | request | expected units/behavior |
|---|---|---|---|---|
| F01 | empty | empty | get0 | InvalidIndex; construction succeeds |
| F02 | 0a00 | ab | get0 | ab |
| F03 | 0a023200 | fallback | get0 | empty (present empty wins) |
| F04 | 0a04100e3200 | empty | get0 | empty (wins over predefined14) |
| F05 | 0a02100e | empty | get0 | kotlin/String |
| F06 | 0a02102b | empty | get0 | kotlin/collections/MutableListIterator |
| F07 | 0a02102c | fallback | get0 | fallback (index44 out of range) |
| F08 | 0a00 | empty | get0 | MissingString |
| F09 | 0a040802100e | empty | get0,get1,get2 | kotlin/String twice, InvalidIndex |
| F10 | 0a020802 | first,second | get1 | second (requested index, not record ordinal) |
| F11 | 0a0208000a00 | picked | get0 | picked (zero record contributes nothing) |
| F12 | 0a0608ffffffff0f0a00 | picked | get0 | picked (negative range contributes nothing) |
| F13 | 0a0422020103 | [0061,d83d,de00,0062] | get0 | [d83d,de00] |
| F14 | 0a0422020102 | [0061,d83d,de00,0062] | get0 | [d83d] (surrogate split preserved) |
| F15 | 0a0422020201 | abc | get0 | abc (reversed ignored) |
| F16 | 0a0422020004 | abc | get0 | abc (out-of-range ignored) |
| F17 | 0a03220101 | abc | get0 | abc (one substring entry ignored) |
| F18 | 0a052203010263 | abc | get0 | b (extra entry ignored) |
| F19 | 0a042a02242e | a$b$ | get0 | a.b. |
| F20 | 0a021801 | p/A$B | get0 | p/A.B (slash retained) |
| F21 | 0a021802 | xy | get0 | empty (no descriptor validation) |
| F22 | 0a021802 | x | get0 | x |
| F23 | 0a021802 | empty string | get0 | empty |
| F24 | 0a0a1802220201062a027824 | zLx$Y;z | get0 | ..Y (substring then replace then operation) |
| F25 | 0a0418011863 | A$B | get0 | A.B (unknown99 does not override known1) |
| F26 | 0a021863 | A$B | get0 | A$B (unknown-only leaves defaultNone) |
| F27 | 0a063204f09f9880 | empty | get0 | [d83d,de00] |
| F28 | 0a033201ff | ignored | get0 | UnsupportedUtf8 (explicit wins, no fallback) |
| F29 | 0a0508003201ff0a00 | ok | get0 | ok (invalidUTF8 in zero-range record unselected) |
| F30 | 0a002801280128ffffffff0f | name | local1,local-1,local2,get0 | true,true,false,name |
| F31 | 0a00 | [0000,d800,dc00,ffff] | get0 | exact same four units |
| F32 | 0a021802 | [d83d,de00,0061] | get0 | [de00] (strip by unit) |

Typed-record manual cases (no raw-wire encoder needed):
T01: absent predefined falls back; predefined -1 falls back; predefined0 returns kotlin/Any.
T02: all44 predefined entries use exact ordered pinned PREDEFINED_STRINGS list, no generated expectation.
T03: replace [-1,65536] on [ffff,ffff,0000] => [0000,0000,0000]; one entry ignored.
T04: substring [-1,1] ignored; [0,0] empty; [len,len] empty; three entries use first2.
T05: explicit present valid empty overrides bad predefined/missing d2; explicit NUL byte => [0000].
T06: explicit CESU8 eda080 and overlong NUL c080 => UnsupportedUtf8; invalid explicit in another
record must not break get on first record. d2 unpaired surrogate always remains valid data.
T07: public malformed raw/span/unknown representations have no effect on typed resolution;
counts from all consumed typed collections still enforce bounds. Never use span for indexing.
T08: positive range i32::MAX rejected with range_slots limit before expansion/allocation;
negative i32::MIN contributes0; sum overflow uses checked arithmetic.
T09: entries/input_bytes/range_slots thresholds exact and one-below; all ignored tails,
duplicate local entries, zero-range explicit strings count. Raised limits clamp hard defaults.
T10: queries/work/output_units thresholds exact and one-below; repeat lookup charges again;
negative-index and missing-d2 failures consume query/work; no refund after UnsupportedUtf8.
T11: local queries include negative/out-of-range i32 without record validation; duplicate
membership unchanged; zero queries rejects first method, zero work may reject construction.
T12: zero output_units permits empty output; nonempty fails before allocation; output_units
is cumulative across successful and failed allocation attempts. No partial output on failure.
T13: operation defaultNone/explicitNone same; replace '$'->'x' before InternalToClassId
leaves x, while replace x->'$' before InternalToClassId becomes '.' (order distinguishes).
T14: d2 longer/shorter than expanded records accepted at construction; explicit/predefined
names work without corresponding d2 slots. Unknown/wrong-wire fields stay #125 decoder's job.

No every-byte truncation claim for semantic resolver: #125 wire regressions cover malformed
wire, while prefixes that are valid tables can legitimately resolve differently. Preserve
raw/header/string_table regressions unchanged; add composition only after #125 delivery.

## Exact numerical budget cases

All unspecified limits are hard defaults. Counters below mean consumed work/queries/
output_units; private counters can be checked by exact subsequent calls at the boundary.
Error context is (index,record); None means no selected compact record. Failed debits
leave that bucket unchanged, preserve earlier successful debits, and return no output.

B01: empty table+d2, limits entries0,input_bytes0,range_slots0,work0,queries0,output_units0:
new succeeds, counters(0,0,0). get0 => Limit(queries),(Some0,None), counters unchanged.
B02: same empty input with work1,queries1: get0 => InvalidIndex,(Some0,None), counters(1,1,0).
B03: F02 using d2=ab: entries2,input_bytes4,range_slots1,work6 suffice for new, counters(6,0,0).
entries1 => Limit(entries),(None,None); input_bytes3 => Limit(input_bytes),(None,None);
work5 => Limit(work),(None,None); range_slots0 with work6 => Limit(range_slots),(None,Some0).
B04: F02, work14,queries1,output_units2: new+get0 succeeds ab, counters(14,1,2).
work13: get0 fails final work4 reservation debit => Limit(work),(Some0,Some0), counters(10,1,2).
output_units1,work14: get0 => Limit(output_units),(Some0,Some0), counters(10,1,0).
B05: F28 with d2 empty: entries1,input_bytes1,range_slots1,work5,queries1,output_units0:
new counters(2,0,0); get0 => UnsupportedUtf8,(Some0,Some0), counters(5,1,0).
work4 instead => Limit(work),(Some0,Some0) BEFORE UTF8 scan, counters(4,1,0).
B06: F02, work17,queries3,output_units2: new(6,0,0); get-1 => InvalidIndex,
(Some-1,None), counters(7,1,0); get0 succeeds ab, counters(15,2,2); get1 => InvalidIndex,
(Some1,None), counters(17,3,2). With queries2 instead, third call Limit(queries),
(Some1,None), counters(15,2,2); no work debit after failed query debit.
B07: F08 empty d2: new work1; get0 under total work3,queries1 => MissingString,
(Some0,Some0), counters(3,1,0); work2 => Limit(work),(Some0,None) before record visit, counters(2,1,0).
B08: F30 with d2=name: entries5,input_bytes8,range_slots1 => new work13.
local1 costs query1+methodwork1+one comparison1 => counters(15,1,0), true.
local-1 next costs methodwork1+three comparisons3 => counters(19,2,0), true.
local2 next costs methodwork1+three comparisons3 => counters(23,3,0), false.
B09: typed record substring[0,0], d2=ab: entries4,input_bytes4 => new work8.
get0 under work12,queries1,output_units0 returns empty, counters(12,1,0): method1,
record1, source2, output0. Zero output limit does not forbid empty names.
B10: caller table records.len=65536, other top-level lengths0, entries hard default:
Limit(entries),(None,None) in O(1), before touching any record; records.len1 with
substring.len65535 and d2 empty => entries excess on metadata addition before content scan.
B11: F28 with d2 empty, query twice with work8,queries2: first UnsupportedUtf8 leaves(5,1,0),
second UnsupportedUtf8 leaves(8,2,0). It must not restart constructor/work budgets.
B12: two default records, d2=[ab,c], work20,queries2,output_units1: new(10,0,0);
get0 => Limit(output_units),(Some0,Some0), counters(14,1,0), output remainder1;
get1 succeeds c, counters(20,2,1). Failed output debit must preserve the unused unit.

## All predefined outputs, manually transcribed in source order

Indices0..43, one present predefined_index per typed-record case, d2 empty:

0 kotlin/Any
1 kotlin/Nothing
2 kotlin/Unit
3 kotlin/Throwable
4 kotlin/Number
5 kotlin/Byte
6 kotlin/Double
7 kotlin/Float
8 kotlin/Int
9 kotlin/Long
10 kotlin/Short
11 kotlin/Boolean
12 kotlin/Char
13 kotlin/CharSequence
14 kotlin/String
15 kotlin/Comparable
16 kotlin/Enum
17 kotlin/Array
18 kotlin/ByteArray
19 kotlin/DoubleArray
20 kotlin/FloatArray
21 kotlin/IntArray
22 kotlin/LongArray
23 kotlin/ShortArray
24 kotlin/BooleanArray
25 kotlin/CharArray
26 kotlin/Cloneable
27 kotlin/Annotation
28 kotlin/collections/Iterable
29 kotlin/collections/MutableIterable
30 kotlin/collections/Collection
31 kotlin/collections/MutableCollection
32 kotlin/collections/List
33 kotlin/collections/MutableList
34 kotlin/collections/Set
35 kotlin/collections/MutableSet
36 kotlin/collections/Map
37 kotlin/collections/MutableMap
38 kotlin/collections/Map.Entry
39 kotlin/collections/MutableMap.MutableEntry
40 kotlin/collections/Iterator
41 kotlin/collections/MutableIterator
42 kotlin/collections/ListIterator
43 kotlin/collections/MutableListIterator

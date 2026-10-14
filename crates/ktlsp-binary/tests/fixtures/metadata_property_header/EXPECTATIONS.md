# Literal Property acceptance freeze before implementation

Independent manual derivation from pinned schema/generated tags and existing wire
rules, never implementation output. Tuple order F/O/G/S/N is flags/old_flags/
getter_flags/setter_flags/name; '-' means None, numbers in flag columns mean Some.
W(kind) wraps wire ErrorKind; Missing means MissingName. Every success asserts
entire supplied slice pointer/byte identity, including successful prefix cuts.

## Four authored raw fixtures

| File | Exact hex | F/O/G/S/N | N/F/work; last tag |
| --- | --- | --- | --- |
| minimal | 10 00 | -/-/-/-/0 | 2/1/4; 0 |
| explicit | 08 86 10 10 00 38 00 40 00 58 86 04 | 518/2054/0/0/0 | 12/5/22; 9 |
| duplicates | 08 01 38 02 40 03 58 04 10 01 08 06 38 00 40 06 58 00 10 00 0a 01 ff 3a 01 80 42 01 00 5a 00 12 00 | 0/6/0/6/0 | 33/15/63; 31 |
| opaque | 10 00 1a 01 80 22 00 2a 00 32 00 62 01 ff 6a 01 80 a2 06 01 00 | -/-/-/-/0 | 21/8/37; 17 |

opaque contains malformed return_type, empty TypeParameter, receiver Type and
setter ValueParameter, invalid UTF8 context Type, malformed packed IDs, extension100.
It must succeed only as selected header, despite nested initialization failures.

## Presence/last wins/wrong wire

For each selected flag (tag/default): (08/2054),(38/0),(40/0),(58/518):
- `10 00` yields None; `TAG 00 10 00` yields Some0 independently.
- `TAG DEFAULT_VARINT 10 00` yields explicit Some(default). DEFAULT_VARINT is
  86 10, 00, 00, 86 04 respectively. No other flag changes from None.
- `TAG 01 TAG 02 10 00` yields Some2; reverse yields Some1.
- `TAG 01 10 00` and `10 00 TAG 01` give identical independent values.
- Each flag alone (without name) ends Missing at full length.

Required name: empty=>Missing0; `12 00`=>Missing2; `10 00`=>name0;
`10 01 10 02`=>name2; reverse=>name1. Flags never supply name.
For each selected field tag T in08/10/38/40/58 use all wrong wires1/2/5:
tags T+1 followed eight zero bytes; T+2 followed00 (empty bytes); T+5 followed
four zero bytes. Alone yields Missing at end; with name0 gives independent None
for that flag; for name wrong-wire use preceding/following `10 01` and expect1.
Wrong-wire before/after recognized `T 01` never changes that recognized value.
Wrong-wire length-delimited payload80/ff/tag0/group bytes is opaque.
`48 01 50 02 10 00` yields all flags None/name0: Type IDs are not flag tags.
`90 00 80 00`=>name0; `90 80 80 80 10 00`=>name0 (low32 tag alias).

Signed values repeat independently for all five selected fields; for flags append
`10 00`, other flags stay None. Repeat duplicates with positive/negative last-win:

| Value bytes | int32 |
| --- | --- |
| ff ff ff ff 0f | -1 |
| ff ff ff ff ff ff ff ff ff 01 | -1 |
| ff ff ff ff ff ff ff ff ff 7f | -1 |
| 80 80 80 80 08 | -2147483648 |
| ff ff ff ff 07 | 2147483647 |
| 80 80 80 80 10 | 0 |
| 80 80 80 80 80 80 80 80 80 02 | 0 |
| 80 00 | 0 |

Append unknown100 forms to name0: a0 06 01; a1 06 01 02 03 04 05 06 07 08;
a2 06 00; a2 06 03 ff 00 0b; a5 06 01 02 03 04. All succeed/raw identity.
Reserved19000 c0 a3 09 01 and max536870911 f8 ff ff ff 0f 01 succeed similarly.
`10 00 32 80 08`+1024ff has N1029/F2/work1033; work1032 fails work at2
(dispatch), exact1033 succeeds. Opaque payload is charged, never recursively read.

## Structural errors and no early completion

| Bytes | Error offset |
| --- | --- |
| 10; 80 (separate) | W(Truncated)1 |
| 00;03;07 | W(InvalidFieldNumber)0 |
| 0b;0c;13;14;3b;3c;43;44;5b;5c | W(UnsupportedGroup)0 |
| 0e;0f;16;17;3e;3f;46;47;5e;5f | W(InvalidWireType)0 |
| 12 02 ff | W(Truncated)3 |
| 0d 01 02 03 | W(Truncated)4 |
| 09 01 02 03 04 05 06 07 | W(Truncated)8 |
| 12 80 80 80 80 08 | W(NegativeLength)1 |
| 12 ff ff ff ff 07 | W(Truncated)6 |
| 10 + ten80;12 + ten80 | W(MalformedVarint)10 |
| ten80 | W(MalformedVarint)9 |

Repeat each error after prefixes `10 00`, each flag-default alone, and the explicit
fixture; same kind, offset+prefix length. Selected tags08/10/38/40/58 followed
1..9 continuation80 bytes truncate at supplied length; ten80 malformed at tenth.
Wire errors always precede Missing, with present or absent name/all flags.

## Every byte cut (72 cases)

Lists partition n0..len: T=>Truncated at n; M=>Missing at n; S gives exact tuple.
- minimal len2: T[1], M[0], S2=(-,-,-,-,0).
- explicit len12: T[1,2,4,6,8,10,11], M[0,3];
  S5=(-,2054,-,-,0);7=(-,2054,0,-,0);9=(-,2054,0,0,0);12=(518,2054,0,0,0).
- duplicates len33: T[1,3,5,7,9,11,13,15,17,19,21,22,24,25,27,28,30,32],
  M[0,2,4,6,8]; S10=(4,1,2,3,1);12=(4,6,2,3,1);14=(4,6,0,3,1);
  16=(4,6,0,6,1);18=(0,6,0,6,1);20/23/26/29/31/33=(0,6,0,6,0).
- opaque len21: T[1,3,4,6,8,10,12,13,15,16,18,19,20], M[0];
  S2/5/7/9/11/14/17/21=(-,-,-,-,0).

Counts:3+13+34+22=72. Prefix success is selected-header success only; unframed
input cannot identify a missing container tail or establish full initialization.

## Exact budget thresholds/precedence

Each fixture passes exact N/F/work totals and each+1. bytes N-1 fails bytes at0
even records/work0. records F-1 fails at listed last tag; work total-1 fails at
same tag (dispatch). Empty all0=>Missing0. Limits usize::MAX clamp hard maxima.
`10 00` records1/work0,1=>work0;work2=>work1;work3=>work0;work4=>success.
records0/work0=>records0. `10` records1/work2=>work1;work3=>Truncated1.
`10 00 80` records1/work4=>records2;records2/work6=>work3;work7=>Truncated3.
`10 00 00` work6=>InvalidFieldNumber2. `12 00` work3=>work0;work4=>Missing2.
`12 02 ff` work4=>work2 (bulk shortage);work5=>Truncated3.
`10 00 32 01 80` work8=>work2;work9=>success.
`10 00 32 03 ff ff ff` work9=>work4 (bulk debit);work10=>work2;
work11=>success. Hard bytes+1 fails bytes0 before record/work.
name0 +65534 copies `38 00` is N131070/F65535/work262140, GSome0, succeeds;
append `40 00` fails records131070 under usize::MAX. Successful scan under hard
byte/field ceilings cannot reach hard64Mi work independently; test small thresholds,
do not claim impossible hard-work witness. Future CI tests may allocate bounded
large test input; decoder never allocates from declared lengths.

## Finite hostile tests (future original native CI)

For all a,b0..255 decode [a,b] and [10,00,a,b] twice with bytes32/records8/work256;
for each a repeat length n0..31. Assert deterministic full Result, input-relative
error offset<=len, success entire raw pointer/byte identity. 139264 inputs/278528
calls. Literal assertions above establish behavior; probes alone do not prove
all-input safety, timing/RSS/stack/platform/full protobuf/semantic compatibility.

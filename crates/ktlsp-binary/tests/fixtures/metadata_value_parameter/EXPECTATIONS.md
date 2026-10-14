# Independent literal/arithmetic freeze before implementation

No product execution supplies expectations. Tuple is (flags,name,type_id,vararg_id).
W wraps wire ErrorKind. Every success raw==supplied bytes AND pointer identity.

## Four fixtures

| fixture | tuple | N/F/work/last-tag |
| --- | --- | --- |
| minimal | (None,0,None,None) | 2/1/4/0 |
| explicit | (Some0,0,Some0,Some0) | 8/4/16/6 |
| duplicates | (Some0,5,Some6,Some7) | 28/12/52/25 |
| opaque | (None,0,None,None) | 19/8/35/16 |

Opaque interleaves two occurrences each of singular type3/vararg4/default8 and
one annotation7; inner 80 or00 are deliberately malformed/opaque. Additional literal
`10 00 3a 00 3a 01 ff a2 06 01 00` proves repeated annotation7 and extension100
retained; bytes11/fields4/work19. All selected IDs absent, not derived from inline.

## Every-byte cuts: partitions, tuples explicitly before source

minimal: T[1], M[0], S2=(None,0,None,None).
explicit: T[1,3,5,7], M[0,2];
S4=(Some0,0,None,None), S6=(Some0,0,Some0,None), S8=(Some0,0,Some0,Some0).
duplicates: T[1,3,5,7,9,11,13,15,17,18,20,21,23,24,26,27], M[0,2];
S4=(Some1,2,None,None), S6=(Some1,2,Some3,None),
S8=(Some1,2,Some3,Some4), S10=(Some0,2,Some3,Some4),
S12=(Some0,5,Some3,Some4), S14=(Some0,5,Some6,Some4),
S[16,19,22,25,28]=(Some0,5,Some6,Some7).
opaque: T[1,3,5,6,8,10,11,13,15,17,18], M[0];
S[2,4,7,9,12,14,16,19]=(None,0,None,None).
T means W(Truncated) at cut n; M means MissingName at n. Total61 cuts.
Unframed field-boundary prefixes can succeed; no container truncation inference.

## Selected scalar matrix

All four field numbers1/2/5/6 independently test:
- absent, explicit0, positive300, signed -1 via5byte/10byte, i32MIN/MAX,
  nonminimal zero `80 00`, low32 overflow `80 80 80 80 10`=>0.
- duplicate recognized scalars last-win, including signed negative and cross order.
- wrong wire1/2/5 alone supplies no presence; before/after baseline
  `08 01 10 02 28 03 30 04` preserves (Some1,2,Some3,Some4).
- name wrong-wire only => MissingName at EOF; wrong others plus `10 00` remain None.
- nonminimal tag/value `90 00 80 00` and low32-tag alias `90 80 80 80 10 00`
  => (None,0,None,None).
- coexistence `10 00 1a 00 28 03 22 00 30 04`=> (None,0,Some3,Some4);
  reversing inline/ID occurrence order yields same tuple. No type precedence.

## Wire error and precedence matrix

| literal | error/offset |
| --- | --- |
| empty | MissingName0 |
| 08 00 28 00 30 00 | MissingName6 |
| 12 00 | MissingName2 |
| 10 or80 | W(Truncated)1 |
| 00/03/07 (separate) | W(InvalidFieldNumber)0 |
| 0b/0c/13/14/2b/2c/33/34 | W(UnsupportedGroup)0 |
| 0e/0f/16/17/2e/2f/36/37 | W(InvalidWireType)0 |
| 12 02 ff | W(Truncated)3 |
| 0d 01 02 03 | W(Truncated)4 |
| 09 01 02 03 04 05 06 07 | W(Truncated)8 |
| 1a 80 80 80 80 08 | W(NegativeLength)1 |
| 1a ff ff ff ff 07 | W(Truncated)6 |
| selected tag +ten80 | W(MalformedVarint)10 |
| ten80 | W(MalformedVarint)9 |
| 1a +ten80 | W(MalformedVarint)10 |

Each structural row repeated with present-name prefix `10 00`, absent-name prefix
`08 00`, and all-selected prefix `08 01 10 02 28 03 30 04`; error kind unchanged,
offset adds prefix length. Each selected tag+1..9 continuation80 => Truncated at
supplied end. Ten continuation80 => Malformed at tenth continuation. All failures
precede MissingName, even after name present. No panic or partial Result.

## Exact budget edges

All fixtures exact N/F/N+2F and each+1 succeed. bytes N-1 fails bytes0 even with
records/work0. records F-1 and work total-1 fail at frozen last tag. Empty all0
=>Missing0. `10 00` work0/1=>work0, work2=>work1, work3=>work0, work4=>success;
records0/work0=>records0. `10` work2=>work1, work3=>Truncated1.
`10 00 80` records1/work4=>records2; records2/work6=>work3;
records2/work7=>Truncated3. `10 00 00` work6=>InvalidFieldNumber2.
`12 00` work3=>work0(dispatch), work4=>MissingName2.
`12 02 ff` work4=>work2(bulk short), work5=>Truncated3.
`10 00 1a 01 80` work8=>work2(dispatch), work9=>success.
`10 00 1a 03 ff ff ff` work9=>work4(bulk read), work10=>work2(dispatch), work11=>success.
`10 00 1a 80 08`+1024ff: N1029/F2/work1033; work1032=>work2(dispatch).
All errors work/records are W(Limit(label)); offsets above, no refund/reset.
usize::MAX clamps. MAX_BYTES+1=>bytes0 before other checks. `10 00` +65,534 copies
`08 00`: N131070/F65535/work262140 succeeds (Some0,0,None,None); append `28 00`
=>records131070. Hard-work maximum has no independent successful witness.

Finite hostile probes all65536 two-byte inputs and same prefixed with1000, paired
decode equality/raw identity/error offset<=len, bounded bytes32/records8/work256.
No all-input/CPU/timing/RSS/stack or official runtime equivalence claim.

Existing shared wire tests cover lower-level framing; no broad new fuzzer/harness.
No genuine artifact hunt required: authored independent literals test this schema.
Preserve existing LICENSE/NOTICE/PROTOBUF_LICENSE unchanged in future fixture dir.

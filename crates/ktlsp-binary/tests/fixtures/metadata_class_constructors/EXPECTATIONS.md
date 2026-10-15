# Frozen independent Class/Constructor expectations

Literal arithmetic frozen before source. Native tests have not run locally.
Helper provisional references below record the original freeze, now reconciled
at released main9bf15b6; fixture values and expectations stay unchanged.

## Independent success arithmetic and literal expectations

EXPECTATIONS.json +17 hex fixtures freeze complete scalars, ordered child tuples,
body/parameter spans, IDs, records/work/owned minima and each one-less error stage
and offset. O=size_of::<ConstructorOccurrence>, S=size_of::<ValueParameterBody>,
I=size_of::<i32>; no hardcoded ABI layout is required. P(0)=0, else next power2.
For Class N bytes/F fields, child j n_j bytes/f_j fields, K_j packed ID items,
L_j packed varint byte count, V_j total packed+unpacked IDs, parameter count p_j:
R=2F+sum(f_j+K_j).
B=P(C)*O+sum(P(p_j)*S+P(V_j)*I).
W=2N+4F+sum(n_j+2f_j+K_j+L_j+V_j)+B.
Two allocating children literal `18 00 42 05 12 00 f8 01 00 42 05 12 00 f8 01 00`
retains child bodies4..9/11..16, each flagsNone,param empty span2..2,IDs[0].
N16/F3, R10, B2O+2S+2I, W64+B. Owned/work one-less fails outer push9;
records-minusone fails second child ID at13. This observes inner storage from
BOTH children and outer storage together; per-child budget replacement fails.

Records include both parent scans and packed rereads. Wire bytes are charged
again on each scan; opaque parameter payload bytes are scanned only as bulk
Constructor bytes, never parsed. Growth is logical lengths0/1/2/4 -> slots1/1/2/4.
Exact limits and each +1 succeed. Each separate one-less dimension fails exactly
as frozen (owned0 rows have no owned-minus-one probe). Combined zero dimensions
first hit byte preflight where byte bound is short; otherwise existing ordered
record/work/owned events govern.

477 every-byte prefix cases: use listed successful complete boundaries;
zero =>Class.MissingFqName0; additional complete boundaries before required fq
are explicit cut2, signed cut6, genuine Pair cut3, duplicate_scalars cut2
=>MissingFqName@cut. Every
remaining cut =>Class.Wire.Truncated@cut. Successful prefixes return original
raw prefix and only fully available children/scalars; nested bytes inside an
incomplete outer length never become a partial child. Prefix expectations are
pre-frozen literals, not computed by the implementation under test.

## Scalar, duplicate and wrong-wire positive controls

Ten scalar byte strings/expected signed values are frozen in EXPECTATIONS.json.
For EACH, author wrapper `18 00 42 [body length] 08 [value]` for Constructor flags;
`18 00 42 [body length] f8 01 [value]` for unpacked version;
`18 00 42 [body length] fa 01 [value length] [value]` for packed version.
The resulting exact tuples are (Class flagsNone,fq0), one Constructor with
flagsSome(value)/versions[] for flags, or flagsNone/versions[value] for either
version form; parameters[]. No ZigZag, index validation/default manufacture.
All lengths are one byte (body<128). Body span4..N; child.raw and class raw
pointer identities required. Formula above freezes corresponding minima.

Parent scalar controls `08 [value] 18 00 42 00` =>Class flagsSome(value),fq0;
`18 [value] 42 00` =>Class flagsNone,fq=value; each has one empty Constructor.
Absent flags and explicit6 are distinct. The signed fixture combines Class -1 /
fq i32MIN with Constructor -1 and IDs[-1,i32MAX,0]. Authored positives do not
represent genuine Pair compiler coverage.
Duplicate control literal `08 01 18 01 42 02 08 01 08 06 18 00 42 04 08 06 08 00`
=>Class flagsSome6/fq0; two children body6..8 flags1 and body14..18 flags0;
parameters[]/IDs[]; flags1 and6 in distinct messages are NEVER merged. N18/F6,
child totals n6/f3; R15, B2O,W72+2O. One-less records Constructor@16,
one-less work/owned Wire@12. Prefix successes complete ends[4,8,10,12,18];
cut2 MissingFqName2; remaining nonzero cuts Truncated@cut.

Wrong-wire child flags1 (wire1,2,5), parameters2(wire0,1,5), versions31(wire1,5)
are individually preserved opaque before and after baseline child
`08 06 12 02 10 05 f8 01 07`: flags6,param raw1005,IDs[7]. Frozen wrongwire
literals: `09 00 00 00 00 00 00 00 00`, `0a 01 80`, `0d 00 00 00 00`,
`10 00`, `11 00 00 00 00 00 00 00 00`, `15 00 00 00 00`,
`f9 01 00 00 00 00 00 00 00 00`, `fd 01 00 00 00 00`.
Parent wrongwire8 forms are already explicit opaque_class controls. Field8
nonminimal/low32-aliased tags still select under existing reader semantics.
Unknown Class field9/type6 malformed raw bodies and child annotation/plugin/
extension bodies stay opaque. Field2/wire2 parameter80 succeeds as opaque even
though separate ValueParameter decode would fail; no nested initialization.

## Failure precedence and failure-charge events

22 literal faults in EXPECTATIONS.json freeze exact Class versus Constructor
error stages and offsets, including malformed tails/MissingFqName before child,
nonfinal packed EOF, negative lengths, groups, invalid tags/types, malformed
10-continuation varints, later child failures and first-child precedence.
These fault rows use adequate default budgets. Prefix2 `18 00` must not be
mistaken for a partial typed result when a later error occurs.

Nonfinal packed EOF `18 00 42 04 fa 01 01 80 08 06`: Class flagsSome6,fq0 would
be parent-valid; child packed end is Class8, exactly the following parent tag.
R7,W37,owned0 reach Constructor.Truncated@8. With records6 =>Constructor.
Limit(records)@7; work36 =>Constructor.Limit(work)@8; work35 =>Constructor.
Limit(work)@7. Parent W16 + selection prefix(4+8) + child outer6 + packed record1
+read1+EOFshort1=37. Selection tail08 06 is NOT reached. No child/outer vector
reservation precedes this failure; future public error need not expose counters.

Two-child fault `18 00 42 00 42 01 80`: R7,W29+O,ownedO reach Constructor.
Truncated@7. W28+O =>Constructor.Limit(work)@7; ownedO-1 fails first outer push
Wire.Limit(owned_bytes)@2. The first outer capacity charge is still included in
later child failure work; no refund/reset after the first occurrence.
`18 00 42 02 12 00 42 01 00`: first child empty parameter spends S before O;
second invalid tag at8. R8,W36+S+O,ownedS+O reach Constructor.InvalidFieldNumber8;
one-less work35+S+O =>Constructor.Limit(work)@8. Required child+outer storage remains
charged while visiting the later child. All failure arithmetic is static/unrun.

Event ledger for empty_one `18 00 42 00`: parent work8, selection work8, then
outer ownedO BEFORE outer workO and try_reserve. Work0/1 =>Class.Wire.Limit(work)0;
work2 =>Class.Wire.Limit(work)1; work3 =>Class.Wire.Limit(work)0;
work4 =>Class.Wire.Limit(work)2; work5 =>Class.Wire.Limit(work)2;
work6 =>Class.Wire.Limit(work)3; work7 =>Class.Wire.Limit(work)2;
work8/9 =>Wire.Limit(work)0;work10 =>Wire.Limit(work)1;
work11 =>Wire.Limit(work)0;work12/13 =>Wire.Limit(work)2;
work14 =>Wire.Limit(work)3;work15 =>Wire.Limit(work)2;
work16 with owned0 =>Wire.Limit(owned_bytes)2;
work16 with ownedO =>Wire.Limit(work)2. Records0 precedes work0 at Class0;
records2 with work8 =>Wire.Limit(records)0. Distinct parent/selection stages are
observable without changing existing helpers. Source review checks that failed
owned/work/allocation paths retain previous debits; no invasive test hook/helper
redesign is required merely to expose private counter values.

## Hard clamps and finite future test plan

MAX_BYTES=16MiB, MAX_RECORDS=65535, MAX_VALIDATION_UNITS=4*MAX_BYTES (confirm pins).
bytes MAX+1 and usize::MAX caller fields still produce Wire.Limit(bytes)0 before
content/other limits. Hard-record witness `18 00` +32766 copies `40 00`:
N65534,F32767,R65534,W262136,owned0 succeeds; append one `40 00` =>F32768;
even raised record usize::MAX must fail Wire.Limit(records)@65534 in second scan.
No allocator needed. Compare default versus all raised usize::MAX for frozen
fixtures and record witness; do not claim a hard-work/owned-bound success witness
where records mask it. Inspect static clamp paths independently. Do not allocate
unbounded hostile input. Growth0/1/2/4 is exercised for outer, parameter and ID
vectors, including repeated empty bodies and all child vectors against one budget.

Future focused tests only after SOURCE/CI release: 17 fixtures,
477 prefix cases, 22 literal faults, scalar
and wrongwire matrices, exact/minusone/plusone budgets and event ledger, hard
clamps, genuine pipeline below. Optional finite two-byte inventory 65536 cases
and bounded repeated-byte0..31 patterns can be authored later only within root
SOURCE allowlist; no broad fuzz harness/runtime happens at this contract stage.
Static checks are not passing native tests or performance/platform evidence.

## Genuine pipeline, provenance and notices

Existing committed Pair.class.bin is unchanged2983 bytes / SHA2568fed0fa...9736.
PAIR_EXTRACTION.json freezes exact class/JAR/d1 provenance. Read existing Rust
pipeline in metadata_constructor.rs genuine_pair_constructor_and_both_parameters:
parse_class -> annotations::decode -> metadata::decode -> payload::decode
(or existing wire envelope) -> Class238-byte payload47..285 -> new composition.
No fresh artifact/compiler/JVM hunt, execution or parser helper.

Literal Class flagsSome1031/fq0/F15; Constructor Class field29..56/body31..56
(d1 field76..103/body78..103), flagsSome70, no version IDs, two opaque parameter
bodies Constructor4..10/12..18 =Class35..41/43..49 =d1 82..88/90..96;
raw `10 05 1a 02 38 00` and `10 06 1a 02 38 01`. Existing separately bounded
ValueParameter decode may assert tuples(None,5,None,None)/(None,6,None,None)
in the future test; those separate checks do not enter this composition budget.
New composition R34,B=O+2S,W569+O+2S. Header/d1/payload decoding retains separate
operation budgets; never claim whole-pipeline accounting or semantic integration.
Fixture compiler provenance remains UNVERIFIED. All flags/version positive cases
are authored controls, not genuine version-bearing compiler output. Preserve
old fixtures/tests/provenance. Temp copies of existing LICENSE.txt,NOTICE.txt,
PROTOBUF_LICENSE.txt are byte-identical with pinned hashes in INPUT_PINS.json.
Future fixture reuse references existing Pair bytes and retains these notices;
no binary copy/new external attribution hunt is required.


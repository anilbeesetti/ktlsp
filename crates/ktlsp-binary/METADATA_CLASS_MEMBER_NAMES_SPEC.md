# Class member names — admitted source contract

Coordinator admitted contract v3 SHA2566da02f889ef6cc539040a8012352004cd751704d22bfa4d1c42defa0c78279fe
and its exact source/API/shared-helper/publication/original-CI scope on 2026-10-14.
The preimplementation freeze below is preserved; its preparation-only stage describes
that historical freeze. Source and completed original CI review are separate.

# Class member name composition: admission contract v3

Owner chat 01a1386b-94ba-7fc2-a874-d24cadec7a83. Coordinator
01a1300d-2ba0-7832-97d0-91d070e2d297/local. Contract preparation only:
SOURCE/PUBLICATION/CI HOLD until coordinator reads this reviewed packet and admits
the exact paths/API/helpers. No native execution performed.

Owned attached worktree /Users/anil/.codex/worktrees/class-member-names/ktlsp,
branch codex/class-member-names, base537c3704d62ecd65117ac2af79c208cb19ea8f22,
tree38f78e013d4b4550fa835fd8931f9593dad8d28f,
sole parent5e8d283f3cb9765631847642e35f71586315b177. No source changes.

## Finite public API and meaning

New metadata::class_member_names::decode(&metadata::Outcome, Limits)
-> Result<Vec<Member>, Error>. Existing source calls the supported value
Outcome::Decoded { header, bytes }; there is no DecodedMetadata type.
Require Decoded AND header.k==1 AND header.mv==[2,3,0] before bytes/framing.
Reject Absent, Unsupported, Decoded Package kinds2/5, and forged Decoded wrong
kind/version with ErrorKind::UnsupportedContainer at offset0. Validation concerns
this selected container/header, not authenticity of caller-constructible fields.

Member has kind Kind::{Function,Property}, ordinal:usize (zero-based position
among ALL Class wire fields), span:Range<usize> (entire outer member field),
body_span:Range<usize>, name:Vec<u16>, flags/old_flags/getter_flags/setter_flags:
Option<i32>. Both spans are decoded-d1-relative. Function accessor flags are None.
Raw flag presence/value is retained: no default injection, bit conversion or
accessor inheritance. Preserve encoded interleaved order, duplicates and overload
names. UTF16 output may be empty, contain NUL or lone surrogates. Ordinals/spans
are encoded data, NOT stable semantic SymbolIds.

Error { offset:usize, kind:ErrorKind }; kinds UnsupportedContainer, Wire(wire::
ErrorKind), Class(class_header::ErrorKind), Function(function_header::ErrorKind),
Property(property_header::ErrorKind), Name(name_resolver::Error).
All offsets are d1-relative. Name errors use selected member BODY START, preserving
resolver requested-index/record context; resolver-constructor errors use table
start. MissingClass uses d1 EOF, missing selected name uses member body end.
Framing/table errors rebase existing offsets. No arithmetic may wrap.

Only Class9/10 with wire2 select Function/Property; wrong-wire9/10 remain opaque
and count in ordinal. Constructors8, TypeAlias11, types and all other nested
bodies stay opaque, including malformed/tag0/groups/invalidUTF8 within them.
Selected Function/Property HEADER scans validate their complete outer wire bodies,
but their return/type/parameter/extension message bodies remain opaque. Full
Class initialization, type resolution and protobuf required-nested validation are
not claimed. Existing selected scalar last-recognized-wins behavior is reused.

Order: supported-container guard; O(1) d2-entry count preflight; combined input
preflight; framed typed table;
Class node + complete class_header shared scan (required fq_name); Resolver
constructor; second complete Class field scan in encoded order; for each selected
member node, corresponding shared header decoder, one resolver query, output push.
Both Class scans pay wire records/work. A malformed Class tail therefore fails
before any member result or empty result. Every success requires full traversal;
first later selected member/name error aborts the entire operation, no partial Vec.
Class fq_name is required signed data but is not queried/resolved here.

## One operation budget and exact private helper proposal

New Limits has bytes/records/nodes/depth/work/owned_bytes plus entries/range_slots/
queries/output_units. Defaults reuse string_table/name_resolver hard defaults;
all caller values clamp. BEFORE any d2 loop, reject header.d2.len() greater than
min(limits.entries,MAX_RECORDS) as Name Limit(entries),indexNone/recordNone at0.
This O(1) guard precedes even byte-limit checks and bounds forged Header d2 lists
of arbitrarily many empty slices. Later Resolver constructor still validates total
typed entries (records+d2+local+transform-list elements). bytes counts decoded d1
length + all d2 UTF16 byte lengths
with checked arithmetic, without counting selected table strings twice (in d1).
Already completed classfile/annotations/header-d1 decoding is outside this operation.
Length-only resolver preflight remains independently hard-bounded outside metered
work as its existing contract requires. All later framing/table/Class/member/query
work and records share ONE remaining allowance. No per-member reset.
Exact constructor mapping: resolver Limits.entries=composition.entries,
input_bytes=composition.bytes, range_slots=composition.range_slots,
queries=composition.queries, output_units=composition.output_units, work=SHARED
remaining wire.work after framing/table/first-Class scan. Resolver itself clamps
these values to its hard defaults as before. string_table::Limits receives the
six identically named composition fields. No hidden extra caller work allowance.

Minimal shared paths, ALL REQUIRE ROOT ADMISSION:
- src/metadata/mod.rs: one additive pub mod class_member_names export slot.
- src/metadata/payload.rs: extract pub(super) decode_with_budget(bytes,&mut
  string_table::Budget); public decode retains existing byte check/new budget.
- src/metadata/{class_header,function_header,property_header}.rs: extract
  pub(super) decode_with_budget(bytes,&mut wire::Budget); public wrappers retain
  current byte checks, errors/defaults and newly-created standalone budgets.
- src/metadata/string_table.rs: make node private-to-metadata; expose remaining
  owned counter private-to-metadata for linear resolver transfer. Existing push
  performs deterministic power-of-two logical capacity charges BEFORE reserve.
- src/metadata/wire.rs: expose remaining work private-to-metadata for linear
  resolver transfer (field visibility or tiny take/restore methods).
- src/metadata/name_resolver.rs: private new_with_budget/get_string_with_budget
  that transfer the shared remaining work counter into the EXISTING Resolver
  constructor/query and return the remainder. One Resolver per operation retains
  queries/output_units. A private optional owned remaining counter in Resolver
  lets emit debit checked name length*2 BEFORE try_reserve_exact; transfer this
  remainder back with work after each query. Public new/get_string signatures,
  standalone errors/counters/defaults/behavior stay unchanged (owned=None).
  Constructor has no name allocation. On query errors restore remainders then
  abort the whole composition. This is a linear transfer of ONE remaining counter,
  never an independent fresh allowance, post-allocation accounting or per-member
  budget. No generic public budget trait, cache, query framework or broad refactor.
  Exact emit order remains source UTF16 work, substring/operation length,
  output_units debit, checked length*2, existing output-byte work debit, NEW optional
  owned debit with Name Limit(owned_bytes) and unchanged requested index/record
  context, try_reserve_exact, iteration. Thus work failure precedes owned failure
  when both are insufficient; standalone owned=None has exactly its old order.

Use string_table::Budget for this composition's table and output Vec capacities;
all UTF16 name bytes debit the SAME owned bucket before allocation. Work already
charges those UTF16 bytes as existing emit does; do not charge them twice.
Node depths: table root1/record2; Class root1; selected member2. Each node pays1
work. Second Class scan has no extra node. Nodes count actual selected messages.
Requested capacity is logical, excludes allocator surplus/churn/RSS. Hard depth2
does not validate opaque deeper bodies. No whole-pipeline RSS claim.

Exclusive new paths:
crates/ktlsp-binary/src/metadata/class_member_names.rs
crates/ktlsp-binary/tests/metadata_class_member_names.rs
crates/ktlsp-binary/tests/fixtures/metadata_class_member_names/{mixed.hex,minimal.hex,
EXPECTATIONS.md,README.md,LICENSE.txt,NOTICE.txt,PROTOBUF_LICENSE.txt}
crates/ktlsp-binary/METADATA_CLASS_MEMBER_NAMES_SPEC.md
crates/ktlsp-binary/METADATA_CLASS_MEMBER_NAMES_REVIEW.md
No Cargo/lock/dependency/workflow/server/status/root-doc edits. Preserve existing
fixture notices; new authored hex project data. Real Unit reuses existing file.

## Independent literal fixture freeze (before implementation)

MIXED d1 (43 bytes), d2 units [[67],[102],[112],[102]] (8 bytes):
04 0a 02 08 04 18 00 4a 06 08 00 10 01 48 06 52 0a 08 00 10 02 38 00 40 01
58 00 4a 02 10 03 42 01 ff 5a 01 80 48 00 a2 06 01 00
table1..5 has one compact record body3..5, rangeSome4. Class5..43.
Literal ordered records (kind,ordinal,field span,body span,name,F/O/G/S):
Function,1,7..15,9..15,[102],Some6/Some0/None/None
Property,2,15..27,17..27,[112],Some0/Some0/Some0/Some1
Function,3,27..31,29..31,[102],None/None/None/None
Remaining Class fields: fq_name ordinal0 5..7; constructor ordinal4 31..34;
alias ordinal5 34..37; wrongwire Function ordinal6 37..39; unknown ordinal7 39..43.
No semantic overload discrimination; the two f records MUST both remain.

Let R=size_of::<string_table::Record>() and M=size_of::<Member>(). These are
platform layout quantities, never compiler-output-derived expectations. Logical
table capacity R, member vector capacity4M, name bytes6. Exact mixed requirements:
bytes51, records28 (envelope1+table2+Class scans16+members9), nodes6 (table2+Class1+
members3), depth2, entries5, range_slots4, queries3, output_units3,
owned_bytes R+4M+6; work190+R+4M.
Manual work derivation: payload14+R; Class scans108; resolver constructor13;
selected header scans36; queries15 (each query1+record1+source1+outputbytes2);
Class/member nodes4; output vector4M. Exact and +1 succeed; below:
bytes50=>Wire Limit(bytes)0 (including records/work0 precedence),
records27=>Wire Limit(records)39 (last second-scan unknown),
work189+R+4M=>Wire Limit(work)39 (last second-scan dispatch),
owned R+4M+5=>Wire Limit(owned_bytes)27 (third output growth),
owned R+1=>Name Limit(owned_bytes),indexSome1/recordSome0 at9 (first name BEFORE
try_reserve_exact, not the later output Vec push),
owned R+1 AND work R+113=>Name Limit(work),indexSome1/recordSome0 at9 (first name
output-byte work debit BEFORE owned debit); workR+114 with same owned limit gives
the Name Limit(owned_bytes) above. These are manual partial-work sums through the
first member, not thresholds discovered by implementation.
nodes5=>Wire Limit(nodes)29 (third selected node), depth1=>Wire Limit(depth)3,
entries4=>Name Limit(entries),indexNone/recordNone at1,
entries3=>Name Limit(entries),indexNone/recordNone at0 (O(1) d2 preflight, even
with bytes0/records0/work0); d2 lenMAX_RECORDS+1 empty slices fails identically
with all caller limitsusize::MAX, before any d2 iteration,
range_slots3=>Name Limit(range_slots),indexNone/recordSome0 at1,
queries2=>Name Limit(queries),indexSome3/recordNone at29,
output_units2=>Name Limit(output_units),indexSome3/recordSome0 at29.

MINIMAL d1=00 18 00, d2=[], legitimate empty result after complete required-Class
validation. Exact bytes3,records3,nodes2,depth1,work12,owned0,entries0/range0/
queries0/output0. work11=>Wire Limit(work)1 (second fq dispatch); records2=>Wire
Limit(records)1. bytes2=>Wire Limit(bytes)0. nodes1=>Wire Limit(nodes)1.
00 alone=>Class MissingFqName1; 00 08 06=>Class MissingFqName3.
00 18 00 00=>Class Wire InvalidFieldNumber3. Empty d1=>Wire Truncated0.
00 18 00 48 00 50 00=>empty; wrong-wire member tags do not select.

Selected failures with table/d2 from MIXED (prefix04 0a02 0804; Class begins5):
- Class18 00 4a 00 => Function MissingName9.
- Class18 00 52 00 => Property MissingName9.
- Class18 00 4a 01 80 => Function Wire Truncated10.
- Class18 00 52 01 00 => Property Wire InvalidFieldNumber9.
- Class18 00 4a 02 10 04 => Name InvalidIndex,indexSome4,recordNone,offset9.
- Class18 00 4a 06 10 ff ff ff ff 0f => Name InvalidIndex,indexSome(-1),recordNone,offset9.
- Class18 00 4a 04 10 01 0b => outer truncated member length fails Class Wire
  Truncated12; use length03 instead => Function Wire UnsupportedGroup11.
- Class18 00 4a 02 10 01 00 => Class Wire InvalidFieldNumber11 BEFORE member query.
Add matching Property bad index/malformed/group cases. Present name never skips
later structural failures. Unselected malformed nested bytes pass.

UnsupportedUTF8 selected literal d1=05 0a 03 32 01 ff 18 00 4a 02 10 00,d2=[]:
Name UnsupportedUtf8,indexSome0,recordSome0,offset10. Changing Class tail to1800
only => empty success (bad protobuf string bytes unselected). Separate d2 lone
surrogate, NUL and empty-name variants retain [55296], [0], [] exactly; name
canonicality and semantic identifier validity are intentionally outside scope.
Duplicate scalar and wrong-wire-name checks reuse existing literal header fixtures;
no names/flags derived from the new implementation as expected output.

Unit.class integration MUST call parse_class -> annotations::decode -> metadata::
decode -> NEW class_member_names::decode, rather than repeat existing manual
traversal. Existing licensed Unit.class781 bytes SHA256
896588212a98f36148f2fbe1ea2878d6ba23c6376c6346c765cec5ac0c5716ce.
Expect ONE Function ordinal4, field37..49/body39..49,name UTF16
[116,111,83,116,114,105,110,103],FSome65558/ONone/GNone/SNone. Genuine Function
body literal10031a02300448968004; Class19..52. Unit Class fields flags ordinal0,
fq ordinal1, supertype ordinal2, constructor ordinal3, Function ordinal4,
extension ordinal5. No new compiler/JVM/oracle/artifact download.

Every byte cut of MIXED n0..43 is frozen here. Envelope Wire Truncated at n for
n=[0,1,2,3,4]. n5=>Class MissingFqName5. Class Wire Truncated at n for
n=[6,8,9,10,11,12,13,14,16,17,18,19,20,21,22,23,24,25,26,28,29,30,32,33,35,36,
38,40,41,42]. Success cuts: n7=>[]; n15=>first record; n27=>first2;
n31,34,37,39,43=>all3. Every successful record retains the above absolute spans
and exact flag/name values. Complete field-boundary prefixes may succeed without
claiming completeness of an unframed message. MINIMAL n0=>Wire Truncated0;
n1=>Class MissingFqName1; n2=>Class Wire Truncated2; n3=>[].

Finite adversarial tests: these every-byte cuts, selected low32/nonminimal/negative
indices and flags, wrong-wire member/names,
malformed outer tails and selected bodies, opaque nested bodies, aggregate budgets
across duplicate members and UTF16 names, bounded deterministic short-input probes.
No allocation from declared lengths. usize::MAX clamp checks reuse hard byte/record
limits; do not claim finite probes prove all-input/RSS/CPU/stack/JVM compatibility.

## Acceptance and honest limits

Same architect/correctness/adversarial agents review this contract, changed source,
actionable fixes and original completed CI. Root admits shared helper/export slot
before edit. No local native execution; local fmt/compile-only Clippy only after
coordinator permission. Original unchanged Linux Native Rust CI provides pinned
fmt/clippy/full-workspace tests; preserve first failures, no unchanged retry.
Publication issue/PR then push/attach requires root admission; exclusive expected
head/base lease and original reviewed passing CI required before merge. Root
pulls reports. No independent integration or status promotion.

T008/T008a HOLD; broader C13/B3/B4 GAP; B11 UNVERIFIED; #7/#83/#96 OPEN and Darwin
UNKNOWN. #68/#18 remain OPEN. No library semantic reconstruction, SymbolIds,
types, JAR ingestion, server provider, parity or performance claims. Selected
malformedUTF8 stays UnsupportedUtf8; preexisting other APIs remain separately
budgeted. New operation shares only its stated already-decoded-input boundary.

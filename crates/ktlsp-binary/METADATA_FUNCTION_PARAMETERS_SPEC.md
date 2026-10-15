# Function/ValueParameter source admission

Exact source-only grant at fe280c5fab13cde5a9bdf50893aee859fc626a2a;
contract seal SHA256 f30e3c4d123eb8e4f7aa591ca10983f14ab1724f34fe9a32dcefd61e46a9c81e.
Root admitted exactly nineteen new paths and one additive module export, local
format and compile-only locked workspace all-target Clippy. No local runtime,
commit, publication, push, PR, CI or merge is admitted. Source reviews/checks
are recorded separately in METADATA_FUNCTION_PARAMETERS_REVIEW.md.

The frozen historical preparation contract follows unchanged. Its pending grant
wording describes that earlier preparation stage, not the later source grant.

---

# T018/#18/#68 Function selected ValueParameter composition — preparation v1

Temporary contract/data only. Source, execution, commit, publication, push, PR,
CI and merge are held pending a separate exact root grant. No change to feature
scope or public status. Full metadata/semantic/platform/performance parity stays
UNVERIFIED; T008/T008a HOLD, C13/B3/B4 GAP, B11 UNVERIFIED, #96 Darwin UNKNOWN.

## Input identity and scope

Own managed worktree /Users/anil/.codex/worktrees/t018-function-parameters-contract/ktlsp,
branch codex/t018-function-parameters-contract, HEAD fe280c5fab13cde5a9bdf50893aee859fc626a2a,
tree b32813f2f200741eb17365a6d7ef00155ce107de, sole parent
77bd0ac8b4e74dadd037a3404b82dba7e004c395. Root delivery clear file
/private/tmp/ktlsp-root-241-owner-delivery-clear.json SHA256
2404ed741c00ceeac79e747d65ad3a630e7ea1f2b6e5ec621df190022f300f27.
Root reported original exact-main 618=605 ordinary+13 compile-fail, 0 failures,
2 exercised ignored helpers, 22 protocol cases, run38624764002/job116060993360.
Those are historical integration receipts, not this contract's validation.

Decode ONE unframed caller-selected Function body. Caller independently selects
supported mv=[2,3,0] and kind/container; bytes cannot enforce those selections.
Return existing FunctionHeader plus one ordered vector of typed field6/wire2
ValueParameter occurrences. Never merge, sort, deduplicate or apply name/type
lookup, flag defaults/conversion, inline/ID precedence, linkage or semantics.
All nested child Types/annotations/defaults/extensions remain opaque. Unselected
Function Types/IDs/extensions stay opaque. No FunctionTypes245 dependency.

FunctionHeader already retains only raw/flags/old_flags/name: no opaque child
directory exists. Reuse unchanged whole-parent and whole-child budget helpers.
No parent staging, helper redesign, recursion, cache or dependency is proposed.

## Exact prospective API

New metadata::function_parameters, public string_table::Limits re-export:

```rust
#[derive(Debug, PartialEq, Eq)]
pub struct SelectedParameter<'a> {
    pub span: std::ops::Range<usize>,
    pub parameter: super::value_parameter::ValueParameter<'a>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct FunctionWithParameters<'a> {
    pub header: super::function_header::FunctionHeader<'a>,
    pub parameters: Vec<SelectedParameter<'a>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Wire(super::wire::ErrorKind),
    Function(super::function_header::ErrorKind),
    Parameter(super::value_parameter::ErrorKind),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error { pub offset: usize, pub kind: ErrorKind }
pub fn decode(bytes: &[u8], limits: Limits) -> Result<FunctionWithParameters<'_>, Error>;
```

Error Display/std::error::Error/From<wire::Error> follow class_member_names.
Outer preflight/node/selection/push faults use Wire; complete Function helper
faults use Function(kind) at unchanged offset; child helper faults use
Parameter(kind) at body_start+child_offset ONCE. This preserves origin even for
wire budget errors. Parent MissingName and child MissingName are distinct.
Public structs remain caller-constructible; no validated raw/typed equivalence
guarantee for fabricated values. All offsets/spans are Function-relative.

header.raw equals and points to the complete supplied bytes. Every parameter.raw
equals and points to bytes[span.clone()], with body-only ordered nonoverlapping
spans. No owned raw copy/field-number redundancy; only field6/wire2 is selected.
Selected repeated bodies remain separate, even identical names/bytes.

Standalone function_header/value_parameter APIs, error kinds, limits, pointer
borrowing, raw bytes, accepted opaque bodies and tests remain byte-for-byte
unchanged. Existing callers class_member_names/package_member_names, constructor,
value_parameter_types and existing fixture tests need no migration. The only new
caller is the focused new test. No production/server consumer or capability.

## Required control flow and aggregate budget

1. Length preflight: len>min(bytes,MAX_BYTES) => Wire(Limit(bytes))@0.
2. One string_table::Budget::new(limits), root node(1,0).
3. function_header::decode_with_budget(bytes,&mut budget.wire) finishes the ENTIRE
   parent framing/scalars/required-name check before selection starts.
4. Charged second scan: wire::field(bytes,&mut pos,len,&mut budget.wire), then
   work(1,field.span.start) before selection for EVERY field, including ignored.
5. For (6,Value::Bytes(raw)), body span=end-raw.len()..end; node(2,body_start),
   unchanged value_parameter::decode_with_budget(raw,&mut budget.wire), then
   budget.push(one shared parameters vector,SelectedParameter,outer_tag_start).
6. Return only after complete second scan, without partial output on any failure.

Parent malformed tail/invalid wire/required-name errors precede every child
fault. Valid wrong-wire field6 values are ignored/raw, not errors themselves;
their malformed framing still fails the parent. Child failure precedes capacity
reservation, but child node admission precedes child parsing. No reset/refund or
fresh per-child budgets. Rebase child decode failures only; node/push offsets are
already Function-relative. Selected Type bodies inside a child are never decoded.

Admission order from unchanged helpers: node checks depth, consumes node, then
work1; wire field consumes record, then work1, then bytes, then dispatch work1;
push consumes logical owned capacity, then reservation work, then allocator.
Failed subtraction leaves that counter unchanged; earlier successful admissions
remain consumed, including node/record/owned when following work admission fails.
No retry or rollback. Failed child and prior successful children consume work
even though no successful result escapes. Allocation error is Limit(allocation)
at outer tag. Logical requested retained vector capacity excludes allocator
surplus, reallocation churn, stack, CPU/walltime and RSS.

Let N=len,F=parent field count, m=selected valid children, ni/fi=child lengths/field
counts, S=size_of::<SelectedParameter>(), q=0 when m0 else next_power_of_two(m).
For success: records=2F+sum(fi), nodes=1+m, depth=1 if m0 else2,
owned=q*S, work=1+2N+4F+sum(1+ni+2fi)+q*S. Each outer scan pays for every byte,
including all opaque child bytes; child decoding pays again. Capacity growth
increments1,1,2,4... slots for lengths0,1,2,4...; one vector spans all occurrences.
Use symbolic S; no ABI-size claim or runtime size measurement during preparation.
Limits clamp bytes16MiB, records/nodes65535, depth2, work64Mi, owned16MiB.
Other header/d1/table/wire/name operations remain separately bounded; this is only
the Function composition operation, not a whole-pipeline bound.

## Literal cases and future tests

FIXTURES.json freezes authored literals, tuples, errors, minima and pointer spans.
Parent tuple order(flags,old_flags,name); parameter tuple(flags,name,type_id,
vararg_element_type_id), null denotes absence. No expected values come from a new
decoder. Required tests: exact authored fixtures; duplicate order/signed presence;
wrong-wire and opaque unknown; Function-first precedence; rebasing after prefixes;
all six budget minima and one-less; competing limit/failure admission order;
growth1..5; hard clamps; genuine Pair pipeline; every byte prefix; finite hostile
determinism/bounds. New nontrivial source must have these focused regressions.

Simple 10 00 costs R2,N1,D1,W9,O0; selected 10 00 32 02 10 01 costs
R5,N2,D2,W26+S,O=S. Empty parent at defaults =>Function(MissingName)@0;
empty parent with all limits0 =>Wire(Limit(depth))@0. Raw parent empty with its
bytes/records/work all0 remains its original MissingName@0.
10 00 32 00 reaches Parameter(MissingName)@4 after R4,N2,W18,O0; it never reserves.
With nodes1 or depth1, child node fails Wire(nodes/depth)@4 first.
With owned0 and otherwise enough budget, empty/malformed child fault still wins.

For k1..5 named children 10 00 + k*(32 02 10 00):
R=2+3k,N=1+k,D2,W=9+17k+qS,O=qS, body spans4+4i..6+4i.
One-less records offsets4,8,12,16,20 (child local field rebased);
one-less work offsets2,6,10,16,18;
one-less owned offsets2,6,10,10,18;
one-less nodes offsets4,8,12,16,20; depth1 at4; bytes len-1 at0.
For k4, work-one-less is Parameter(Wire(Limit(work)))@16 (final child dispatch,
no capacity growth); k1/2/3/5 fail outer Wire(Limit(work)) at vector reservation.
No owned-minus-one case exists when the exact minimum is0; assert owned0 success.
With usize::MAX limits, k21844 succeeds R65534, next k21845 fails
Wire(Limit(records)) at outer tag87378. Hard bytes fails MAX_BYTES+1@0.
No unreachable hard work/storage boundary is fabricated: caller-small boundaries
exercise those counters. Clamp with all usize::MAX and a normal valid input.

Every-prefix frozen genuine controls: copy length47 success cuts2,18,28,38,42,47;
equals length20 success cuts2,6,16,20. Cut0 Function(MissingName)@0; every other
cut Function(Wire(Truncated))@cut. Partial flags become present copy@42,
equals@20; children appear copy@28/38, equals@16. Also freeze every prefix of
minimal/selected/duplicates/wrong_wire/opaque/signed/nonminimal fixture literals
using the separately listed per-fixture prefix tables. Do not execute until grant.

Finite hostile suite exact future enumeration, not executed here: all65536 byte
pairs [a,b] and [10,00,32,02,a,b] plus [10,00,32,04,10,00,a,b], decoded twice
under Limits{bytes32,records16,nodes8,depth2,work256,owned4096}; all byte values
0..255 repeated lengths0..15 as parents and named child wrappers (body begins
10 00 and repeated data length<=15). No randomness/unbounded recursion. Require
equal results, error.offset<=len, on success exact parent/child borrowing and
ordered spans; only fixed field6 wrappers require child span4..6 or4..8 and
unchanged parent name0. Raw pair/repeated-parent probes impose no parent-name
value expectation (for example 10 07 validly has name7).
For named wrappers success requires one child, with name last-recognized-wins;
do not assume name0 after later recognized fields. Extra optional decoded fields
are not guessed. Allocation failure, if any, remains a controlled error at tag2.

New-module unit checks may use existing private budgets without modifying them:
node budget nodes1/depth1/work0 first node =>work then second =>nodes;
wire budget records1/work0 first record =>work then second =>records;
push budget ownedS/work0 first empty-vector push =>work with owned0/len0,
second identical push =>owned_bytes. These prove earlier admissions persist
when a later admission fails, without a successful allocation or runtime here.

## Genuine existing-data controls and provenance

Reuse ONLY committed metadata_constructor/Pair.class.bin and PAIR_EXTRACTION.json;
no jar/compiler search, download, fresh oracle or Java execution. Preserve existing
notices by byte-identical copy to new fixture directory; do not edit old fixtures.
PROVENANCE.json pins exact class/d1 and full Function/child bytes and slices.
Pair class2983 SHA8fed0fa88eb2df5723812a95330dee6bc91fd66b13906acb7906f9a813da9736;
CP89 MUTF8 class span1276..1599; decoded d1 length285
SHAa4c2bf2d0bcd52aac574d156b413e3926cec249c5c2a23259d126c68872afa8e;
Class body47..285. Distribution kotlin-stdlib2.3.20 is not compiler proof;
compiler provenance remains UNVERIFIED. Source Kotlin v2.3.20
d57eb4a26ed01ab03cf195da686c8047032ff16f; metadata.proto
SHAa08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2;
ProtoBuf.java SHAda7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716.
These existing pins are reused, not new source/oracle verification.

Future genuine test: parse_class(existing Pair)->annotations::decode->metadata::decode
(k1,mv[2,3,0])->wire::decode (Class payload47..285)->select exact Class field9
Function copy field149..198/body151..198 and equals field198..220/body200..220
by spans/literal bytes (do not reconstruct names)->new decode on that borrowed body.
Assert Function/child raw pointers to original decoded d1 slices, exact parent
and child scalar tuples, offsets, order and same-budget minima. Prior pipeline
stages have their existing separate budgets; this is a data pipeline control,
not semantic symbols/name resolution/provider support. Authored scalars and
real-pipeline controls remain explicitly distinct.
Copy R18,N3,D2,W149+2S,O2S, child body20..28/30..38;
equals R10,N2,D2,W70+S,OS, child body8..16.

## Exact future path allowlist (root approval still required)

NEW only:
- crates/ktlsp-binary/src/metadata/function_parameters.rs
- crates/ktlsp-binary/tests/metadata_function_parameters.rs
- crates/ktlsp-binary/METADATA_FUNCTION_PARAMETERS_SPEC.md
- crates/ktlsp-binary/METADATA_FUNCTION_PARAMETERS_REVIEW.md
- crates/ktlsp-binary/tests/fixtures/metadata_function_parameters/minimal.hex
- crates/ktlsp-binary/tests/fixtures/metadata_function_parameters/selected.hex
- crates/ktlsp-binary/tests/fixtures/metadata_function_parameters/duplicates.hex
- crates/ktlsp-binary/tests/fixtures/metadata_function_parameters/wrong_wire.hex
- crates/ktlsp-binary/tests/fixtures/metadata_function_parameters/opaque.hex
- crates/ktlsp-binary/tests/fixtures/metadata_function_parameters/signed.hex
- crates/ktlsp-binary/tests/fixtures/metadata_function_parameters/nonminimal.hex
- crates/ktlsp-binary/tests/fixtures/metadata_function_parameters/pair_copy.hex
- crates/ktlsp-binary/tests/fixtures/metadata_function_parameters/pair_equals.hex
- crates/ktlsp-binary/tests/fixtures/metadata_function_parameters/PROVENANCE.json
- crates/ktlsp-binary/tests/fixtures/metadata_function_parameters/README.md
- crates/ktlsp-binary/tests/fixtures/metadata_function_parameters/EXPECTATIONS.md
- crates/ktlsp-binary/tests/fixtures/metadata_function_parameters/LICENSE.txt
- crates/ktlsp-binary/tests/fixtures/metadata_function_parameters/NOTICE.txt
- crates/ktlsp-binary/tests/fixtures/metadata_function_parameters/PROTOBUF_LICENSE.txt

Shared serialized slot ONLY crates/ktlsp-binary/src/metadata/mod.rs additive
pub mod function_parameters; after root grants/reconciles its current identity.
No other existing source/helper/test/fixture, Cargo/lock/dependency/config/workflow,
parser/vendor/server/lib.rs/public status documentation edits. Source references
and this temporary seal stay outside repository; no new script/helper is shipped.

## Review and later delivery gates

Retain SAME3 /root/architect, /root/correctness, /root/adversarial. Distinct actual
contract readbacks and corrected-finding rechecks precede CONTRACT_READY. READY
only seals preparation, never source admission or successful tests. Later grant
must approve exact API/allowlist/base and allowed validation/publication scope.
No local runtime under #96; no current fmt/Clippy/Cargo invocation authorized.
Future delivery needs original hosted CI three unchanged pinned commands,
actual completed CI and source readbacks by SAME3, own branch push, PR attach,
root exclusive expected-head/base lease, separate actual EXIT0/fullPASS read-only
guard, then separate ordinary merge, OWNER+ROOT exact-main proof. No rerun-to-green,
protection bypass, force-main, proxy actor, approval-settings change or main chase.

First branch-write sandbox error is preserved in FIRST_ERROR.txt. Managed creation
succeeded. Root authorized one require_escalated ordinary switch -c following
absent-ref/clean/exact-base checks; it exited0. No alternate checkout or duplication.

# Phase2 source admitted; hosted runtime validation pending

Root granted exact20 paths at initial integration cut
1c41e58daf531d96144686638a84bcac157ea0e3 (tree9a1465b4bb0efcb43d8d7c70979f02d134727c39,
sole parent626d374ee64a8abc3b50cf7ad050a7e72209e10e) after property227 ownership
release. One fast-forward preserved all19 Phase1 bytes, followed by the single
additive type_arguments export retaining jvm_property_signature/all incoming
exports. No later main integration or merge lease is implied by this initial cut.

Pinned local workspace format check passed after confined formatting of the two
new Rust files; first unformatted check/log is preserved. First compile-only
cargo clippy --locked --workspace --all-targets -- -D warnings passed. No local
native/parser/worker/server/tests/bench/oracle/JVM runtime. Publication and ORIGINAL
unchanged hosted Linux Native CI are authorized; runtime outcomes remain pending.
Retained SAME3 must review actual exported combined source and original completed
CI before delivery. First future CI contradiction must STOP for root diagnosis,
without fixture tuning or unchanged rerun. Frozen9/127/15 values and licensed
Pair/notices remain unchanged; selected data composition establishes no semantics.

The Phase1 and contract records below are historical snapshots. Their earlier
unexported/source-held restrictions do not override this specific Phase2 grant.

# Phase 1 uncompiled source preparation

Root admitted exactly16 new paths and three existing reader/test paths at clean
bb64eb5b5200ef801ffb224336bf2eb997b68054, following actual retained SAME3 contract
approval. metadata/mod.rs remains READONLY while property227 owns the export.
This tree is intentionally unexported/uncompiled preparation, not delivered or
runtime-validated. No fmt/compiler/Clippy/local native/tests/publication/CI admission.
Frozen contract below remains authoritative; historical contract-only stage text
records the earlier phase and does not override this narrow source grant.

# T018 child: selected Type -> typed Argument composition

CONTRACT ONLY. No product/test source or runtime evidence. Assigned base:
HEAD bb64eb5b5200ef801ffb224336bf2eb997b68054,
tree 0ebc83052425eefe3747d3f99a3e02e5c6a2190a,
sole parent 048c344c5a75f7cf16cadc627c7ff765c025c7f7.
Managed worktree /Users/anil/.codex/worktrees/t018-type-argument-contract/ktlsp,
branch codex/t018-type-argument-contract; root coordinator
01a1300d-2ba0-7832-97d0-91d070e2d297/local. Original exact-main proof:
/private/tmp/ktlsp-root-219-owner-delivery-clear.json (579/0/2 plus22 protocol).
That receipt validates its exact prior integration, not this proposal.

## Result and boundary

One caller-selected unframed Type body, caller independently verifies supported
metadata mv=[2,3,0]/kind/container. Return existing selected TypeHeader scalars plus
every recognized field2 length-delimited Argument, in encoded order. Decode each
Argument using existing schema logic: projection presence, signed-low32 type_id,
every borrowed inline Type body with Argument-relative spans. Type field2 repeats
are independent entries, never last-win. Argument inline Type field2 repeats are
all retained, never merged. Empty Argument and inline Type occurrences remain.
Unknown/wrong-wire data and order remain in complete borrowed raw input.
Other Type nested fields5/10/13, inline-Type internals and unknown bodies stay opaque.
No recursive Type graph, singular-message merge/initialization, classifier/name or
TypeTable lookup, nullability application, substitution, type semantics/linkage,
server integration, oracle, comparative speed/memory or full metadata/LSP parity.
No new prerequisite is imposed on T008. #18/#68 OPEN; T008/T008a HOLD; broader
C13/B3/B4 GAP; full B11 UNVERIFIED; #7/#83/#96 OPEN, Darwin UNKNOWN.

## Exact prospective API

New metadata::type_arguments module, no aliases or convenience APIs:

```rust
pub use super::string_table::Limits;
pub use super::wire::{Error, ErrorKind};
#[derive(Debug, PartialEq, Eq)]
pub struct SelectedArgument<'a> {
    // Body-only offsets relative to supplied Type bytes.
    pub span: std::ops::Range<usize>,
    pub argument: super::type_argument::TypeArgument<'a>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct TypeWithArguments<'a> {
    pub header: super::type_header::TypeHeader<'a>,
    pub arguments: Vec<SelectedArgument<'a>>,
}
pub fn decode(bytes: &[u8], limits: Limits) -> Result<TypeWithArguments<'_>, Error>;
```

Limits is the existing string_table::Limits: bytes/records/nodes/depth/work/
owned_bytes, same defaults and hard clamps (depth<=2). All fields are used. No
public existing struct, function signature, type, derives, defaults or error is
changed. Header.raw is complete parent raw; Argument.raw equals parent[span].
InlineType.span remains Argument-relative; parent coordinate is span.start plus
inline span. Pointer identity/containment/order must be asserted including empty
slices. No extra raw copy or body-sized owned allocation.

## Smallest private reuse refactor; later serialized grant required

1. type_header.rs: public decode keeps its current bytes check, creates same
wire::Budget, then calls pub(super) decode_with_budget(bytes,&mut wire::Budget).
Move unchanged initializer/field loop/match into helper; no new byte/node/depth
check inside helper. No schema duplication or field callback framework.
2. type_argument.rs: public decode keeps current bytes check and identical
string_table::Budget initialization, then calls pub(super)
decode_with_budget(bytes,&mut string_table::Budget). Move unchanged initializer/
loop/match/push into helper; no new node/depth check. Preserve Argument-local
inline spans and errors. No inline-Type decoding.
3. New operation checks bytes > min(limits.bytes,MAX_BYTES) first -> bytes@0;
creates ONE string_table::Budget; calls node(1,0); decodes header with its shared
wire budget. Then rescans parent once using wire::field + work(1,field.start).
For ONLY (2,Value::Bytes(raw)): derive body span; call node(2,span.start); call
Argument helper(raw,&mut same budget), map any error.offset by +span.start ONCE;
then budget.push SelectedArgument at parent field tag start. No public decoder
call, fresh child budget, refund/reset, uncharged scan or copied schema logic.
All other parent fields merely undergo bounded wire+dispatch scanning twice.
4. wire.rs/string_table.rs/Budget implementations stay UNCHANGED. Existing helpers
suffice. Root must separately grant existing-reader/export edits after contract
review. No edits to active jvm_property_signature owner paths.

Two parent passes deliberately trade extra bounded work for smallest reusable
refactor. Complete TypeHeader validation precedes any child processing. Thus later
malformed parent tail wins over earlier malformed Argument. Parent rescan is in
encoded order and decodes each selected Argument immediately before the next field.
Malformed child can beat a later rescan resource failure after full header success.
No partial output is returned on any failure. Failed allocation remains Limit
("allocation") from existing helper; deterministic OOM injection is not proposed.

## Aggregate arithmetic (successful structurally valid scans)

Let N=parent bytes; F=parent fields (all wires/unknown included); K=selected
Arguments; Mi/Gi=bytes/fields for Argument i (opaque inline bodies included in Mi
but their internals are never fields); ji=number inline bodies in Argument i.
C(0)=0, C(n>0)=smallest power of two >=n; A=size_of::<SelectedArgument>(),
S=size_of::<InlineType>(). Use Rust size_of on target, never hard-code ABI sizes.

records R=2F+sum Gi; nodes Q=1+K; depth D=1 if K=0 else2;
owned O=C(K)*A+sum C(ji)*S;
work W=1+K+2N+4F+sum(Mi+2Gi)+O.

Each parent pass charges N+2F (wire record work1, byte reads N, dispatch1).
Each child node charges1. Argument loop charges Mi+2Gi plus its vector growth;
root node charges1. Logical vector growth is 1/1/2/4/... slots. All vectors share
ONE owned balance; owned growth also charges work before reserve. No RSS/allocator
surplus/reallocation-churn/whole-pipeline memory claim. Byte ceiling applies to
parent once; child bytes are a subset, no new byte limit/reset. Overlapping reads
are counted in work, not bytes. All caller limits may tighten, never raise hard
caps. Exact minima and +1 pass; deficits fail at explicit first charge, not simply
'at EOF'. Pair: N14/F3/K2/M=[4,4]/G=[1,1]/j=[1,1], R8,Q3,D2,
O=2A+2S,W=55+2A+2S; inline bodies never consume depth3.

## Error order and coordinates

Global admission bytes -> root depth -> root nodes -> root node work. Empty input
with bytes0/records0/nodes1/depth1/work1/owned0 succeeds; all0 -> depth@0;
depth1/nodes0 ->nodes@0; depth1/nodes1/work0 ->work@0.
Wire field: record count -> record work -> tag reads -> tag validity -> value
reads/length/bulk bounds -> schema dispatch work. Wire short reads debit remaining
bytes+1 before Truncated@end. Do not replace with len-only work estimates.
Child: parent wire+dispatch first -> depth2 -> nodes -> node work -> existing
Argument field loop -> child inline vector owned -> inline capacity work ->
reserve/allocation -> finally outer vector owned -> outer capacity work -> reserve.
node errors already use parent body-start. Argument errors map once to Type frame;
Type header/rescan/outer push errors already use Type frame. Errors are never d1
coordinates. All successful spans remain within their declared frame.
Unknown enums do not erase recognized projections, even low32-negative encodings;
wrong-wire known fields do not erase recognized values. Scalar recognized values
last-win; booleans nonzero true; int values signed low32; wire slow-path low64 rule,
nonminimal tags/length low32, unsupported groups and malformed/negative lengths
unchanged. No guessed absent defaults or validity requirements (STAR+type allowed).

## Frozen expectations before runtime

EXPECTATIONS.json contains independent literal successes, malformed cases and full
prefix outcomes for EVERY prefix of EVERY frozen valid literal. Prefix errors
use default budgets; prefix success inherits only complete top-level fields.
No product import/execution was used. Fixture self-check is limited to literal
lengths/slices/hashes/arithmetic; never a parallel schema decoder.

Distinguishing opaque-inline success:12 03 12 01 00 -> Argument span2..5,
inline raw00/span2..3, projection/type_idNone; malformed inline content stays
unscanned at depth2. Child error mapping after prior sibling12 00 12 01 18 and
nonminimal parent92 00 81 00 18 must each be Truncated@5 (not hardcoded+2).

Exact resource cases for literal 12 00 (N2,F1,K1,M0,G0): R2,Q2,D2,W10+A,O=A.
bytes1 with depth0/nodes0/records0/work0 ->bytes@0.
Default other limits: records0 ->records@0 (header); records1 ->records@0 (rescan).
work0 ->work@0(root); work1 ->work@0(header record work);
work2 ->work@0(tag); work3 ->work@1(length); work4 ->work@0(dispatch);
work5 ->work@0(rescan record work); work6 ->work@0(tag);
work7 ->work@1(length); work8 ->work@0(dispatch); work9 ->work@2(child node);
work10/ownedA ->work@0(outer capacity); work10+A succeeds.
work10/ownedA-1 ->owned_bytes@0 (owned before capacity work).
depth1 ->depth@2; nodes1 ->nodes@2; records1/depth1 ->records@0
(rescan precedes child depth). records0/depth0 ->depth@0(root precedes header).

For Pair exact R8/W55+2A+2S/O2A+2S/Q3/D2 passes; R7 ->records@12
(final parent scalar during rescan); Q2 ->nodes@8; D1 ->depth@2;
O-1 ->owned_bytes@6 (second outer growth); W-1 ->work@12
(final parent scalar dispatch). Separate per-child record1/work6+S/ownedS passes each
standalone Argument but does NOT establish composition: composition records2,
work12+2S or owned2S alone cannot pass aggregate boundaries.

Literal12 02 12 00: R3,Q2,D2,O=A+S,W18+A+S.
owned0 ->owned_bytes@2 (child inline push before outer push);
ownedS ->owned_bytes@0 (outer push); work18/ownedS-1 ->owned_bytes@2;
work17/owned0 ->work@2(child dispatch before owned).
Malformed12 01 00 with owned0/depth2 ->InvalidFieldNumber@2 before outer storage;
depth1 ->depth@2 before child malformed. Parent12 01 00 00 ->
InvalidFieldNumber@3 before child malformed, regardless owned0.

For k1..5 copies12 00: R2k,Q1+k,D2,O=C(k)A,W1+9k+C(k)A.
OwnedO-1 fails first growth at parent tag[0,2,4,4,8]. WorkW-1 with exact O
fails at[0,2,4,8,8] (k4 child-node offset8; others outer growth tag).
For one child with j1..5 empty inline12 00 bodies: parent12 [2j] then j copies
12 00; R2+j,Q2,D2,O=A+C(j)S,W10+8j+A+C(j)S. OwnedO-1 fails outer
push@0 (child capacity paid first); W-1 outer capacity@0. Tight child-owned
C(j)S-1 fails at parent offsets[2,4,6,6,10], BEFORE outer capacity.

Hard caps: bytes MAX_BYTES+1 with usize::MAX others ->bytes@0; caller
usize::MAX is clamped by existing helpers. k32767 empty Arguments has R65534,
Q32768, N65534, W294904+32768A, O32768A (subject to actual size_of, so owned ceiling may fail earlier; do not claim
hard-record witness without checking other bounds). Instead hard records witness parent
65535 copies10 00 with no selected child: header R65535 succeeds but rescan first
record fails@0 under records usize::MAX, if work/bytes suffice; owned0 and Q1.
No invented independent hard-work/owned successful witness. Tightened tests prove
these limits. Allocation mapping/overflow helper review is static-only.

Finite hostile test proposal: all (a,b)0..255 for [a,b] and [12,02,a,b], plus each
a repeated n0..31, twice per input; 139264 inputs/278528 calls, bytes36,records8,
nodes8,depth2,work1024,owned1024. Compare complete Results; success assert all
borrowing/spans/order; error offset<=len. No timing/RSS/oracle inference.

## Genuine Pair assertion and unchanged provenance

Reuse existing metadata_type_argument/Pair.class.bin and notices/provenance ONLY;
no external hunt, extraction/compiler/oracle or artifact regeneration. Class2983,
SHA2568fed0fa88eb2df5723812a95330dee6bc91fd66b13906acb7906f9a813da9736;
JAR0ae12504a5040ebaf37703908483420d1a5624dd1d93f357665f8c77c848a01e.
Distribution2.3.20-release-208; exact compiler UNVERIFIED. Pair provenance preserved
byte-for-byte; packet PROVENANCE.json pins input hashes/license/notice/schema docs.

Modify ONLY existing genuine_pair_generic_return... assertion/import: retain real
parse_class -> annotations -> metadata decode -> payload/Class -> structural
Function selection by pinned whole47byte literal -> function_header, d1 spans
and Resolver name expectations. Function-to-selected-Type framing stays current
literal-guarded4..18, explicitly separately bounded/outside new operation.
Replace TypeHeader decode+fixed-offset nested Argument slicing+public per-child
decodes with ONE type_arguments::decode(selected14bytes, exact Pair limits).
Compare complete header to existing expected TypeHeader(class_nameSome0/all else
None); assert selected argument spans2..6/8..12 structurally returned, raw/pointers
d1[157..161]/[163..167], inline raws38 00/38 01 and spans2..4, projection/type_id
None for each. Keep separately bounded inline TypeHeader calls type_parameter
Some0/Some1, and Resolver UTF16 kotlin/Pair, copy. Inline-Type interpretation,
header/framing/resolver operations retain separate budgets, never included in Pair
aggregate. Existing standalone Argument/Header fixtures/test behavior unchanged.
New authored Pair Type14byte literal is copied from already pinned extraction
only and named separately from actual genuine assertion; manual bytes are not
runtime success. Artifact/license/notice hashes must be checked before later CI.

## Exact prospective file allowlist and callers

NEW:
crates/ktlsp-binary/src/metadata/type_arguments.rs
crates/ktlsp-binary/tests/metadata_type_arguments.rs
crates/ktlsp-binary/METADATA_TYPE_ARGUMENTS_SPEC.md
crates/ktlsp-binary/METADATA_TYPE_ARGUMENTS_REVIEW.md
crates/ktlsp-binary/tests/fixtures/metadata_type_arguments/{selected.hex,duplicates.hex,
opaque.hex,opaque_inline.hex,enum.hex,signed.hex,nonminimal.hex,wrong_wire.hex,pair_type.hex,
EXPECTATIONS.md,README.md,PROVENANCE.json}
EXISTING (later ROOT-SERIALIZED explicit source/export grant):
crates/ktlsp-binary/src/metadata/type_header.rs (private helper extraction only)
crates/ktlsp-binary/src/metadata/type_argument.rs (private helper extraction only)
crates/ktlsp-binary/src/metadata/mod.rs (sole additive pub mod type_arguments;)
crates/ktlsp-binary/tests/metadata_type_argument.rs (genuine test+import only)
No config/Cargo/workspace/lock/dependency/workflow/vendor/parser/server/rootdocs,
wire/string_table/helper implementation, or jvm_property_signature owner changes.
No other source callers exist at base: rg TypeHeader/TypeArgument identifies only
these modules and their tests/docs. Public exports and callers retain outputs;
new consumer only tests. Original standalone unit tests must pass unchanged.

## Gates and reviewers

SAME3 retained: architect + separate correctness/functionality + adversarial edge/
crash reviewers reserved by root. Actual contract reviews/fixes/rechecks required
before READY. No source admission implied by READY. No fmt/compiler/Clippy/native
runtime/tests/bench/oracle/JVM/upstream-code execution, push/issue/PR/CI/publication
in this stage. Temp contract/literal/hash Python only. No framework if a helper
proves insufficient: stop and specify need to root.
Later root grants serialized source/export ownership and local fmt+compile-only
Clippy; runtime remains held under96 and runs only ORIGINAL unchanged hosted Linux
Native CI. Retain first failures, SAME3 source and completed-original-CI reviews.
Normal own-branch push/open+ATTACH PR, root stable cut/exclusive expected-head/base
lease, separate strictly READONLY enved fresh guard actual EXIT0+complete exact
facts, separate expected-head squash merge, OWNER+ROOT exact tree/sole parent/
original exact-main CI proof. No rerun-to-green/main chasing, bypass/admin/force/
approval setting changes. Capacity error holds same agents/model, no duplicates.

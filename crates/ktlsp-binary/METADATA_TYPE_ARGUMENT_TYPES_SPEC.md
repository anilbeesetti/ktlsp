# Source preparation admitted; runtime validation pending

ROOT admitted the exact20 paths on clean45619398c7d8d5ca7b59acc5178acb4b208a67ee,
tree7049d191752446b7b696e27c4d08ac12a93798fc, soleparent0007ac79eefc0771f06c26596f81baeaf5379312.
Admission SHA256f8d3f313b14ce56d1adbcfc567865836ffeddabd0f29cbe6300b13dd90aec398.
Frozen contract027a11442a9e8ec35dc96d6daefe6ee15b8771953012628d85e40a870510a4f7,
expectations86425365e8d8270e6f4b35137566bfdfb023dccfcf2a4ed139d993051f7c5c92.
Local format and compile-only locked Clippy are admitted; no local product/parser/
worker/server/tests/bench/native/JVM/oracle runtime. No commit/publication/hosted
CI/merge grant. This stage supersedes the historical PREP-only stage text below,
without changing its frozen API/literals/arithmetic/limits. Source reviews and
check outcomes are recorded separately in METADATA_TYPE_ARGUMENT_TYPES_REVIEW.md.

# T018/#18/#68 Type.Argument -> selected inline TypeHeader contract v1

PREP ONLY. Exact setup recovered and verified; SOURCE admission pending. No source/runtime/compiler/
fmt/Clippy/Cargo/commit/push/ticket/publication/CI grant. Initial actual clean
worktree/base and first setup failure preserved in SETUP.json. Assigned existing
worktree /Users/anil/.codex/worktrees/3d8f/ktlsp; no duplicate worktree/fallback.
Verified HEAD45619398c7d8d5ca7b59acc5178acb4b208a67ee,
tree7049d191752446b7b696e27c4d08ac12a93798fc, soleparent0007ac79eefc0771f06c26596f81baeaf5379312,
branch codex/t018-type-argument-types-contract; clean. Initial255 filesystem and
postfetch128 DNS guard failures remain historical in SETUP.json; ROOT granted
permission-correct setup recovery and separate successful ordinary branch switch.
ROOT chat01a1300d-2ba0-7832-97d0-91d070e2d297/local. Retained reviewers:
/root/architect, /root/correctness, /root/adversarial. Two review slots granted.

## Result, public API and exact boundary

Decode ONE caller-selected unframed Argument for independently selected metadata
mv=[2,3,0]/kind/container. Add metadata::type_argument_types only:

```rust
pub use super::string_table::Limits;
pub use super::wire::{Error, ErrorKind};
#[derive(Debug, PartialEq, Eq)]
pub struct SelectedType<'a> {
    /// Body-only offsets relative to supplied Argument bytes.
    pub span: std::ops::Range<usize>,
    pub header: super::type_header::TypeHeader<'a>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct TypeArgumentWithTypes<'a> {
    pub raw: &'a [u8],
    pub projection: Option<super::type_argument::Projection>,
    pub type_id: Option<i32>,
    pub inline_types: Vec<SelectedType<'a>>,
}
pub fn decode(bytes: &[u8], limits: Limits) -> Result<TypeArgumentWithTypes<'_>, Error>;
```

Return flattened parent scalars and ONE typed vector. Consume existing opaque
staging vector, do not return it or retain a TypeArgument parent wrapper. Every
recognized field2/wire2 occurrence remains ordered, separate, unmerged, including
empty. All10 TypeHeader scalar presence/raw conversions are inherited unchanged.
No existing public API, struct fields, derives, defaults or errors change.
Success borrows entire Argument raw and every TypeHeader.raw, with pointer identity
and exact slices bytes[span], including empty bodies. No body copies. No recursive
Type graph, singular-message merge, initialization/required fields, projection
semantics, name/symbol/TypeTable resolution, Kotlin reconstruction, nullability
application, semantic server consumer or comparative timing/RSS/parity claim.
Opaque nested Type messages/extensions remain opaque even if their contents fail
protobuf. The new selected Type itself now gets a bounded wire scan.
Parents #17/#18/#68 remain OPEN; T008/T008a HOLD, C13/B3/B4 GAP, B11 UNVERIFIED,
Darwin UNKNOWN, Windows unresolved. Completed research is reused, not restarted.

## Minimal implementation and deterministic order

1. Enforce bytes > min(limits.bytes, MAX_BYTES) -> Limit(bytes)@0 first.
2. Create ONE existing string_table::Budget, call node(1,0).
3. Fully call type_argument::decode_with_budget(bytes,&mut budget). This performs
   complete parent framing/dispatch and opaque staging Vec reservations first.
4. Move raw/projection/type_id out, consume inline_types in encoded order.
5. For each opaque child, call budget.node(2,span.start); then allocation-free
   type_header::decode_with_budget(child.raw,&mut budget.wire). Rebase ONLY that
   helper's errors by span.start ONCE. Node errors already Argument-relative.
6. budget.push typed SelectedType at span.start. Push errors already Argument-
   relative; never rebase them. Consume/move span, no extra raw/span vector.
7. Return flattened result after all occurrences succeed; any failure drops all
   partial results. No public decoder calls, budget resets/refunds, helper edits,
   tag-start guesses, second parent scan, recursive callbacks or uncharged work.

Parent staging reserves at field TAG start, result reserves at BODY start. This
is intentional: nonminimal tags/lengths mean body.start-2 is not the tag. Parent
syntax errors (and legitimate earlier staging/resource errors) precede ALL child
node/header/result errors. Under adequate parent budget, malformed parent tail
wins over an earlier malformed child. Budget changes never promise syntax-error
precedence over resource errors. Child order determines first child failure.

## Aggregate arithmetic and resource precedence

N=Argument length, F=parent field count, K=recognized inline Type occurrence count;
Mi/Gi=child i byte length/field count (opaque grandchildren included in Mi only).
C(0)=0; C(K>0)=smallest power2 >= K. S=size_of::<type_argument::InlineType>(),
T=size_of::<SelectedType>(); use target Rust size_of, never ABI literals.
records R=F+sum Gi; nodes Q=1+K; depth D=1 if K=0 else2;
owned O=C(K)*(S+T);
work W=1+N+2F+K+sum(Mi+2Gi)+O.
Both staging and typed-result reservations monotonically consume ONE balance;
dropping or consuming staging NEVER refunds charges. Requested capacity growth
1/1/2/4/... is logical, excluding allocator surplus/churn/stack/RSS; no pipeline
memory bound. Each wire field charges record work1+bytes+dispatch1. Nodes cost1.
Owned debit precedes capacity-work debit and try_reserve_exact; allocation errors
map through existing Limit(allocation). All input spans are bounded by16MiB, so
once-rebase additions cannot overflow usize on supported32/64-bit targets.
Existing hard clamps: bytes16MiB, records/nodes65535, depth2, work64Mi units,
owned16MiB. No independent child byte ceiling because its raw is a parent subset.

Entry precedence: bytes -> root depth -> root nodes -> root work. Parent: records
-> record work -> tag reads -> tag validity -> value/length/bulk -> dispatch work
-> opaque vector owned -> capacity work -> reserve. After COMPLETE parent:
child depth -> nodes -> node work -> same wire order -> result owned -> capacity
work -> reserve. Wire short-read work debit remaining+1 precedes Truncated@end.
Unknown/wrong-wire fields still pay reads/dispatch but no selected-vector entry.

## Frozen independent successful literals

Unspecified scalars None. Full raw/pointer/span equality applies to every result.
Use repository input literals unchanged for selected/duplicates; copy bytes only
as independently frozen new fixtures. Genuine Pair fixture/notices stay readonly.

| name | bytes | parent | ordered child span/header | R,Q,D,O,W |
| --- | --- | --- | --- | --- |
| empty | empty | None,None | [] | 0,1,1,0,1 |
| empty_inline | 12 00 | None,None | 2..2/all None | 1,2,2,S+T,6+S+T |
| pair0 | 12 02 38 00 | None,None | 2..4/type_parameter0 | 2,2,2,S+T,12+S+T |
| pair1 | 12 02 38 01 | None,None | 2..4/type_parameter1 | 2,2,2,S+T,12+S+T |
| selected | 08 02 12 02 30 04 18 00 | Inv,Some0 | 4..6/class_name4 | 4,2,2,S+T,20+S+T |
| duplicates | 08 00 08 63 12 02 30 04 12 02 18 01 12 00 18 01 18 ff ff ff ff 0f 08 03 08 ff ff ff ff 0f | Star,Some(-1) | 6..8/class_name4;10..12/nullabletrue;14..14/all None | 11,4,2,4(S+T),60+4(S+T) |
| repeated | 12 02 38 00 12 02 38 01 | None,None | 2..4/parameter0;6..8/parameter1 | 4,3,2,2(S+T),23+2(S+T) |
| wrong_wire | 0a 01 80 10 01 1a 01 80 | None,None | [] | 3,1,1,0,15 |
| opaque_nested | 12 04 12 02 00 80 | None,None | 2..6/all None | 2,2,2,S+T,16+S+T |
| opaque_extension | 12 05 a2 06 02 00 80 | None,None | 2..7/all None | 2,2,2,S+T,18+S+T |
| child_wrong_wire | 12 03 3a 01 80 | None,None | 2..5/all None | 2,2,2,S+T,14+S+T |
| nonminimal_empty | 92 00 80 00 | None,None | 4..4/all None | 1,2,2,S+T,8+S+T |

Complete per-prefix partition frozen below; all absent prefixes return defaults.
T means Truncated@prefix length unless an earlier fully selected child error is
specified (none in these successful literals). At complete field boundaries,
selected values are exactly those of fully retained fields; never initialization.
- empty N0 success[0].
- empty_inline N2 success[0,2], T[1].
- pair0/pair1 N4 success[0,4], T[1,2,3].
- selected N8 success[0,2,6,8], T[1,3,4,5,7].
- duplicates N30 success[0,2,4,8,12,14,16,22,24,30], all other cuts T.
- repeated N8 success[0,4,8], all other cuts T.
- wrong_wire N8 success[0,3,5,8], all other cuts T.
- opaque_nested N6 success[0,6], all other cuts T.
- opaque_extension N7 success[0,7], all other cuts T.
- child_wrong_wire N5 success[0,5], all other cuts T.
- nonminimal_empty N4 success[0,4], all other cuts T.
Every cut0..=N must be asserted; exact full expected successes/errors for all98
cuts are independently frozen in temporary EXPECTATIONS.json before source.

## Error seeds and exact coordinates under adequate budgets

| literal | ErrorKind@Argument-relative offset |
| --- | --- |
| 12 01 80 | Truncated@3 |
| 12 01 00 | InvalidFieldNumber@2 |
| 12 01 0b | UnsupportedGroup@2 |
| 12 01 0e | InvalidWireType@2 |
| 12 01 80 00 | InvalidFieldNumber@3 (parent, before child) |
| 12 01 80 08 | Truncated@4 (parent, before child) |
| 12 00 12 01 80 | Truncated@5 |
| 92 00 81 00 80 | Truncated@5 |
| 12 02 38 00 12 02 08 80 | Truncated@8 |
| 12 06 12 80 80 80 80 08 | NegativeLength@3 |
| 12 0a 80 80 80 80 80 80 80 80 80 80 | MalformedVarint@11 |
| 12 02 ff | Truncated@3 (parent bulk) |
| 12 80 80 80 80 08 | NegativeLength@1 (parent) |

Also inherited parent malformed rows00/03/07 invalidField0;0b/0c group0;
0e/0f invalidWire0;09+7bytes truncated8;0d+3bytes truncated4;ten80 malformed9.
Prepend complete pair0 => same parent error offset+4. Wrap same malformed bodies
in selected inline Type => child error offset+body.start, never double rebase.

## Presence, scalar conversion and opaque controls

For parent projection known00/01/02/03 => In/Out/Inv/Star. For both projection and
parent type_id use signed low32 varints. Unknown projection values never erase
known; verify known/unknown/known and unknown/known/unknown in separate literals.
Type integer fields1/4/6/7/8/9/11/12/14 use signed low32 independently, last-win
within EACH occurrence; child nullable uses full raw64!=0, not low32. Cases:
00,80 00 =>0; ff ff ff ff 0f and ff ff ff ff ff ff ff ff ff 01 =>-1;
80 80 80 80 08 =>MIN; ff ff ff ff 07 =>MAX;80 80 80 80 10=>0.
Bool00/80 00 false,01/02/80 80 80 80 10 true. Raw64 slow-path inherited limit
must stay documented, not universal JVM fast-path compatibility. Duplicate Type
bodies with different scalars MUST NOT merge or clear earlier occurrences.
For EACH parent known field1/2/3 and child selected scalar, wrong wires1/2/5
(or wire0 for parent field2) before/after recognized values retain presence;
length bodies80 remain opaque if field is wrong-wire or unselected. Empty child
all None distinct from explicit scalar zero; parent Inv with no type and Star
with inline type/id both structurally valid, no initialization validation.
Unknown child length fields2/5/10/13/100/101..199 with malformed interior00/80/
0b are opaque framing successes, not recursive errors/depth3. Unknown supported
wire0/1/2/5 fields and reserved/max field numbers remain exact in borrowed raw.

## Exact tightened limits, growth and hard clamps

All table minima and each minimum+1 pass; independently lower each resource by1.
For selected: records3=>records@4;nodes1=>nodes@4;depth1=>depth@4;
owned(S+T)-1=>owned_bytes@4;work(20+S+T)-1=>work@4(result reserve).
For duplicates: records10=>records@10;nodes3=>nodes@14;depth1=>depth@6;
owned4(S+T)-1=>owned_bytes@14;work60+4(S+T)-1=>work@14.
For repeated: records3=>records@6;nodes2=>nodes@6;owned2(S+T)-1/work23+2(S+T)-1
=> respective limit@6. Pair records1/nodes1/depth1/ownedO-1/workW-1 => respective
limit@2;bytes3=>bytes@0 even all other limits0. Empty bytes/records/owned0,
nodes1/depth1/work1 pass; all0=>depth@0;depth1/nodes0=>nodes@0;work0=>work@0.
For12 00: work0(root)@0;work1(record)@0;work2(tag)@0;work3(length)@1;
work4(dispatch)@0;work5(staging-capacity)@0;work5+S(child-node)@2;
work6+S(result-capacity)@2; exact6+S+T passes. With ownedS-1 and work5,
owned_bytes@0 wins before staging capacity-work. With ownedS and work6+S,
owned_bytes@2 wins before result capacity-work. records0/depth1=>records@0;
records1/depth1=>depth@2 AFTER complete staging. depth0/records0=>depth@0.

k1..5 copies12 00: C=[1,2,4,4,8],R=k,Q=1+k,D2,O=C(S+T),W=1+5k+O.
ownedO-1 errors at body[2,4,6,6,10]; workW-1 at body[2,4,6,8,10].
Staging-only limitC*S-1 fails tag[0,2,4,4,8] before ANY child; nodes1 + that
staging deficit must still fail owned rather than child nodes. Prove no refund
by ownedC*S: staging can finish but first typed reserve fails body2. ownedC*T
alone also cannot satisfy aggregate (unless it incidentally exceeds aggregate,
which it cannot for positive S); do not infer target sizes outside Rust size_of.
Opaque_extension and nested exact budgets prove grandchildren unscanned/depth2.
Use caller usize::MAX for each clamps; MAX_BYTES+1=>bytes@0.65535 copies18 00
fits R65535/Q1/O0/W262141 then one extra=>records@131070 with callerusizeMAX.
Empty children65535 pass Q65536? NO: nodes hard65535 fails last child body131070,
provided layout-dependent owned/work hard ceilings permit reaching it; do not
invent that independent hard-node witness. Use tightened node limits instead.
Independent hard-work/owned exhaustion not claimed; allocation failure/overflow
mapping static review only, no deterministic OOM or huge allocation test.

Finite hostile probe eventual test: all a,b0..255 for[a,b] and[12,02,a,b], plus
all a repeated n0..31;139264 inputs, twice=>278528 calls. Limits bytes36/records8/
nodes8/depth2/work1024/owned1024. Equal complete Results, success raw/pointer/
ordered span assertions, errors offset<=len. No local execution admission.

## Licensed genuine Pair pipeline and touched callers

Reuse unchanged fixture metadata_type_argument/Pair.class.bin2983,
SHA2568fed0fa88eb2df5723812a95330dee6bc91fd66b13906acb7906f9a813da9736.
Existing Pair JAR SHA2560ae12504a5040ebaf37703908483420d1a5624dd1d93f357665f8c77c848a01e,
Kotlin2.3.20-release-208, exact compiler provenance UNVERIFIED. Preserve all
original LICENSE/NOTICE/PROTOBUF_LICENSE/provenance bytes; no artifact/compiler/
JVM/oracle hunt/extraction/regeneration. Input hashes recorded in PINS.json.
Keep actual parse_class->annotations->metadata k1/mv[2,3,0], decoded d1len285;
payload/Class d1span47..285 flags1031/fq0; whole47byte Function literal identity,
field149..198/body151..198;Function name17/flags65734/old_flagsNone; exact raw
pointers; literal guarded selected Type d1[155..169] bytes
1204120238001204120238013000. Keep outer type_arguments::decode with its unchanged
aggregate R8/Q3/D2/O2A+2S/W55+2A+2S (A existing SelectedArgument size).
Keep returned Argument spans2..6/8..12 and d1spans157..161/163..167, original
retained opaque assertions. REPLACE standalone type_header::decode(inner raw)
with new type_argument_types::decode(argument.raw, exact Pair limits), separately
for each Argument: bytes4/R2/Q2/D2/OS+T/W12+S+T. Compare new raw/scalars to already
checked parent, one typed occurrence/span2..4/raw3800 or3801 and pointer to d1
159..161/165..167; assert complete TypeHeader type_parameterSome0/Some1 with all
other9 None. Retain resolver UTF16 kotlin/Pair and copy plus raw d2[17].
Do not duplicate old public TypeHeader decoding; resulting pipeline replaces it.
Each decoder operation separately budgeted, never shared across entire pipeline.

All actual exact456 code callers searched and read: type_argument helper only
type_arguments; type_header helper in type_arguments/class_supertypes/
function_types/type_alias_types/value_parameter_types. These existing consumers
remain readonly/unchanged; no caller source expansion needed. Remaining references
are their focused tests/docs. Incoming mod.rs retains all7 new exports since9b.
New consumer only focused tests and existing genuine Pair test import/assertion.
Existing standalone Argument/Header/TypeArguments tests remain behavior-unchanged.
No source consumers in server/import/semantic modules. Helpers/Cargo/config/
exports/workflows/dependencies/lock/public status readonly except exact export
below AFTER ROOT grant. Added API can only be advertised as bounded data.

## Exact prospective path allowlist, subject to ROOT SOURCE approval

NEW source/test/spec/review:
crates/ktlsp-binary/src/metadata/type_argument_types.rs
crates/ktlsp-binary/tests/metadata_type_argument_types.rs
crates/ktlsp-binary/METADATA_TYPE_ARGUMENT_TYPES_SPEC.md
crates/ktlsp-binary/METADATA_TYPE_ARGUMENT_TYPES_REVIEW.md
NEW fixtures under crates/ktlsp-binary/tests/fixtures/metadata_type_argument_types/:
empty_inline.hex,pair0.hex,pair1.hex,selected.hex,duplicates.hex,repeated.hex,
wrong_wire.hex,opaque_nested.hex,opaque_extension.hex,child_wrong_wire.hex,
nonminimal_empty.hex,README.md,EXPECTATIONS.md,PROVENANCE.json.
EXISTING narrowly:
crates/ktlsp-binary/src/metadata/mod.rs (one pub mod type_argument_types export)
crates/ktlsp-binary/tests/metadata_type_argument.rs (import/genuine test only).
Total20 paths:18 NEW +2 existing. No other writes. Existing helpers unchanged.

## Gates

PREP artifact alone grants no SOURCE. Exact intended base/helper/input identity,
ROOT exact allowlist/contract grant and retained SAME3 approval required first.
After SOURCE: reuse SAME3 actual source review and changed-behavior recheck; local
fmt/compile-only Clippy ONLY after separate ROOT grant, no local product/parser/
worker/server/tests/bench/native/JVM/oracle runtime under96. Original unchanged
hosted Linux Native CI pinned fmt/lockedClippy/workspacetests supplies runtime.
Preserve first failure and stop for ROOT diagnosis, no fixture tuning/rerun-to-green.
After publication grant push only own branch/open+ATTACH PR/wait required CI and
integration clearance. Separate strictly read-only fresh exacthead/base/scope/
config/clean identity guard must return actual0+complete facts under exclusive
ROOT lease before separate ordinary expected-head merge. Retain lease through
actual tree/soleparent and independent OWNER+ROOT original exact-main all3 proof.
No admin/bypass/force-main/settings changes. Finished architect review required.

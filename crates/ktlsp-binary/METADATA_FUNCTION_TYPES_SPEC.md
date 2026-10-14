# T018 Function inline Type composition — frozen preparation contract v1

Preparation only, 2026-10-14. Owned checkout /Users/anil/.codex/worktrees/7151/ktlsp,
branch codex/t018-function-inline-types, clean HEAD 9b4cf49e817d0bb30cbd2f09b674f91afb1aa5fd,
tree f85d7afb021618081aeee27e7b9dd6190bbc0fcf, sole parent e84f13f10babcdcc431dd6f5e3e59843e0201eaa.
Root confirmed delivered remote identity; initial SSH DNS exit128 is a read/network
outcome only. No source/runtime/export/publication/CI grant is implied.

## Finite API and reach

New metadata::function_types module following existing type_arguments conventions:

```rust
pub use super::string_table::Limits;
pub use super::function_header::{Error, ErrorKind};
#[derive(Debug, PartialEq, Eq)]
pub struct SelectedType<'a> {
    pub field_number: u32,
    pub span: std::ops::Range<usize>,
    pub header: super::type_header::TypeHeader<'a>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct FunctionWithTypes<'a> {
    pub header: super::function_header::FunctionHeader<'a>,
    pub types: Vec<SelectedType<'a>>,
}
pub fn decode(bytes: &[u8], limits: Limits) -> Result<FunctionWithTypes<'_>, Error>;
```

Input is one unframed caller-selected supported mv=[2,3,0] Function body.
Caller selects metadata header/version/kind/container independently; bytes cannot
prove any of these. Preserve each field3/wire2 return_type and field5/wire2
receiver_type in ONE vector in global encoded order, including duplicate and
empty occurrences. field_number stays 3 or 5; span is EXACT child body-only
Function-relative range, not field/tag/length/d1/container range. header.raw and
every child header.raw are pointer-identical to the supplied input/slice, including
empty slices at their exact starts. No copied raw or field vector.

Each selected body uses existing TypeHeader raw/scalar decoding. Scalar presence,
last-recognized-wins, signed low32 int32, nonzero full-u64 nullable, wrong-wire and
raw preservation rules remain unchanged. Conflicting classifiers and simultaneous
inline/ID fields succeed as raw data. Unselected Function fields (including ID
fields7/8, nested parameters/tables/contracts/extensions) stay opaque. Within Type,
nested Argument/Type/annotation bodies stay opaque, including malformed bytes.
Selected duplicate singular messages are explicitly UNMERGED: this is occurrence
data, not protobuf singular-message merge or full initialization/compatibility.
No ID lookup, name resolution, recursive graph, substitution, nullability meaning,
type reconstruction, semantic/server/performance/platform promotion.

Public structs remain caller-constructible; raw/typed consistency is not guaranteed
for arbitrary caller literals. New consumer is integration tests only. Existing
public FunctionHeader decode/raw/scalar behavior and tests remain unchanged;
existing Class/Package name composition, TypeHeader and Type/Argument users do not
migrate. Reach: new module, new integration tests, literal fixtures/notices, finite
spec/review receipt, one later additive export. SOURCE_ALLOWLIST.json is prospective,
not permission. metadata/mod.rs is exclusively another owner's until root release.
Cargo/workspace/dependencies/lock, config/workflows, wire/string_table/type_header/
function_header helpers, parser/vendor/server, public status and other owners' paths
are READONLY. No new dependency, shared-helper redesign or convenience abstraction.

## Algorithm and aggregate accounting

1. Check input length against min(limits.bytes,MAX_BYTES), error Wire(Limit(bytes))@0.
2. Create ONE string_table::Budget::new(limits); node(1,0).
3. Run function_header::decode_with_budget(bytes,&mut budget.wire) to completion.
   Whole parent wire validation and required-name check precede ANY child decode.
4. Second charged parent scan using wire::field and work(1,field.span.start) for
   every complete field. For selected wire2 only, derive span=end-raw.len()..end;
   node(2,span.start); run type_header::decode_with_budget(raw,&mut budget.wire).
   Map only that child wire Error to Function Error, adding span.start exactly ONCE.
   Then budget.push shared vector at parent tag start. Root/header/rescan/node/push
   errors already use Function coordinates; do not rebase them.
5. Return parent header and ordered vector only after complete second pass.

Budget helpers remain unchanged and READONLY. Node charge order is depth -> nodes
-> work; wire fields charge records -> record work -> tag/value/bulk reads ->
dispatch work. push growth charges checked logical owned bytes -> work ->
try_reserve_exact -> push. Allocation refusal is Wire(Limit(allocation)) at parent
tag, statically inherited; no deterministic allocator failure/runtime claim.
Existing checked arithmetic and bounded spans suffice; no unchecked external span.
Failure immediately returns Error, no partial output, retry/refund/reset or early
success. MissingName@input.len occurs only after a clean first pass; a later
malformed parent overrides earlier malformed child, and missing name overrides
child faults. Budget errors in the parent also override child faults. Error kinds
wrap existing wire kinds except MissingName. Error strings contain kind/offset only.

Let L=input length, F=parent field count, K=selected occurrence count, Mi/Gi=child
body lengths/flat field counts, S=size_of::<SelectedType<'_>>() on actual target,
C(0)=0 and C(K>0)=next power of two. Successful exact minima:
bytes=L; records=2F+sum(Gi); nodes=1+K; depth=1 if K=0 else2;
owned_bytes=C(K)*S; work=1+2(L+2F)+sum(1+Mi+2Gi)+C(K)*S.
Both parent scans charge bytes read/skipped and fields; bytes cap applies ONCE to
input, not cumulatively. Each child reread and allocation-growth work is charged
under the SAME budget. Logical capacity growth is 1,1,2,4,... at lengths0,1,2,4,...;
no actual ABI size or allocator capacity is guessed. Only requested retained vector
capacity is bounded, excluding surplus/churn/stack/RSS/whole-pipeline memory.
Header/d1/payload/container/resolver operations outside this decode retain separate
budgets. Work is accounting, not CPU/walltime/performance evidence.

All caller limits tighten only: bytes/owned clamp16MiB, records/nodes clamp65535,
work clamps64Mi, depth clamps2, even usize::MAX. Inherited nonminimal tags/lengths
use low32, values use deterministic low64 slow-path; field0 precedes wire validity,
groups3/4 unsupported, wires6/7 invalid, negative low32 lengths fail, ten
continuation bytes fail. No universal buffer-dependent JVM fast-path compatibility.
Reuse the existing limitation comment in API docs rather than widening claims.

## Independent evidence and acceptance

EXPECTATIONS.json freezes literals, exact minima, all byte-prefix partitions,
parent/child failure offsets and one-less deficits before source exists. Expected
values come from held committed data and static existing helper rules, never a
product decoder/encoder/oracle output. Symbolic S avoids target-layout assumptions.
EXPECTATIONS.md adds resource sequencing, growth, clamping and deterministic hostile
families. data_self_check.py performs only literal slicing, length/hash/arithmetic
and held CP31 UTF8 transcription; it does not parse classfiles/protobuf or execute
product/tests/binaries/parser/worker/server/bench. No redundant replay/framework.

Genuine return witness reuses licensed committed Unit.class unchanged, not copied
or repacked. PROVENANCE.json records actual hashes, independent held CP31 bytes,
d1/Function/Type spans, prior committed evidence, pin references and notice hashes.
Genuine receiver coverage ABSENT in this finite witness; every receiver literal is
synthetic. Compiler provenance UNVERIFIED. Source/schema pin claims are inherited
from committed specs/receipts; no fresh source download, artifact hunt or research.

Own architect plan approved; separate correctness/functionality and adversarial
reviewers plus SAME architect must approve final bytes/contract after fixes. Retain
SAME3 for later source and actual original CI. No source admission until root gives
exact allowlist/base after owner239 export release. Later format/compile-only
Clippy/unchanged original hosted Linux tests/publication/integration/expected-head
merge each follow root's explicit grant; no local native runtime under issue96.
T008/T008a HOLD; B3/B4 GAP; fullB11 UNVERIFIED; issue96 OPEN/DarwinUNKNOWN;
#18/#68 stay open. This packet establishes neither delivery nor full parity.

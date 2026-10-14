# Class supertype source stage — unmerged, runtime unexecuted

Root source amendment3e787f529721ff4dc9b8912db689076513b4746f6f1de74bd5f1fbf5bc491c3a
admits exactly24 new paths plus own additive export at
6b965cfa8008df51deba5ff4a56be31e9b30d16b (tree
d6c1fa7ab838488f9dd8283d9cfb19c0cecb4025, sole parent
efbfb991014f926ab2031d489a53a12ef386ed7b). This explicitly supersedes only the
historical serialized source/export delivery hold below: FunctionTypes and this
API have no source dependency. Existing helpers remain unchanged/read-only.
No commit, issue/push/PR, hosted CI, final integration or merge admission follows.
Only local pinned format and compile-only locked all-target workspace Clippy are
admitted; native tests/runtime remain unexecuted under #96/Darwin UNKNOWN.

The reviewed preparation contract follows verbatim as historical acceptance text;
fixtures EXPECTATIONS JSON/MD and PROVENANCE are copied unchanged. Source-stage
README links alone are adjusted for the installed path. Review receipts track
actual current source separately. No semantic/provider/full-parity promotion.

# T018 Class inline supertype Type composition — preparation contract v1

Preparation only, 2026-10-14. Owned clean managed checkout
/Users/anil/.codex/worktrees/579f/ktlsp, codex/t018-class-supertype-types-contract.
Exact baseline28ff42d743b6290b9a1e587e36eced80cebd6438,
treef989c624b58fead0144ebe197967e6ff2291444d,
sole parent9b4cf49e817d0bb30cbd2f09b674f91afb1aa5fd.
Initial clean detached cached9b4 adopted ONLY assigned28ff, without main chase.
Root delivery-clear file/hash verified as recorded in PROVENANCE.json.
No source/export/helper/Cargo/issue/PR/CI/replay/publication/runtime grant.

## Minimum additive API and reach

New metadata::class_supertypes, matching existing type_arguments conventions:

```rust
pub use super::string_table::Limits;
pub use super::class_header::{Error, ErrorKind};
#[derive(Debug, PartialEq, Eq)]
pub struct SelectedSupertype<'a> {
    pub span: std::ops::Range<usize>,
    pub header: super::type_header::TypeHeader<'a>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ClassWithSupertypes<'a> {
    pub header: super::class_header::ClassHeader<'a>,
    pub supertypes: Vec<SelectedSupertype<'a>>,
}
pub fn decode(bytes: &[u8], limits: Limits) -> Result<ClassWithSupertypes<'_>, Error>;
```

Input is ONE unframed caller-selected Class body for k1/mv[2,3,0]. Caller
independently validates metadata header/version/kind and chooses the container;
this bytes-only API cannot prove those facts. Every Class field6/wire2 occurrence
is a selected supertype, in encoded order, including empty bodies and duplicates.
No redundant field discriminator: this collection selects only field6.
span is the exact body-only Class-relative range, excluding tag/length and d1.
header.raw and each child header.raw borrow the supplied input/exact slice with
pointer identity (including empty slices at the exact span.start). No raw copies,
separate flat fields, iterator, cache or owning self-borrowing aggregate.

Class raw/flags/fq_name preserve existing standalone behavior: optional flags
absence/default6 remain distinct, signed required fq_name index, independent
last-recognized-scalar wins. TypeHeader retains all ten existing optional scalars,
signed low32 integer conversion, full deterministic u64!=0 nullable conversion,
last-recognized-scalar wins and wrong-wire presence preservation. No scalar bit,
index/name/classifier/nullability interpretation. Empty Type is accepted.
Different supertype bodies are distinct occurrences; scalars never merge across
them. Raw retains unknowns, duplicate fields, extensions and wrong wires.

Class field2 supertype ID encodings (packed or unpacked) remain opaque RAW ONLY,
including malformed packed contents; no ID collection, lookup, inline/ID
reconciliation, type-table graph, name-to-symbol resolution or inheritance.
Other parent nested bodies remain opaque. Inside selected Type, nested
Argument/Type/annotation bodies remain opaque even if malformed. No recursive
initialization, Type/Argument graph, symbol/type reconstruction, substitution,
semantic provider or full Class/protobuf/Kotlin compatibility claim.

Public structs remain caller-constructible; arbitrary caller literals do not
have raw/typed consistency guarantees. Existing public wrappers, raw/scalars,
helpers, tests and fixtures remain unchanged. Existing class_header callers are
class_member_names and integration tests; TypeHeader callers are type_arguments
and integration tests. None migrate. New consumer: integration tests only.
SOURCE_ALLOWLIST.json freezes prospective module/tests/literals/notices/spec/review
paths and the one additive metadata/mod.rs export; it grants no writes. Export
serialization remains owner239, then FunctionTypes owner01a13b00-4a8a-7ef1-ab36-614a098a68fb,
then this child ONLY after their OWNER+ROOT delivery/release and an exact root
allowlist/base grant. All helpers, Cargo/workspace/dependencies/lock/config/workflows,
parser/vendor/server/root-status docs and other owners' paths remain READONLY.

## Algorithm, precedence and shared accounting

1. Reject bytes.len()>min(limits.bytes,MAX_BYTES) as Wire(Limit(bytes))@0.
2. Create ONE string_table::Budget::new(limits); budget.node(1,0).
3. Call class_header::decode_with_budget(bytes,&mut budget.wire) to completion.
   Complete parent wire validation and required fq_name precede any selected
   child node/decoder/storage. MissingFqName is at input.len after clean wire only.
4. Second charged parent scan: wire::field then work(1,field.span.start) for EVERY
   complete field. Select only (6,Value::Bytes(raw)). span=end-raw.len()..end;
   budget.node(2,span.start); type_header::decode_with_budget(raw,&mut budget.wire).
   Convert ONLY this child wire Error to existing Class ErrorKind::Wire and add
   span.start exactly ONCE to its offset. Parent/root/rescan/node/push errors
   already have Class-relative offsets; never rebase those.
5. After a successfully decoded child, budget.push ONE shared supertypes vector
   at the parent tag start. Return header/vector only after full second scan.

Reuse existing private shared-wire helpers READONLY. No helper extraction,
redesign, dependency, per-child public decoder/reset/refund/retry or partial output.
Wire fields charge records -> record-work -> tag/value/bulk reads -> dispatch-work.
Node charges depth -> nodes -> work. Growth charges checked logical owned bytes ->
work -> try_reserve_exact -> push, at selected parent tag. Allocation refusal maps
to Wire(Limit(allocation)); inherited static handling only, no deterministic
allocator-failure/runtime claim. Span bounds come from checked internal wire reads;
child.start+error.offset<=child.end<=input.len. No caller-supplied spans.

A later parent structural error wins over any earlier child error; missing fq_name
wins over a child error after a clean first scan. Parent resource errors likewise
precede children. During second scan, child node/decoder failure precedes storage
failure. Parent rescan dispatch failure precedes node failure. Error text contains
only existing kind/offset; no input contents. Class/Type parent errors retain their
existing wire distinction, with no new public ErrorKind.

Let L=input length, F=parent flat field count, K=selected occurrences, Mi/Gi=child
lengths/flat field counts; S=size_of::<SelectedSupertype<'_>>() on actual target,
C(0)=0, C(K>0)=next power of two. Successful exact minima:
bytes=L; records=2F+sum(Gi); nodes=1+K; depth=1 if K0 else2;
owned_bytes=C(K)*S;
work=1+2(L+2F)+sum(1+Mi+2Gi)+C(K)*S.

Bytes cap applies ONCE to the input, not summed rereads. Both parent scans,
selected child rereads/nodes and all vector growth share the SAME budget.
Logical additional capacities1,1,2,4,... at lengths0,1,2,4,...; no guessed ABI or
allocator surplus. Bounds cover requested retained vector capacity only, excluding
surplus/churn/stack/RSS/whole-pipeline memory. Accounting work is not CPU/time.
External classfile/header/d1/payload/table/resolver operations keep separate budgets.
All caller limits only tighten: bytes/owned clamp16MiB, records/nodes clamp65535,
work clamp64Mi, depth clamp2, even requested usize::MAX. No independent hard-work/
hard-node/storage reachability claim where another hard cap stops earlier.

Inherited wire rules remain: low32 tags/lengths, low64 varints, nonminimal values
terminating within10 bytes; negative signed low32 lengths fail; tenth continuation
fails; field0 before invalid wire; groups3/4 unsupported, wires6/7 invalid.
API docs retain `shortcut: raw64 inherits wire's slow-path rule; revisit for JVM
fast-path equivalence.` No universal buffer-dependent JVM compatibility claim.

## Frozen evidence, review and future gates

EXPECTATIONS.json/EXPECTATIONS.md freeze all literal tuples/spans, prefix partitions,
resource minima/one-less offsets/growth and hostile boundaries before source.
Derived only from actual held data and existing helper source, never future decoder,
encoder/compiler/oracle output. Auxiliary data_check_output.json records stdlib
fixed byte transcription/slicing/hash/arithmetic only; no product parser or test
framework. All product assertions remain UNEXECUTED.

Existing licensed Unit.class stays unchanged, referenced in place rather than
copied/repacked. Actual bytes, held independent CP31/d1 transcriptions, Class
33-byte slice and supertype30 01, provenance and notices were read/verified.
PROVENANCE.json freezes hashes. Compiler provenance UNVERIFIED; artifact distribution
version is not compiler provenance. Source/schema pins inherited from completed
receipts, not freshly reacquired. No new artifacts/downloads/research restart.

SAME architect initial plan approved, with SAME separate correctness and adversarial
final-preparation review required. Fix/recheck actionable findings, seal manifests;
retain all SAME3 for source and completed original hosted CI/delivery if admitted.
Future source/export requires exact root base/allowlist grant. No local Rust native
runtime/build/tests/binaries/parser/worker/server/bench/JVM/compiler/oracle/Windows
execution. Future unchanged pinned fmt, locked all-target workspace Clippy and
locked workspace tests follow root's hosted grants; push/open/ATTACH PR, one stable
final cut, separate successful read-only merge guard then separate ordinary
protected expected-head merge under exclusive exact-main/base lease, actual squash
tree/sole-parent verification and independent OWNER+ROOT original exact-main CI
proof/preservation/release. Never bypass protection/settings or rerun-to-green.
Queue235 ->237 ->239 ->243 ->241 unchanged; this packet requests no lease.

T008/T008a HOLD; broader C13/B3/B4 GAP; fullB11 UNVERIFIED; #96/Darwin UNKNOWN;
#18/#68 and semantic/provider/full-parity/performance/platform gaps stay open.
This finite preparation is neither implementation, delivery nor full parity.

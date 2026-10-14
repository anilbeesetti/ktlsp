# Selected TypeParameter — source admitted, runtime validation pending

Child #173 of #18/#68. Root admitted the frozen twelve exclusive new paths,
visibility-only internal push/integers reuse, publication and original Linux CI.
Root released the additive module export after Property source committed.
No local native runtime.
Below is the preserved preimplementation contract, not a delivered parity claim.
The unchanged literal freeze is tests/fixtures/metadata_type_parameter/EXPECTATIONS.md.

# T018 selected TypeParameter record — contract only, v1

Frozen preparation 2026-10-14: base 941c72efd9896b0847ab342ec0dd22c10a000f3b,
tree c9a8c8ae5ee4984b6b21849007bafbeebea33975, assessed product 50bef1c6.
Owned managed worktree /Users/anil/.codex/worktrees/t018-type-parameter/ktlsp,
branch codex/t018-type-parameter. No source admission/publication yet.

Decode one unframed TypeParameter as data, with required signed id/name,
optional Boolean/variance presence, repeated signed bound IDs, borrowed opaque
bound-message bodies/spans. No recursive Type/Annotation/extension initialization,
name/type-table lookup, index validation, substitution, linkage, generic semantics,
container traversal, server consumer, Java/tool/oracle execution, or parity claim.
The caller separately selects supported mv=[2,3,0] data and supplies the body;
a bytes-only API cannot validate the metadata header/version/container kind.

## Proposed API

New metadata::type_parameter module:

```rust
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
    pub records: usize,
    pub work: usize,
    pub owned_bytes: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variance { In, Out, Inv }
#[derive(Debug, PartialEq, Eq)]
pub struct UpperBound<'a> {
    pub raw: &'a [u8],
    pub span: std::ops::Range<usize>, // body only, relative to input
}
#[derive(Debug, PartialEq, Eq)]
pub struct TypeParameter<'a> {
    pub raw: &'a [u8],
    pub id: i32,
    pub name: i32,
    pub reified: Option<bool>,
    pub variance: Option<Variance>,
    pub upper_bound: Vec<UpperBound<'a>>,
    pub upper_bound_id: Vec<i32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind { Wire(super::wire::ErrorKind), MissingId, MissingName }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error { pub offset: usize, pub kind: ErrorKind }
pub fn decode(bytes: &[u8], limits: Limits) -> Result<TypeParameter<'_>, Error>;
```

Limits Default uses existing MAX_BYTES=16MiB, MAX_RECORDS=65,535,
MAX_VALIDATION_UNITS=64Mi; owned_bytes defaults16MiB. All ceilings clamp, even
usize::MAX. Error Display/Error and wire conversion mirror function_header.
No effective-default getters/constructors/unknown vector: full borrowed raw retains
all exact fields, duplicates, wrong-wire/unknown enums and order; callers can use a
separate wire operation if they need a field inventory. Optional absent defaults
are false and Inv, without replacing None. Required zero/negative values valid as
raw data. Public structs are caller-constructible; no validated-object guarantee.

## Source contract

Existing offline source pins verified by SHA256 before preparation:
Kotlin v2.3.20 d57eb4a26ed01ab03cf195da686c8047032ff16f;
metadata.proto a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2
(TypeParameter177-197; blob18cfd13e9e512c491f2f62ec241f49c81d6e5e81),
ProtoBuf.java da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716
(TypeParameter7569, parser7617-7678, defaults7946, initialization7955;
blobfc424d5e7aac8661deb862c8e791d4fcf779c194),
patched CodedInputStream.java 7fbb3aeebd3c3d5414d7171bc3ba2785de50b40863a4f381c4b43986e15784fe
(readBool382 uses raw64 !=0). Existing protobuf2.6.1 baseline/notices unchanged.
Sources are read-only reference data, never shipped or executed.

Recognize exact (number,wire): (1,0) id, (2,0) name, (3,0) reified,
(4,0) variance, (5,2) upper_bound, (6,0|2) upper_bound_id.
ID/name/bound IDs/variance use low32 signed interpretation, never zigzag.
Reified uses full existing deterministic u64 !=0, not low32 or strict0/1.
Variance0=In,1=Out,2=Inv; other signed values remain raw unknown and do not
set/clear presence or replace earlier known enum. Singular recognized values last
win. Wrong-wire fields never set/clear/replace selected fields. Repeated bound IDs
append in encounter order across unpacked/packed segments; empty packed valid.
Each selected bound body is retained even empty; body span derived only from an
already validated wire Bytes field. Mixed bound bodies/IDs both retained, no
exclusivity check. Selected bound contents wholly opaque, including malformed
protobuf, invalid UTF8 and incomplete required nested messages. Annotation100 and
extensions/all other fields remain raw opaque, even if invalid nested messages.

Unchanged wire compatibility: nonminimal varints, low32 tags/lengths and low64
slow-path values; ten continuation bytes malformed; low32 negative lengths fail;
field0 precedes wire validation, groups3/4 unsupported, wires6/7 invalid. No
canonical-protobuf or universal buffer-dependent JVM fast-path compatibility claim.
This slice introduces no strings/UTF8 decoding and does not fix name resolver's
selected malformed-UTF8 UnsupportedUtf8 GAP.

## Budget and precedence, prospective shared-helper proposal

Reach existing string_table::Budget::new with matching records/work/owned limits
and existing default unused nodes/depth; no node()/nested scan, so no node/depth
charges or public depth/node fields. Reuse budget.wire, Budget::push and
Budget::integers by proposing ONLY pub(super) visibility on the latter two methods.
No helper behavior, existing API, integer/Operation enum or error change. This
visibility change and mod.rs export need ROOT-SERIALIZED explicit admission.
No duplicated allocation/packed reader, no new dependency. If root declines this
shared scope, revise and re-review contract before source, never silently copy it.

Byte limit first at0, then one shared budget for ENTIRE message. Each field calls
wire::field then dispatch work1 at tag_start before selected processing. Bounds
append via existing precharged push at tag_start, never scan/copy body. Bound IDs
use integers: unpacked interpretation1 at tag_start; each packed entry shares same
budget, pays record1/work1 at entry_start, varint byte reread, interpretation1, then
push at entry_start. No per-field reset/refund or length-sized reservation.

All vectors share ONE owned allowance. Existing deterministic logical capacity
increments: length0 adds1 slot; each positive power-of-two length adds that length
in slots. Charge added_slots*sizeof(T) to owned_bytes, then same count to work,
then try_reserve_exact, then append. Allocator surplus does not reduce charges;
reserve failure is Limit("allocation"). No runtime OOM forcing test required.
Requested retained capacity excludes allocator surplus/churn/transient buffers,
stack structs and RSS. Bounds are borrowed, no recursion or depth-dependent work.

For a successful scan let N=input bytes, F=fields, P=packed ID entries,
L=packed ID byte count, U=unpacked ID count, C=sum logical reservation bytes.
Records=F+P. Work=N+2F+L+2P+U+C. Empty packed fields still pay field/dispatch.
Constants/required EOF checks/raw/span/scalar bookkeeping unmetered. Work failure
before allocation/append is mandatory. The byte/work/record/capacity bounds are
separate from header/d1/table/name resolution; no whole-pipeline budget/RSS claim.

Wire error offsets/precedence unchanged, relative to input. Each field error
precedes dispatch, which precedes interpretation/append. Records before record
work before reading; owned_bytes before reservation work before allocator failure.
For packed errors end is packed payload end, never following field/end-of-input.
Only clean full-message EOF checks required fields: MissingId first, then
MissingName, at input.len(). Malformed/limit tail after valid selected fields
always fails, even if another required field is missing. No partial result.

## Exclusive paths and shared admission

Exclusive prospective NEW paths only:
- crates/ktlsp-binary/src/metadata/type_parameter.rs
- crates/ktlsp-binary/tests/metadata_type_parameter.rs
- crates/ktlsp-binary/tests/fixtures/metadata_type_parameter/minimal.hex
- crates/ktlsp-binary/tests/fixtures/metadata_type_parameter/mixed.hex
- crates/ktlsp-binary/tests/fixtures/metadata_type_parameter/scalars.hex
- crates/ktlsp-binary/tests/fixtures/metadata_type_parameter/EXPECTATIONS.md
- crates/ktlsp-binary/tests/fixtures/metadata_type_parameter/README.md
- crates/ktlsp-binary/tests/fixtures/metadata_type_parameter/LICENSE.txt
- crates/ktlsp-binary/tests/fixtures/metadata_type_parameter/NOTICE.txt
- crates/ktlsp-binary/tests/fixtures/metadata_type_parameter/PROTOBUF_LICENSE.txt
- crates/ktlsp-binary/METADATA_TYPE_PARAMETER_SPEC.md
- crates/ktlsp-binary/METADATA_TYPE_PARAMETER_REVIEW.md

ROOT-SERIALIZED shared proposal: metadata/mod.rs adds pub mod type_parameter;
metadata/string_table.rs adds pub(super) on push<T> and integers ONLY.
No Operation change exists/needed; it belongs to string-table transformations.
Public API additions only, no changed existing enum variants/struct fields or
caller migrations/source-compatibility break. New tests are sole callers; existing
wire/table tests must remain unchanged. No Cargo/lock/workflow/dependency/lib.rs/
parser/vendor/server/status/HANDOFF/README edits. Separate Property owner untouched.
Notice copies byte-identical from metadata_wire fixtures.

## Review, validation and delivery gates

FIXTURES.md freezes literals without calling any implementation or encoder.
Architecture plan reviewed; SAME two independent contract reviewers and architect
finished-contract review required before root reads/admit source. Preserve initial
findings/corrections. Auxiliary preparation only; repo remains clean at base.
No publication/source/test/spec/product edits, native execution, push/PR/CI/merge
until root admission. Under #96 no local product/parser/worker/server/tests/bench
runtime; local fmt and compile-only Clippy need later root compile-slot grant.
Original unchanged Linux Native Rust CI is future runtime executor. SAME3 review
changed source and completed original CI, root exclusive expected-base/head merge
lease, ordinary expected-head squash; exact reviewed-tree/sole-parent/main original
attempt1 checks and owner+root delivery verification. No retries of unchanged
failures, bypass/force/settings changes or cache/archive cleanup.
T008/T008a HOLD; broaderC13/B3/B4 GAP; B11 UNVERIFIED; #7/#83/#96 OPEN,
Darwin UNKNOWN. Native data validation is not full metadata/semantic/LSP parity.

# ValueParameter inline Type-header composition

Source admitted by root on exact delivered PR229 base
9b4cf49e817d0bb30cbd2f09b674f91afb1aa5fd, tree f85d7afb021618081aeee27e7b9dd6190bbc0fcf,
sole parent e84f13f10babcdcc431dd6f5e3e59843e0201eaa. Released TypeHeader helper SHA256
0ce3555bf60382a597f3dae6d7222144b6d6a4c6c0c2ae4a6340f181641af422 matches the reviewed provisional bytes.

The following historical contract is preserved verbatim from the pre-source
freeze SHA256 dbd4a989a8296b500750d5f90fe676ad3843ff9ab4f8c97e386df2044ec3af20.
Its preparation-only and future-grant wording describes that earlier stage;
root subsequently granted exactly its17 paths, local pinned fmt/compile-only
Clippy, publication and original unchanged hosted Linux CI. Runtime stays hosted
only under #96. No merge lease follows source admission. REVIEW records actual
source/check/review/delivery status; it must not be inferred from this contract.

---

# T018 ValueParameter inline Type-header composition — preparation v2

Temporary contract and data ONLY. No product-source write, native build/test,
parser/server/worker/bench/compiler/oracle/JVM execution, publication or CI is
admitted. Worktree identity is recorded in IDENTITY.json and remains clean W2.
Public assessment 048c344/572 is frozen. This child neither completes T018 nor
adds semantic resolution or a server capability. #18/#68 remain open.

## Proposed smallest additive API (requires root's next exact grant)

New `metadata::value_parameter_types` module:

```rust
pub use super::string_table::Limits;
pub use super::value_parameter::{Error, ErrorKind};
#[derive(Debug, PartialEq, Eq)]
pub struct SelectedType<'a> {
    /// Exactly 3 (type) or 4 (vararg_element_type).
    pub field_number: u32,
    /// Body-only offsets relative to the supplied ValueParameter bytes.
    pub span: std::ops::Range<usize>,
    pub header: super::type_header::TypeHeader<'a>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ValueParameterWithTypes<'a> {
    pub parameter: super::value_parameter::ValueParameter<'a>,
    pub types: Vec<SelectedType<'a>>,
}
pub fn decode(bytes: &[u8], limits: Limits)
    -> Result<ValueParameterWithTypes<'_>, Error>;
```

Input is ONE caller-selected unframed ValueParameter for supported mv=[2,3,0].
Kind/version/container selection remains caller responsibility. Parent.raw borrows
the entire original input; each child header.raw borrows precisely bytes[span].
No byte copies, duplicate raw fields, raw field vector, enum or generic framework.
Public structs remain caller-constructible; no raw/typed equivalence guarantee.

Every wire2 field3 and wire2 field4 occurrence is decoded independently in wire
order, including empty and duplicate bodies. field_number preserves cross-field
identity; spans are parent-relative, never d1-relative. Type scalar optional
presence, signed low32 values, bool handling, last-recognized scalar wins,
wrong-wire and unknown handling follow existing TypeHeader exactly. Parent flags,
required name, both uninterpreted IDs, scalar duplicates and raw retain existing
ValueParameter behavior. No ID lookup/inline-vs-ID precedence, string/symbol binding,
flag/default/nullability application, full initialization or type reconstruction.

Pinned protobuf singular Types normally MERGE. This API returns OCCURRENCES only,
and promises neither merged nor last-wins Types. Name this explicitly in docs and
one `shortcut:` comment: occurrence data is unmerged; upgrade when merged Type
interpretation is implemented. Type arguments, flexible/outer/abbreviated Types,
annotations/defaults/extensions and all deeper bodies remain opaque, even with
malformed inner bytes. There is exactly one parent->Type level, no recursion.
Genuine Pair proves field3 only; field4 positives are authored data.

## Dependency and exact helper extraction

PR229 TypeArguments must be OWNER+ROOT-delivered, and TypeHeader helper/export
ownership released, BEFORE any source/helper/export/publication/CI admission.
Inspected provisional source is exactly bdd8a02f2a3591fed4bf3be28f58b4d5740b2b64,
tree b9475d5b8d58e8cc62d589bd28afafd50b94cd12, sole parent
1c41e58daf531d96144686638a84bcac157ea0e3. Inspection implies no merge/delivery.
Its released `type_header::decode_with_budget(bytes,&mut wire::Budget)` is reused
without redesign. `string_table::Budget` is the shared budget; its `Operation`
enum is name transformation data and is NOT an operation-budget abstraction.

Only existing ValueParameter seam proposed: keep public decode signature, byte
cap/error and creation of its own wire::Budget, then delegate to new private
`pub(super) fn decode_with_budget<'a>(bytes: &'a [u8], budget: &mut wire::Budget)
-> Result<ValueParameter<'a>, Error>`.
Move the current body beginning `let mut pos=0` unchanged into it, removing only
extra &mut on wire::field's budget argument. All scan/dispatch/casts/required-name/
raw/error offsets and charges remain unchanged; no root/node/storage charges in
the old public decoder. Old empty input with default or all-zero limits still
MissingName@0. All its existing
tests remain byte-identical. No other helper extraction or budget changes.

## Algorithm, aggregate boundaries and precedence

1. Reject len>min(limits.bytes,16MiB) as Wire(Limit(bytes))@0.
2. Create ONE string_table::Budget from limits; node(1,0), charging depth then
   nodes then work in the existing order.
3. Decode entire parent with value_parameter::decode_with_budget(bytes,&mut
   budget.wire). All parent framing and required-name validation precedes ANY
   child scan/node/storage. Parent failures return immediately without partial data.
4. Rescan the parent with wire::field under SAME budget.wire. Each complete field
   costs dispatch work1 at its tag start, including unselected fields.
5. For each wire2 field3/4: body span=field.end-raw.len()..field.end; node(2,body
   start); released TypeHeader helper on raw under SAME budget.wire; rebase its
   wire error by body start exactly ONCE and convert via existing Error::from;
   push one SelectedType with budget.push(...,field tag start).
6. Return parent and ordered types only after complete second pass.

Parent and second-pass wire errors already use parent offsets; root/child node
and retained push errors already use parent offsets. NEVER rebase those. Child
helper errors alone are child-relative. Checked budget subtraction/multiplication/
reservation arithmetic comes from unchanged helpers. Slicing and rebase addition
are bounded: raw.len<=field.end<=len<=16MiB, body_start+child_error<=field.end;
wire/helper error offsets never exceed their input. No unchecked external spans.
No fresh public TypeHeader/ValueParameter decoder budget per occurrence.

Hard/default limits reused unchanged: bytes16MiB, records65535, nodes65535,
depth2, work64Mi units, owned_bytes16MiB. usize::MAX clamps; caller can tighten.
Root depth1 and each selected child depth2 consume nodes/work even for empty Type.
No-child input still pays root and BOTH scans; old API pays no extra charges.
No child bytes cap reset; the entire parent input cap covers borrowed children.

For successful parent length N, F parent fields, K selected child occurrences,
M total child body bytes, C total child fields, S=size_of::<SelectedType>(),
Q=0 when K=0 else next_power_of_two(K):
records=2F+C; nodes=1+K; depth=1 if K=0 else2;
owned=Q*S; work=1+2(N+2F)+K+(M+2C)+Q*S.
Use symbolic S, never a presumed ABI value. One vector means storage is shared
across field3/4; logical growth occurs at lengths0/1/2/4, charges1/1/2/4 slots.
Requested retained capacity excludes allocator surplus, churn, input/header/d1
ownership, stack and RSS. Allocation failure maps to Wire(Limit(allocation)) at
outer tag after owned/work debit; no forced OOM/allocator test is planned.

## Exact prospective allowlist and reach

NEW ONLY (not written to repository during preparation):
- crates/ktlsp-binary/src/metadata/value_parameter_types.rs
- crates/ktlsp-binary/tests/metadata_value_parameter_types.rs
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter_types/README.md
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter_types/EXPECTATIONS.md
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter_types/PROVENANCE.json
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter_types/minimal.hex
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter_types/selected.hex
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter_types/duplicates.hex
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter_types/opaque.hex
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter_types/wrong_wire.hex
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter_types/LICENSE.txt
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter_types/NOTICE.txt
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter_types/PROTOBUF_LICENSE.txt
- crates/ktlsp-binary/METADATA_VALUE_PARAMETER_TYPES_SPEC.md
- crates/ktlsp-binary/METADATA_VALUE_PARAMETER_TYPES_REVIEW.md
EXISTING: value_parameter.rs private extraction only as described above.
SERIALIZED ROOT SLOT: metadata/mod.rs additive `pub mod value_parameter_types;`.

Existing public callers: tests/metadata_value_parameter.rs and the raw-value/
genuine-Pair cases in tests/metadata_constructor.rs; no production consumer.
New tests call new API and reuse parse_class, annotations::decode, metadata::decode,
wire::decode and constructor::decode for genuine composition in future hosted
runtime only. Those stages retain separate budgets; only selected ValueParameter
composition shares its aggregate budget. Reuse genuine existing Pair.class.bin and
PAIR_EXTRACTION.json; no genuine class duplicate or fixture rewrite. New notices
are byte-identical existing metadata_wire/constructor notices. Existing TypeHeader,
TypeArgument, TypeArguments, Constructor, raw ValueParameter tests, wire/Budget,
binary lib root, Cargo/lock/workflows/config/export elsewhere remain unchanged.
Owners231(type_parameters/provenance),235(code.rs/exact8/binarylib) are excluded.
No root README/HANDOFF/PLAN/TICKETS/status or broader scope edits.

## Later execution and delivery (NOT granted now)

SAME3 retained architect, correctness/functionality and adversarial reviewers
review this packet, then actual source and completed ORIGINAL hosted Linux CI.
Root supplies exact source base/paths/released dependency grant first. Local fmt
and compile-only Clippy need later admission. Runtime ONLY original unchanged
hosted Linux Native Rust CI under #96; preserve first failures, no expectation
tuning from output and no unchanged rerun-to-green. Required pinned commands:
cargo fmt --all -- --check; cargo clippy --locked --workspace --all-targets -- -D
warnings; cargo test --locked --workspace. No local runtime/platform-gate change.
Own branch push and attached PR, paginated candidate Actions/suites/checks with
zero-check startup failures included, sole expected-head/base root lease, separate
repo-read-only enved freshguard then separate ordinary expected-head squash merge,
OWNER+ROOT exact-main original proof before release. No admin/force/settings/main
push. No passing CI/PR/merge or released helper is claimed by this preparation.

Native core/build/default tests stay Rust-only. T004 data-only; no JVM/JDK/compiler/
JNI/C/C++ parser or bundled Java. T008/T008a HOLD; broader C13/B3/B4 GAP; full B11
UNVERIFIED; #7/#83/#96 OPEN; Darwin UNKNOWN. No parity/performance/semantic claim.

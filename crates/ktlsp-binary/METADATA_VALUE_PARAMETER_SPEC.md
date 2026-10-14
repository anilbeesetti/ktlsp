# Selected ValueParameter raw record contract

Root admitted this exact finite API/path scope after all three contract reviews.
Source exists; original Linux CI and final delivery gates are recorded in REVIEW.
Owned worktree /Users/anil/.codex/worktrees/t018-value-parameter/ktlsp,
branch codex/t018-value-parameter, frozen HEAD
cff36d17cd5cd1386059ecc91572013fdb417b48 (tree
4e432a34ecd8c0eb27f563ec6f0dc483202e2472; sole parent941c72e).
Parents #68/#18 stay open. This delivers no binding, type/default application,
container traversal, name resolution, modifier interpretation or semantic provider.

## Pinned schema and API

Existing source data Kotlin v2.3.20 commit d57eb4a26ed01ab03cf195da686c8047032ff16f:
metadata.proto SHA256 a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2;
ProtoBuf.java SHA256 da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716.
Reference copies were temporary review inputs only, never shipped/executed. Existing wire/protobuf
pins and slow-path low-bit/nonminimal-varint caveats remain unchanged; no general
protobuf conformance or JVM buffer-dependent fast-path equivalence claim.

New metadata::value_parameter public module:

```rust
pub use super::wire::Limits;
#[derive(Debug, PartialEq, Eq)]
pub struct ValueParameter<'a> {
    pub raw: &'a [u8],
    pub flags: Option<i32>,
    pub name: i32,
    pub type_id: Option<i32>,
    pub vararg_element_type_id: Option<i32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind { Wire(super::wire::ErrorKind), MissingName }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error { pub offset: usize, pub kind: ErrorKind }
pub fn decode(bytes: &[u8], limits: Limits) -> Result<ValueParameter<'_>, Error>;
```

Error Display/std::error::Error and From<wire::Error> mirror function_header.
Input is ONE UNFRAMED ValueParameter body, selected independently from supported
mv=[2,3,0] Class/Package metadata. The bytes-only API cannot verify kind/version or
select a parameter. Offsets are relative to supplied bytes. Caller-constructible
public structs imply no validated raw/typed equivalence guarantee.

Only wire0 field1(flags, tag8), field2(name,tag16), field5(type_id,tag40), and
field6(vararg_element_type_id,tag48) are selected. Cast low32 bits signed i32,
never zigzag; retain negatives and unknown flag bits without lookup/validation.
Each recognized scalar independently last-wins, as generated Java23924..23969.
Wrong wire never sets/clears/overwrites selected scalar presence. flags None has
schema default0 but is never replaced with Some0; explicit0 stays Some0. Both IDs
remain optional presence/raw signed data (schema absent default0 is not applied).
Name requires a recognized wire0 occurrence; name0 and negative name succeed.
No requirement to supply type or ID, nor precedence if both occur.

Fields3(type),4(vararg_element_type),7(repeated annotation),8(annotation parameter
default value), extensions100..199 and every unknown/wrong-wire field remain
opaque solely in complete pointer-identical raw bytes. No fields/occurrence vectors,
accessors, partial nested views, recursion or allocation. All occurrence order,
duplicates, framing and bytes are retained. Generated Java23934..23990 MERGES
singular message3/4/8 occurrences and APPENDS annotation7; this API performs NEITHER
merge nor last-wins projection for nested messages. Future nested decoding must
honor merge/append rules. Malformed inner protobuf/required-field absence is accepted
if outer framing is valid; success is selected data only, not ValueParameter
isInitialized/full metadata validity. A one-line shortcut comment will mark opaque
nested validation as deferred until nested metadata decoding is added.

## Aggregate operation budget and error offsets

Reuse wire::Limits and one wire::Budget for the entire scan, no helper edits.
First bytes.len()>min(limits.bytes,MAX_BYTES) => Wire(Limit("bytes")) at0.
Then scan every field with wire::field; charge work1 at tag start AFTER complete
field read BEFORE dispatch. Check MissingName only at clean EOF, offset bytes.len().
No early return when selected scalars are present, no retry/reset/refund/partial output.
Every field attempt costs record1 then work1 at tag start; each read/skipped byte
costs work1. Every complete field costs dispatch1. Successful N bytes/F fields costs
N+2F work and F records. No root/node/depth charges. Owned retained capacity is
identically ZERO; no vector can allocate before admission because no allocation
exists. Do not add an unused owned_bytes setting. Other metadata operations have
separate budgets; no aggregate whole-pipeline/CPU/RSS/stack claim.

Defaults/hard clamps:16MiB bytes,65,535 records,64Mi work. usize::MAX tightens only
through those clamps. Maximum successful flat scan costs<=16MiB+131070 work, so
hard-work maximum is unreachable independently; test small exact work boundaries.
Empty input with all limits0 => MissingName0. Wire failures retain original kinds
and offsets. InvalidFieldNumber wins over bad wire; groups3/4 unsupported;6/7 invalid;
length negative => length-varint start; malformed varint => tenth continuation;
truncation => supplied end after attempted read charge. Failed bulk read charge is
remaining bytes+1 at bulk-read start; dispatch-budget failure offset is tag start.
All wire/limit failures precede missing-name, even after/before name is present.

## Exact admitted reach

NEW ONLY:
- crates/ktlsp-binary/src/metadata/value_parameter.rs
- crates/ktlsp-binary/tests/metadata_value_parameter.rs
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter/EXPECTATIONS.md
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter/README.md
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter/minimal.hex
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter/explicit.hex
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter/duplicates.hex
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter/opaque.hex
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter/LICENSE.txt
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter/NOTICE.txt
- crates/ktlsp-binary/tests/fixtures/metadata_value_parameter/PROTOBUF_LICENSE.txt
- crates/ktlsp-binary/METADATA_VALUE_PARAMETER_SPEC.md
- crates/ktlsp-binary/METADATA_VALUE_PARAMETER_REVIEW.md
Shared serialized slot metadata/mod.rs additive `pub mod value_parameter;` ONLY.
Existing wire::field/Budget reused unchanged. Callers new tests only. No production
consumer/caller migration. Existing fixture notices copied unchanged from metadata_wire.
No Cargo/workspace/lock/dependency/workflow/shared-helper/parser/server/root-status edits.

## Validation/delivery gate

EXPECTATIONS.md and hex fixtures freeze schema/arithmetic expectations before source.
Architect plan approved raw-only allocation-free API; independent correctness and
adversarial contract review required before root admission. Same3 review changed
source/final original Linux CI. No local native product/parser/worker/test/bench/
JVM/oracle/Gradle/Maven runtime under #96. Root may admit fmt/compile-only Clippy.
Pinned cargo fmt --all -- --check; cargo clippy --locked --workspace --all-targets
-- -D warnings; cargo test --locked --workspace, with runtime ONLY original unchanged
Linux Native Rust CI. Preserve first failures, no unchanged rerun-to-green. Root
supplies stable integration base and exclusive expected-head/base merge lease.
Only owned branch push, issue structured/--body-file, PR attached, passing reviewed
head ordinary expected-SHA squash. Independently verify tree/sole parent and original
attempt1 exact-main all3/log counts. No settings/admin/force/bypass/cache cleanup.
#7/#83/#96 OPEN; Darwin UNKNOWN; T008/T008a HOLD; B3/B4 GAP;B11 UNVERIFIED remain.

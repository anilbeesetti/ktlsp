# Selected Type.Argument — source admitted, runtime validation pending

Root admitted the exact fifteen new paths, additive module export, local pinned
format/compile-only Clippy, publication and original unchanged Linux CI at base
7980480e015150b75b982001c871ad1a5b54f35a after TypeHeader198 delivery. Source/export
commit5ee44469958a8453d98b49365a413e31e9ebc818 preserves every incoming export.
Child https://github.com/anilbeesetti/ktlsp/issues/209 of #18/#68.
No local native runtime; genuine Pair composition is now authored, still unexecuted.
Preserved preimplementation contract below; its historical stage statements do
not override this later grant. Frozen EXPECTATIONS.md remains byte-identical.

# T018 selected Type.Argument raw data — contract only v1

2026-10-14. Assigned clean base e5bd9e988db9f831982413e04e4b32f9e284b394.
Managed /Users/anil/.codex/worktrees/t018-type-argument/ktlsp, branch
codex/t018-type-argument. Product source, publication, native/compiler execution,
CI and merge have NO admission. This temp packet is a proposed finite child of
#18/#68. Preserve public assessed cutoffs; no feature-status or parser-gate edit.

## Smallest API and reach

Decode ONE caller-selected unframed Type.Argument. Caller separately selects
supported metadata mv=[2,3,0] and body; bytes-only API cannot enforce version,
kind or container. No recursive parsing, full protobuf initialization, ID/name
resolution, type reconstruction, alias expansion, projection application,
nullability, generics, linkage, server consumer or semantic parity.

```rust
pub use super::wire::{Error, ErrorKind};
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize, pub records: usize, pub work: usize, pub owned_bytes: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Projection { In, Out, Inv, Star }
#[derive(Debug, PartialEq, Eq)]
pub struct InlineType<'a> {
    pub raw: &'a [u8],
    pub span: std::ops::Range<usize>, // body-only, relative to Argument input
}
#[derive(Debug, PartialEq, Eq)]
pub struct TypeArgument<'a> {
    pub raw: &'a [u8],
    pub projection: Option<Projection>,
    pub inline_types: Vec<InlineType<'a>>,
    pub type_id: Option<i32>,
}
pub fn decode(bytes: &[u8], limits: Limits) -> Result<TypeArgument<'_>, Error>;
```

Limits Default: existing MAX_BYTES16MiB, MAX_RECORDS65535,
MAX_VALIDATION_UNITS64Mi work, owned_bytes16MiB; ceilings clamp even usize::MAX.
Reexport wire errors/Display/Error unchanged; offsets here are Argument-relative.
No required fields/new error variants/getters/constructor/unknown vector. Absent
projection=None has schema effective default Inv; explicit Inv is Some(Inv).
Absent type_id=None has schema default0; explicit0 is Some(0). Public structs are
caller-constructible without raw/typed validation guarantee. Existing APIs and
callers unchanged; sole callers added by focused tests, no migration.

## Pinned data behavior

Read-only existing Kotlin v2.3.20 d57eb4a26ed01ab03cf195da686c8047032ff16f:
metadata.proto SHA256 a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2,
blob18cfd13e9e512c491f2f62ec241f49c81d6e5e81, Type.Argument122..137;
ProtoBuf.java SHA256 da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716,
blobfc424d5e7aac8661deb862c8e791d4fcf779c194, Argument5130,
parser5165..5207, defaults5368, initialization5374. Existing wire source pins
and Kotlin/protobuf notices remain authoritative; Java source read as data only.

Recognize EXACT number/wire (1,0) projection, (2,2) inline Type, (3,0) type_id.
Projection takes low32 signed int32:0 In,1 Out,2 Inv,3 Star. Every other enum
value remains raw and neither sets/clears presence nor erases a prior recognized
projection. Known scalars last-win. type_id takes low32 signed int32, never
zigzag/index validation; negative/extreme IDs stay data. Wrong-wire fields stay
raw only and never replace selected values. Full raw borrows pointer-identical
input, preserving all exact bytes, unknown/reserved fields, duplicates and order.

Retain EVERY recognized inline Type occurrence in encounter order, including
empty and malformed/incomplete bodies; span=field.end-body.len()..field.end
derived only from validated wire field. No nested scan or string decoding.
Upstream singular-message duplicates MERGE. This vector preserves the inputs to
that merge, but does not perform it: last-body-wins is NOT equivalent. A body
with class_name followed by one with nullable and then empty is a distinguishing
fixture. Mixed inline bodies/type_id are retained without exclusivity or pairing.
Do not enforce the schema comment about STAR omitting type/nonSTAR requiring it:
it is not a protobuf required-field rule. Empty/nonSTAR-without-type/STAR-with-
type succeed structurally. Nested initialization remains explicitly unchecked.

Unchanged wire rules: nonminimal varints, low32 tags/lengths, deterministic low64
slow-path values, ten continuation bytes malformed; signed-negative low32 length
rejects. Field0 precedes bad wire; groups3/4 unsupported, wires6/7 invalid.
No canonical/universal JVM fast-path equivalence claim. Existing resolver invalid
UTF8 GAP stays unchanged; this child performs no text decoding.

## One aggregate budget and error precedence

Admission len>min(bytes,MAX_BYTES) -> Limit(bytes)@0 FIRST. Use unchanged
string_table::Budget::new matching bytes/records/work/owned and unused default
nodes/depth; reuse budget.wire and budget.push only. No helper/depth/node change.
One budget spans ENTIRE scan: wire::field then work1 at tag_start BEFORE schema
dispatch; all unknown/wrong-wire/late fields scanned, no early return/reset/refund
or partial result. Every opaque body byte is charged once by wire, never reread.

Let S=size_of::<InlineType>() and k retained bodies; C=0 if k0 otherwise next
power of2(k). Successful N-byte,F-field scan: records=F, work=N+2F+C*S,
requested owned=C*S. Stack/raw/scalar/span bookkeeping unmetered. No packed ID
support: field3/wire2 is wrong-wire opaque. At append len0 or power-of2, slots=
max(len,1); checked len+slots then slots*S, aggregate owned subtraction, work
debit slots*S, try_reserve_exact(slots), append, in THAT order at field tag_start.
Allocator surplus does not reduce deterministic logical charges. Errors owned_bytes,
work,allocation retain wire Limit labels. No forced OOM runtime test; mapping gets
static review. Requested retained logical capacity excludes allocator surplus,
churn/transient buffers, stack and RSS; no CPU/walltime/whole-pipeline bound.

Error order: bytes admission, wire record, record work, read/structural validation,
dispatch, owned admission, reserve work, allocation. Truncated end after failed
read work debit; short bulk read debits remaining+1 at body_start, then Truncated
at input end. MalformedVarint tenth continuation, NegativeLength length-start,
other structural errors tag-start. All offsets<=len. Malformed/limited tail
fails after prior selected values; zero-field all-zero limits succeeds. Clamp
all hard limits; no length-driven reservation or recursion/arithmetic from caller
spans. Budgets remain separate from header/d1/payload/Class/Function/Type scans.

## Exact prospective allowlist, NO present source grant

Exclusive NEW files only (15):
1. crates/ktlsp-binary/src/metadata/type_argument.rs
2. crates/ktlsp-binary/tests/metadata_type_argument.rs
3. crates/ktlsp-binary/tests/fixtures/metadata_type_argument/selected.hex
4. crates/ktlsp-binary/tests/fixtures/metadata_type_argument/duplicates.hex
5. crates/ktlsp-binary/tests/fixtures/metadata_type_argument/opaque.hex
6. crates/ktlsp-binary/tests/fixtures/metadata_type_argument/pair_argument.hex
7. crates/ktlsp-binary/tests/fixtures/metadata_type_argument/Pair.class.bin
8. crates/ktlsp-binary/tests/fixtures/metadata_type_argument/PAIR_EXTRACTION.json
9. crates/ktlsp-binary/tests/fixtures/metadata_type_argument/EXPECTATIONS.md
10. crates/ktlsp-binary/tests/fixtures/metadata_type_argument/README.md
11. crates/ktlsp-binary/tests/fixtures/metadata_type_argument/LICENSE.txt
12. crates/ktlsp-binary/tests/fixtures/metadata_type_argument/NOTICE.txt
13. crates/ktlsp-binary/tests/fixtures/metadata_type_argument/PROTOBUF_LICENSE.txt
14. crates/ktlsp-binary/METADATA_TYPE_ARGUMENT_SPEC.md
15. crates/ktlsp-binary/METADATA_TYPE_ARGUMENT_REVIEW.md

Sole shared later root-serialized additive export:
metadata/mod.rs pub mod type_argument. wire/string_table/Budget, TypeHeader198,
TypeTable187, class-member193 helpers, Cargo/lock/dependencies/workflows/vendor/
parser/server/root status/docs/README/HANDOFF READONLY. No pub API caller break.
Notice copies byte-identical metadata_wire; genuine Pair read unmodified from
already-existing licensed stdlib archive. sources/ are temp references, never
shipped. Issue draft unposted; contract/tempdata only.

## Review and gates

SAME architect, independent correctness/functionality and adversarial reviewers
must review actual frozen packet; fix/recheck actionable findings. Then report
READY with hashes/clean sourcebase and issue draft to root. No source/native/
compiler/publication/CI until explicit root source and compile-slot grants.
Genuine composition through TypeHeader198 is intended/unexecuted until its
delivery and root-assigned future integration base; no borrowed owner source.
Later same3 source+actual completed unchanged original Linux CI reviews, own
branch push/attached PR, exclusive expected-head/base root lease, expected-SHA
merge and independent exact-main original proof. Preserve first failures; no
unchanged reruns/bypass/force-main/settings changes.

No local product/parser/worker/server/tests/bench/JVM/compiler/oracle execution.
T008/T008a HOLD; broaderC13/B3/B4 GAP; B11 UNVERIFIED; #7/#83/#96 OPEN,
Darwin UNKNOWN; #18/#68 remain OPEN. Data-only reader is not semantic parity.

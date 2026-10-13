# Selected Function header — source admitted, runtime validation pending

Root admitted the exact finite source/export scope on frozen fb23439. The original
contract below is preserved unchanged as historical preparation acceptance. Source
now exists on the held owned branch. Local fmt/compile-only Clippy admitted; no
local runtime. Same reviewers must review source; issue publication follows their
approval. Push/PR/CI/integration/merge remain held for root-assigned stable base.
The auxiliary source copies/extraction script remain outside the repository.

# Selected Kotlin Function header — preparation only

2026-10-13. Frozen own clone /private/tmp/ktlsp-function-header-01a134e9,
branch codex/function-selected-header-contract, HEAD
fb23439f65f2cc43510f6e4cfd26ffed814b0256, tree
ed8c788aa61a94beb9f7add542c98066c23110ec, sole parent
7b6f33529a4789d5e12cafd32da241fb5d240f89. No main chasing.
Parents #68/#18 remain OPEN. This package is an auxiliary proposal, not product
source, executable tests, publication, PR, CI, delivery or source admission.

## Finite result

Decode only the required Function name index and independent raw flags/old_flags,
borrowing the complete supplied unframed message. This is a prerequisite for later
Package/Class declaration-name traversal. There is no production consumer here.
No Package wrapper, recursive member/type/parameter/annotation/contract decoding,
modifier interpretation, compatibility conversion, links, semantic validation,
name lookup, normalizing rewrite, field collection, cache or dependency.

## Exact proposed API

New metadata::function_header module, mirroring class_header conventions:

```rust
pub use super::wire::Limits;
#[derive(Debug, PartialEq, Eq)]
pub struct FunctionHeader<'a> {
    pub raw: &'a [u8],
    pub flags: Option<i32>,
    pub old_flags: Option<i32>,
    pub name: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Wire(super::wire::ErrorKind),
    MissingName,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub offset: usize,
    pub kind: ErrorKind,
}
pub fn decode(bytes: &[u8], limits: Limits) -> Result<FunctionHeader<'_>, Error>;
```

Error Display/std::error::Error and private From<wire::Error> follow class_header;
Display contains kind and offset only. No shared error/public helper changes.
No effective-flags getter: flags None and old_flags None each have schema default6,
but neither becomes Some6 nor supplies/overrides the other. Explicit6/zero remain
present. name is required on success, including signed negative/zero raw values;
no index check or table access. Public caller-constructed structs have no validated
object guarantee; future consumers must check their consumed values and must not
assume raw/typed equivalence.

Input is ONE UNFRAMED Function body selected independently by its caller from
supported Kotlin mv=[2,3,0] Class(k1) or Package(k2/5) data. Class field9 and Package
field3 are repeated Function containers, not the selected Function's flags field9.
The bytes-only API cannot distinguish a Function body from another message, enforce
version/kind, or choose/parse a container. It does not invoke existing header/d1,
payload/table, wire::decode or name_resolver. All errors are Function-relative;
composing callers may checked-add the body offset themselves. raw is pointer-identical
to the entire supplied slice, preserving order/unknown fields/duplicates. O(1)
retained state, no allocation/recursion, no owned raw copy or fields vector.

## Independently verified pinned source

Kotlin v2.3.20 commit d57eb4a26ed01ab03cf195da686c8047032ff16f reused from
existing metadata work, copied read-only as data into this package's sources/:

- core/metadata/src/metadata.proto, sources/metadata.raw.proto; Git blob
  18cfd13e9e512c491f2f62ec241f49c81d6e5e81, SHA256
  a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2.
  Function lines309..359: optional int32 flags9 default6, old_flags1 default6,
  required int32 name2; all other fields are outside this selected header.
- core/metadata/src/org/jetbrains/kotlin/metadata/ProtoBuf.java, sources/ProtoBuf.java;
  Git blob fc424d5e7aac8661deb862c8e791d4fcf779c194, SHA256
  da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716.
  Function16506 onward; parser exact tags8/16/72 independently set presence and
  assign readInt32 to oldFlags/name/flags. Repeated recognized scalar assignments
  overwrite the same field. initFields17257 sets flags6/oldFlags6/name0 without
  setting presence. isInitialized17278 first requires name then recursively checks
  nested types/parameters/tables/contracts/plugin data/annotations/extensions.
  This decoder deliberately does NOT perform that recursive initialization check.
- Existing patched CodedInputStream SHA256
  7fbb3aeebd3c3d5414d7171bc3ba2785de50b40863a4f381c4b43986e15784fe and
  protobuf2.6.1 bba83652e1be610bdb7ee1566ad18346d98b843c remain the unchanged wire
  baseline, documented in frozen tests/fixtures/metadata_wire/README.md.

Only (field1,wire0)->old_flags, (field2,wire0)->name, (field9,wire0)->flags are
recognized. Wire varint -> low32 bits interpreted signed int32, never zigzag;
nonminimal/negative/unknown flag bits retained. Each field last-recognized-wins
independently. Wrong-wire fields neither set/clear presence nor overwrite values.
Other fields are framed only. Wire0/1/2/5 are accepted for any other number, even
if that schema field's expected wire differs. Length-delimited content is wholly
opaque, including selected fields with wrong wire: malformed nested protobuf,
empty required ValueParameter/TypeParameter, arbitrary UTF8 and packed varints
can succeed. Success establishes selected-header decode ONLY, not full Function
isInitialized, compiler acceptance, resolved name/type, symbol or LSP feature.

Unchanged wire policy: nonminimal varints terminate within10 bytes; tags/lengths
retain low32 bits; wire0 retains low64 slow-path bits; signed negative low32 lengths
fail. Ten continuation bytes fail. Field0 precedes wire validation; groups3/4
unsupported, wires6/7 invalid. Deterministic raw64 follows existing slow-path rule;
pinned JVM buffer-dependent fast-path compatibility is not universally claimed.
No group-based end-of-message, canonical encoding or general protobuf conformance.

## Whole scan, accounting and error precedence

Reuse wire::Limits {bytes,records,work}, hard ceilings16MiB/65,535/64Mi units.
Caller can tighten only; usize::MAX clamps. No envelope/schema-root/node/depth
charge. Empty bytes with all limits0 returns MissingName at0.

First reject bytes.len()>min(limits.bytes,MAX_BYTES) as Wire(Limit("bytes")) at0.
ONE wire::Budget::new(records,work) spans the whole scan. While pos<len call
existing wire::field(bytes,&mut pos,len,&mut budget), then budget.work(1,tag_start)
BEFORE dispatch. Propagate failures immediately, no retry/refund/reset/partial
result. Every complete field including unknown/wrong-wire pays dispatch. Do not
stop when all selected fields are present. Only clean EOF permits MissingName at
bytes.len(); any wire/limit failure takes precedence, with or without prior name.

Each field attempt charges record1 then work1 at tag start before tag reading.
Each read/skipped byte costs work1 once; no nested/packed rereads. Complete N-byte,
F-field scan costs N+2F work and F records. Constant scalar/raw/EOF bookkeeping
and missing-name check are unmetered. Failed single reads debit one before
Truncated at end. Failed bulk reads debit available bytes+1 at bulk-read start
before Truncated at end. Structural errors precede dispatch; dispatch failure
reports tag start after full field read. Record failure precedes record-work;
record-work precedes reading; field0 precedes bad wire; malformed varint reports
tenth continuation; NegativeLength reports length-varint start. Exact cut/limit
outputs are frozen in FIXTURES.md. No whole-pipeline/CPU/walltime/stack/RSS claim.
Header/d1, payload/table, flat-wire traversal and resolver are separate operations
with separate budgets. No assumption that composing independently bounded calls
forms an aggregate end-to-end bound.

## Reach and ownership (future, root-gated)

Exclusive NEW paths only:
- crates/ktlsp-binary/src/metadata/function_header.rs
- crates/ktlsp-binary/tests/metadata_function_header.rs
- crates/ktlsp-binary/tests/fixtures/metadata_function_header/**
- crates/ktlsp-binary/METADATA_FUNCTION_HEADER_SPEC.md
- crates/ktlsp-binary/METADATA_FUNCTION_HEADER_REVIEW.md

Shared metadata/mod.rs additive `pub mod function_header;` ONLY after root admission
and prior ownership release. Reuse wire::field/Budget already pub(super), no helper
change. Callers: new integration tests only; no caller migration. Tests compose
existing metadata/payload/wire/name_resolver with Unit data using distinct budgets;
existing regressions remain unchanged. No Cargo/lock/dependency/config/workflow/
wire/common-error/parser/vendor/server/lib.rs/root-status documentation changes.
Existing fixture/LICENSE/NOTICE/protobuf notice copies retained unchanged.

## Validation and admission gates

FIXTURES.md and five *.hex files freeze independent authored/genuine data before
product source. extract_unit.py is data-only artifact provenance/self-check, not
product/test/worker/parser execution or a second semantic engine. Source copies
are reference evidence only; no generated Java/runtime bundled or executed.
Own architect plan approved; separate correctness/functionality and adversarial
contract review plus architect finished-contract review required before READY.
Preserve initial findings; fix and recheck all changed expectations.

No product/source/export changes, issue publication, push/PR/CI/implementation or
merge until concrete root admission. Local product/parser/worker/server/tests/
bench/runtime/JVM/compiler/oracle/Gradle/Maven execution prohibited while #96
OPEN/UNKNOWN. Later admission may allow local fmt + compile-only locked workspace
all-target Clippy; tests only unchanged original Linux Native Rust CI. SAME2 plus
own architect retained for held source and final root-assigned combined source +
actual completed original CI readbacks. Owner pushes own branch, opens/ATTACHES PR,
waits required CI and root expected-head exclusive merge grant, ordinary guarded
squash, owner/root independent same-tree/sole-parent exact-main evidence. No admin/
force/protection bypass or alternate executor/retry-to-green.

#68/#18 OPEN; #7/#83 OPEN; T008/T008a HOLD; B3/B4 GAP, B11 UNVERIFIED, #96 UNKNOWN,
historical45/32/10/3 unchanged. No full metadata/grammar/semantic/parity/performance claim.

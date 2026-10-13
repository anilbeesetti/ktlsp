# #146 selected Class header — source admitted, validation pending

Root admitted the exact finite source and issue-publication scope after actual
SAME2 plus own architect preparation approvals. Issue146 is published. The stage1
freeze below is preserved as historical acceptance text; source exists only on
the held owned branch. No local runtime; only fmt and compile-only Clippy allowed.
Push/PR/CI/final integration/merge remain held for a future root-assigned base.

# Selected Kotlin Class header — stage 1 contract, not implementation

Preparation date 2026-10-13. Frozen independent clean detached clone:
/private/tmp/ktlsp-class-header-01a1349d, HEAD
882028224a252d728dd479f2403fd2f2d97f3f67, tree
350124dfd59e3f6bc895674591d60bf28744f3a4. Own .git (not linked/shared).
Parents https://github.com/anilbeesetti/ktlsp/issues/68 and
https://github.com/anilbeesetti/ktlsp/issues/18 read back OPEN.
No product/source edits, branch, issue publication, push, PR, CI or merge admission.

## Finite result and usefulness

Decode only Class-container fq_name index and flags, retaining original raw bytes.
These are the minimal identity-index/flags inputs for a later binary declaration
consumer. This child has only integration-test consumers; no production consumer,
symbol, name lookup, flag-bit interpretation or Kotlin declaration is delivered.
No integer lists, Package support, types, member parsing, links, normalization,
recursive validation or owning self-borrowing aggregate. No full Class validity.
Unknown, unselected schema fields and extensions are retained solely by borrowing
the complete raw message; there is no separate field collection or iterator.

## Exact proposed public API

New module metadata::class_header, additive root-gated metadata/mod.rs export only:

```rust
pub use super::wire::Limits;
#[derive(Debug, PartialEq, Eq)]
pub struct ClassHeader<'a> {
    pub raw: &'a [u8],
    pub flags: Option<i32>,
    pub fq_name: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Wire(super::wire::ErrorKind),
    MissingFqName,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub offset: usize,
    pub kind: ErrorKind,
}
pub fn decode(bytes: &[u8], limits: Limits) -> Result<ClassHeader<'_>, Error>;
```

Error implements Display/std::error::Error, quoting only kind and offset, matching
existing wire conventions. Wire failures preserve kind and offset inside Wire;
no shared public wire ErrorKind addition. No new dependency/helper exposure.
flags=None means absent, effective schema default6; Some6 means explicit default,
Some0 means explicit zero. No default getter/wrapper. fq_name is present on every
successful result; absent is never silently converted to schema storage default0.
All scalar values retain signed int32 data, including negatives and unknown bits.
Public caller-constructed ClassHeader has no validated-object guarantee; future
consumers must validate their own consumed indices/flags and never trust raw/typed
equivalence. This API accepts bytes, never public wire Fields/spans/typed headers.

Input is exactly ONE UNFRAMED Class message, e.g. existing payload::Payload.payload
after the caller independently checks metadata::Outcome::Decoded k=1,mv=[2,3,0].
The bytes-only decoder cannot enforce this selection or distinguish Class/Package.
It never invokes header/d1, payload/table or resolver itself. All error offsets
are supplied-Class-relative, not d1/classfile-relative. A composing caller may
checked-add payload_span.start; no offset adapter or aggregate API in this child.
raw borrows exactly the supplied bytes (pointer identity, order and duplicates).
No copied payload, fields, vectors, cache or allocation; O(1) retained state.

## Pinned source rules and explicit subset

Reuse Kotlin v2.3.20 d57eb4a26ed01ab03cf195da686c8047032ff16f already pinned by
HEADER/WIRE/STRING_TABLE/NAME_RESOLVER/PAYLOAD work. Sources are data only:
- core/metadata/src/metadata.proto saved as sources/metadata.raw.proto;
  Git blob18cfd13e9e512c491f2f62ec241f49c81d6e5e81,
  SHA256 a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2.
  A prior connector-content/apply_patch copy sources/metadata.proto added one
  trailing newline (cmp: identical prefix, raw EOF; sizes separately recorded).
  Its SHA256 a34605a82b345ef19de542e0d769cb01a1372740af47a9b0d9a7098870e6a06d
  is retained as retrieval evidence, not the authoritative pinned byte identity.
- core/metadata/src/org/jetbrains/kotlin/metadata/ProtoBuf.java Git blob
  fc424d5e7aac8661deb862c8e791d4fcf779c194,
  SHA256 da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716.
  Class parser lines9138..9200: exact tags8/24 set presence and overwrite value;
  default switch handles other tags as unknown. initFields10220..10221 sets
  flags6/fqName0; isInitialized10250 onward also recursively checks members and
  extensions, which THIS CHILD DOES NOT DO.
- Existing patched CodedInputStream source pin/hash and protobuf2.6.1
  bba83652e1be610bdb7ee1566ad18346d98b843c remain wire fixture baseline.

Class field1/wire0 selects flags optional int32 default6; field3/wire0 selects
required fq_name int32. Convert wire value via low32 bits interpreted signed,
never zigzag; no range/name grammar/enum validation. Singular recognized fields
last-win. Wrong-wire occurrences are unselected opaque data and neither set nor
clear presence nor overwrite prior recognized values. Every complete top-level
field is visited to EOF, even after both fields are present. Missing fq_name is
checked only after a clean complete scan. Raw retains known/unknown duplicates.

All other top-level fields are wire-framed only; wire0/1/2/5 accepted regardless of
schema number/expected wire. Length-delimited member, type, annotation, extension,
packed-list or unknown contents remain wholly opaque: no nested/packed validity,
required-member-field, extension-registry, UTF8 or semantic checks. E.g. an empty
TypeParameter or malformed Function body can succeed if fq_name is present.
Success means selected-header decode, never Kotlin Class isInitialized or compiler
acceptance. Field0, wire6/7 and groups3/4 fail using existing wire rules (field0
precedes wire validation); no early success or group-based end-of-message.

Existing deterministic wire rules apply unchanged: tags/lengths low32, nonminimal
varints up to10 bytes, wire0 low64, signed-negative low32 length rejected, tenth
continuation malformed. No universal JVM fast-path compatibility; no canonical
encoding enforcement. Existing wire's flat decoder still requires its own d1
envelope and is not called here. payload::decode still leaves its tail wholly opaque.

## Exact operation bounds, offsets and precedence

Reuse wire::Limits { bytes, records, work }; hard ceilings16MiB,65,535,64Mi units,
each caller-tightenable (usize::MAX clamps). No nodes/depth/output knobs because
there is no nested traversal or heap output. No schema/root/envelope record or
initial work charge. Empty input with zero records/work reaches MissingFqName0.

First reject total bytes>min(limits.bytes,MAX_BYTES): Wire(Limit(bytes)) at0.
Construct ONE existing wire::Budget::new(records,work). Loop only while pos<len,
calling existing pub(super) wire::field(bytes,&mut pos,len,&mut budget), propagate
any failure immediately (no retry/refund/reset). After each complete field charge
budget.work(1,field.span.start) BEFORE dispatch. Unknown and wrong-wire fields pay
identical dispatch work. Borrowed raw bookkeeping, scalar initialization, EOF and
required-presence check are constant unmetered work. No whole-pipeline/RSS/CPU claim.

Each field attempt charges record1 then work1 at tag start, before reading tag.
Each read/skipped opaque byte charges work1 exactly once, including payload bodies.
No reread/nested/packed charge. Complete N-byte/F-field scan costs N+2F work,
F records; no allocation charge. Failed read attempts and bulk boundary failures
retain existing rules: available bytes+1 checked debit at bulk-read start before
Truncated at section end; single failed byte debit at current cursor; tenth
continuation MalformedVarint at that byte. Wire structural failure precedes dispatch.
Record limit precedes record-work; record-work precedes tag/value error. Dispatch
work failure follows the full field read and reports that field's tag start.
Any wire/dispatch failure precedes MissingFqName; malformed tails cannot be hidden
by absent or already-present fq_name. MissingFqName offset is bytes.len() on clean
EOF only. No partial result. All ranges/slices originate from bounded helper input.

## Future owned paths and integration reach

Exclusive proposed files (not created in repo at stage1):
- crates/ktlsp-binary/src/metadata/class_header.rs
- crates/ktlsp-binary/tests/metadata_class_header.rs
- crates/ktlsp-binary/tests/fixtures/metadata_class_header/**
- crates/ktlsp-binary/METADATA_CLASS_HEADER_SPEC.md
- crates/ktlsp-binary/METADATA_CLASS_HEADER_REVIEW.md
Only shared change: root-gated additive `pub mod class_header;` in metadata/mod.rs.
Existing wire::field and Budget are ALREADY pub(super); no wire/header/table/
resolver/payload/lib.rs/Cargo/lock/workflow/root-doc/vendor/parser/server edits.
No callers to migrate. New tests compose existing header/payload/resolver with a
precompiled Unit as separately budgeted calls; no production caller/export rework.
Existing header/wire/table/payload/resolver regressions must stay unchanged.
Root owns future reconciliation of shared export/integration against admitted base.

## Fixture and validation freeze

FIXTURES.md and authored *.hex freeze expectations before product source/runtime.
Existing Unit.class is reused unmodified as data; SHA/provenance/licenses in
FIXTURES.md. Future fixture directory copies existing wire Apache LICENSE/NOTICE
and protobuf BSD notice byte-for-byte. No generated Java source/runtime is shipped.
Source data here is outside repository; any reproduction must preserve licenses.
No Kotlin/Java/compiler execution, external tool, filesystem/network runtime path.

Stage1 runs no Cargo/fmt/compile/build/native tests/parser/worker/server/bench/JVM/
oracle/Gradle/Maven. After root source admission: same correctness/adversarial plus
own architect review combined source; authorized local fmt/compile-only Clippy;
push own branch/open+attach PR; existing unchanged original Linux Native Rust CI
fmt/clippy/full-workspace tests on root-assigned combined head/tree; same3 actual
completed-CI readbacks; guarded expected-head ordinary merge only after root window.
Owner+root independently verify exact-main same tree/sole parent/original all3.
No additional postmain reviewer turns for identical accepted tree; identity/workflow
change, failure or new finding renews affected reviews. No new executor/CI reruns.

#83/#7 OPEN, T008/T008a HOLD, B3/B4 GAP, B11 UNVERIFIED, #96 OPEN/UNKNOWN and
historical audit45/32/10/3 remain unchanged. Parents#68/#18 OPEN. No server feature,
official runtime parity, full metadata/class validity, speed or RAM claim follows.

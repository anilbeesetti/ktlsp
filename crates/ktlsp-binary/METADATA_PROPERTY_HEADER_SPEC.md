# Selected Property header — source admitted

Root admitted the frozen contract and only the additive module export on
2026-10-14. The contract below is preserved as the preimplementation freeze;
source and native CI review remain separate evidence. No local native runtime.

# Selected Property header: preimplementation contract

2026-10-14. Preparation only. Owned managed worktree
/Users/anil/.codex/worktrees/t018-property-header/ktlsp, branch
codex/t018-property-header, frozen base 941c72efd9896b0847ab342ec0dd22c10a000f3b.
No product/source/test/spec edits or runtime, issue publication, push, PR or CI.
Parent #68/#18 remain OPEN. Root must admit source and publication separately.

## Finite API and source contract

One new public metadata::property_header module follows class_header/function_header:

```rust
pub use super::wire::Limits;
#[derive(Debug, PartialEq, Eq)]
pub struct PropertyHeader<'a> {
    pub raw: &'a [u8],
    pub flags: Option<i32>,
    pub old_flags: Option<i32>,
    pub getter_flags: Option<i32>,
    pub setter_flags: Option<i32>,
    pub name: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind { Wire(super::wire::ErrorKind), MissingName }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error { pub offset: usize, pub kind: ErrorKind }
pub fn decode(bytes: &[u8], limits: Limits) -> Result<PropertyHeader<'_>, Error>;
```

Display, std::error::Error and private From<wire::Error> follow Function header.
No new shared errors/helpers/dependencies. No effective flags accessor. All four
Options independently retain presence; absence never becomes Some(default).

Pinned Kotlin v2.3.20 commit d57eb4a26ed01ab03cf195da686c8047032ff16f:
metadata.proto blob18cfd13e9e512c491f2f62ec241f49c81d6e5e81,
SHA256 a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2;
ProtoBuf.java blobfc424d5e7aac8661deb862c8e791d4fcf779c194,
SHA256 da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716.
Read-only copies in sources/ match existing reference pins; no Java executed.
Property schema starts line358 and generated Property starts19838. Exact selected
wire0 tags: old_flags8 (field1, default2054), name16 (field2, required),
getter_flags56 (field7, protobuf default0), setter_flags64 (field8, protobuf
default0), flags88 (field11, default518). Generated parser independently assigns
readInt32 and presence on each recognized tag. Repeated assignments last-win.
Accessor absence semantically inherits selected property bits according to schema
comments; this API does NOT compute that inheritance or old/new compatibility.

Recognized values keep low32 signed int32, including negative name indices and
unknown flag bits. Wrong wires neither set nor clear presence nor overwrite a
recognized value. Unselected fields only receive flat wire framing. Length bodies
are wholly opaque, including malformed nested protobuf, invalid UTF8, empty
required nested messages and malformed packed values. No schema Type/member/
annotation/contract decoding or full protobuf initialization claim follows.

Input is ONE unframed Property body independently selected by caller from supported
mv=[2,3,0] Class(kind1) field10 or Package(kind2/5) field4. Bytes-only API cannot
check kind/version/container identity. Do not reinterpret entire Class/Package as
Property. raw is pointer-identical to the entire input, preserving unknown fields,
wrong wires, duplicates and ordering. All offsets are Property-body-relative.
Caller-constructed public structs can have inconsistent raw/typed values; future
consumers must validate consumed values. Adding a module is additive; no existing
struct/enum shape changes or caller migrations are proposed.

## Whole operation, error precedence and storage

Reuse wire::field plus ONE wire::Budget for entire scan. Limits bytes/records/work
clamp to existing hard16MiB/65535/64Mi units. Reject oversized bytes first at0;
record/work counters span all fields, including wrong-wire and unknown fields.
Every completed field pays dispatch work1 at tag start BEFORE dispatch. No early
success after name or flags. Wire errors take precedence over MissingName; only
clean EOF without recognized name gives MissingName at bytes.len(). Empty input
with all limits0 gives MissingName0. No partial result/retry/reset/refund.

Successful N-byte/F-field scans cost N+2F work and F records: record1+work1 before
tag, one per byte read/skipped, dispatch1. Failed single read debits1 before EOF;
bulk shortage debits available+1 at payload start before Truncated at EOF. Record
failure precedes work; wire structure precedes dispatch. MissingName/scalar/EOF
bookkeeping unmetered. Retained state O(1), no allocation/recursion/owned copy/vector;
large length bodies are charged/skipped once without decoding or rereading. No
whole-pipeline/RSS/CPU/walltime bound. Separate header/d1/table/resolver calls have
separate budgets. wire slow-path low64/nonminimal and strict envelope policies
remain unchanged, including possible JVM fast-path differences and group rejection.
Selected malformed protobuf UTF8 elsewhere remains UnsupportedUtf8 GAP; opaque
Property bodies neither validate nor repair it.

## Exact future reach (exclusive new paths)

- crates/ktlsp-binary/src/metadata/property_header.rs
- crates/ktlsp-binary/tests/metadata_property_header.rs
- crates/ktlsp-binary/tests/fixtures/metadata_property_header/README.md
- crates/ktlsp-binary/tests/fixtures/metadata_property_header/EXPECTATIONS.md
- crates/ktlsp-binary/tests/fixtures/metadata_property_header/minimal.hex
- crates/ktlsp-binary/tests/fixtures/metadata_property_header/explicit.hex
- crates/ktlsp-binary/tests/fixtures/metadata_property_header/duplicates.hex
- crates/ktlsp-binary/tests/fixtures/metadata_property_header/opaque.hex
- crates/ktlsp-binary/tests/fixtures/metadata_property_header/LICENSE.txt
- crates/ktlsp-binary/tests/fixtures/metadata_property_header/NOTICE.txt
- crates/ktlsp-binary/tests/fixtures/metadata_property_header/PROTOBUF_LICENSE.txt
- crates/ktlsp-binary/METADATA_PROPERTY_HEADER_SPEC.md
- crates/ktlsp-binary/METADATA_PROPERTY_HEADER_REVIEW.md

Shared metadata/mod.rs proposal ONLY `pub mod property_header;`, after explicit
root serialized slot. No Operation change needed. Existing wire/accounting,
TypeParameter owner files, Cargo/lock/workflow/lib.rs/README/root status unchanged.
Callers: new tests only. Existing tests remain regression controls. Reuse unchanged
metadata_wire fixture notices/licenses; authored raw bytes are project data.
No genuine compiled Property/oracle comparison is claimed or required here.

## Validation/delivery gates

FIXTURES.md freezes independently derived literal expectations BEFORE source.
Own architect plan approved; same architect plus separate correctness/functionality
and adversarial contract approvals required before READY. Preserve initial findings,
fix/recheck changed expectations. Source and public ticket remain held for root.
No local native product/parser/worker/server/tests/bench runtime under #96. Root
must coordinate later compile-only Clippy/fmt. Future unchanged original Linux
Native Rust CI runs pinned fmt, locked workspace all-target Clippy and locked
workspace tests. SAME3 review final source plus completed originalCI; root controls
exclusive expected-base/head merge lease. No admin/bypass/force/main/settings edits.

T008/T008a HOLD; broader C13/B3/B4 GAP; B11 UNVERIFIED; #7/#83/#96 OPEN;
Darwin UNKNOWN. Syntax/data success is not semantic parity/platform/performance.

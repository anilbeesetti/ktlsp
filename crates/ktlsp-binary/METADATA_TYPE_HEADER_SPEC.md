# Selected Type header — #197 source reviewed, runtime validation pending

Root admitted the exact frozen contract a90880e51fedead92eba370cebf9b83a30e49c06314dc8a0fc51ec9b3b5b5157
and expectations b74771ca41036bdc0e5d1a96409e173b459a46f513d7255d0846a4d647a47dd4,
twelve exclusive new paths and additive metadata/mod.rs export. Local formatting
and compile-only Clippy are admitted; no local native runtime/tests. Publication
and original unchanged Linux Native CI are admitted after SAME3 source review.
Root admitted the exact final integration cut at fe4f4a73f0bf9190f7668bf74a9b963f70114449;
new original CI and SAME3 combined-source/completed-CI reviews are required.
Exclusive expected-head/base merge lease remains held.
The preimplementation acceptance contract below is preserved unchanged.

# T018 selected Kotlin Type header — preparation only, v1

2026-10-14. Root assignment: small child of #68/#18. Frozen clean managed
worktree /Users/anil/.codex/worktrees/t018-type-header/ktlsp, branch
codex/t018-type-header, HEAD b026508e126e64572fd3a4b646fee95304a5768d,
tree b0db5b2651e31d7ce33657fd4495f9600f5658ca. No source, export, issue,
push, PR, CI, compiler or runtime grant follows from this preparation.

Decode ONE caller-selected unframed Type message into ten optional scalars and
the borrowed complete raw input. Reuse wire::field/wire::Budget unchanged.
No allocation, recursive Type/Argument/Annotation decoding, graph, classifier
selection, type-table lookup, name/index validation, nullability semantics,
linkage, substitution, symbols or server consumer. Empty and conflicting
classifier messages are valid structural data. No full protobuf initialization
or Kotlin semantic acceptance claim. Nested singular Type fields merge upstream;
this API does not materialize or merge nested objects, retaining ALL occurrences
only in raw, including malformed nested bodies and duplicate annotations.

## Exact API

New metadata::type_header module:

```rust
pub use super::wire::{Error, ErrorKind, Limits};
#[derive(Debug, PartialEq, Eq)]
pub struct TypeHeader<'a> {
    pub raw: &'a [u8],
    pub flags: Option<i32>,
    pub nullable: Option<bool>,
    pub flexible_type_capabilities_id: Option<i32>,
    pub class_name: Option<i32>,
    pub type_parameter: Option<i32>,
    pub flexible_upper_bound_id: Option<i32>,
    pub type_parameter_name: Option<i32>,
    pub outer_type_id: Option<i32>,
    pub type_alias_name: Option<i32>,
    pub abbreviated_type_id: Option<i32>,
}
pub fn decode(bytes: &[u8], limits: Limits) -> Result<TypeHeader<'_>, Error>;
```

There is no Type-specific error, required field or new ErrorKind variant: wire
errors already cover every rejection. Reexport avoids duplicating error wrappers
that cannot add information. Error.offset is Type-input-relative despite wire's
historical d1-relative doc. Wire Display/Error implementation unchanged; content
never enters error messages. Limits is existing bytes/records/work with hard
defaults16MiB/65,535/64Mi units. No owned_bytes/nodes/depth fields: retained
requested heap capacity is ZERO; no allocation/recursion/nested traversal occurs.
No effective-default getters: absent bool=false/int32=0 in schema, but None stays
None. Public struct is caller-constructible; no validated-object/raw-equivalence
guarantee. No unknown/fields/nested-body vectors; complete pointer-identical raw
preserves fields, order, duplicate occurrences, wrong wires and exact bytes.

## Verified pinned sources, data only

Existing offline copies /private/tmp/ktlsp-type-parameter-contract/sources verified
by SHA256 before preparation; no upstream compiler, runtime or encoder executed.
Kotlin v2.3.20 d57eb4a26ed01ab03cf195da686c8047032ff16f:
- metadata.proto Type121–175, blob18cfd13e9e512c491f2f62ec241f49c81d6e5e81,
  SHA256 a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2.
- ProtoBuf.java Type4896, parser4944–5065, defaults6088, initialization6106,
  blobfc424d5e7aac8661deb862c8e791d4fcf779c194,
  SHA256 da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716.
- Patched CodedInputStream.java readBool382 raw64!=0, SHA256
  7fbb3aeebd3c3d5414d7171bc3ba2785de50b40863a4f381c4b43986e15784fe.
Existing protobuf2.6.1 bba83652e1be610bdb7ee1566ad18346d98b843c wire baseline
and third-party notices remain unchanged. No upstream source/runtime shipped.

Recognize ONLY wire0 with field1 flags,3 nullable,4 capabilities ID,6 class name,
7 parameter ID,8 flexible upper ID,9 parameter name,11 outer ID,12 alias name,
14 abbreviated ID. Each scalar independently last-recognized-wins. All integers
take low32 signed int32, never zigzag. Nullable uses existing full deterministic
u64!=0, neither strict0/1 nor low32!=0. Wrong-wire1/2/5 neither set nor clear
presence and never replace previous recognized values. Nested2/5/10/13/100,
extensions101..199 and every unselected number retain only raw framing; no body
protobuf/UTF8/required-field validation. Mixed inline/ID bounds and all four
classifiers can coexist; no exclusivity or association checks.

Inherited wire policy: nonminimal varints terminating within10 bytes accepted;
tags/lengths use low32; varint values use low64 slow-path rule. Ten continuation
bytes fail at tenth. Negative signed low32 lengths fail. Field0 precedes wire
validation; groups3/4 unsupported; wires6/7 invalid. No canonical or universal
buffer-dependent JVM fast-path compatibility claim. Add existing-style shortcut
comment pointing to inherited raw64 limitation, no helper changes. Resolver's
selected malformed-UTF8 UnsupportedUtf8 GAP is unchanged by this text-free API.

## Aggregate budget, exact errors and overflow

Byte ceiling first at0: len>min(limits.bytes,MAX_BYTES) -> Limit(bytes).
One wire::Budget::new(limits.records,limits.work) spans ENTIRE message. While
pos<len, call wire::field(bytes,&mut pos,len,&mut budget), then budget.work(1,
field.span.start) BEFORE dispatch. No early stop, reset/refund/retry or partial
success. All selected/unknown/wrong-wire complete fields pay dispatch.

Successful N-byte/F-field scan consumes records F and work N+2F. No envelope,
root/node/depth charge, packed reread, scalar interpretation surcharge or
requested capacity charge. Empty bytes with bytes/records/work0 succeeds with
all None and pointer-identical empty raw. Scalar/raw bookkeeping is constant
unmetered stack work; work is an accounting bound, not CPU/RSS/walltime.

Error order follows unchanged wire: record1 then record-work1 before reading;
each read/skipped byte work1; failed byte read charges1 before Truncated at end.
Failed bulk read charges available+1 at body start before Truncated at input end.
Structural errors precede dispatch, dispatch failure at tag start follows full
field read. Invalid field0 precedes bad-wire. Malformed varint offset is tenth
continuation; NegativeLength offset is length start. No semantic/EOF error.
All offsets <= supplied length, no following-container bleed. Raw lengths/end
come only from checked wire reader, no caller-supplied span or arithmetic.
Existing checked_sub budgets and bounded cursor lengths preserve overflow/error
policy; usize::MAX clamps all three limits. No new arithmetic/dependency/helper.
Hard bytes/records bounds imply successful work<=16MiB+131070, so hard64Mi
work cannot be independently reached on successful flat scans. Test smaller
work cuts instead of inventing a hard-work witness.

Header/d1, payload/table, Class, flat wire, Function, Type and resolver operations
retain SEPARATE budgets. Composition test establishes the concrete call path,
not an aggregate whole-pipeline work/memory bound or production integration.

## Reach and exact prospective write set

Twelve exclusive NEW paths, no wildcards:
1. crates/ktlsp-binary/src/metadata/type_header.rs
2. crates/ktlsp-binary/tests/metadata_type_header.rs
3. crates/ktlsp-binary/tests/fixtures/metadata_type_header/selected.hex
4. crates/ktlsp-binary/tests/fixtures/metadata_type_header/opaque.hex
5. crates/ktlsp-binary/tests/fixtures/metadata_type_header/unit_type.hex
6. crates/ktlsp-binary/tests/fixtures/metadata_type_header/EXPECTATIONS.md
7. crates/ktlsp-binary/tests/fixtures/metadata_type_header/README.md
8. crates/ktlsp-binary/tests/fixtures/metadata_type_header/LICENSE.txt
9. crates/ktlsp-binary/tests/fixtures/metadata_type_header/NOTICE.txt
10. crates/ktlsp-binary/tests/fixtures/metadata_type_header/PROTOBUF_LICENSE.txt
11. crates/ktlsp-binary/METADATA_TYPE_HEADER_SPEC.md
12. crates/ktlsp-binary/METADATA_TYPE_HEADER_REVIEW.md

One ROOT-SERIALIZED later additive metadata/mod.rs `pub mod type_header;` export
requires separate explicit grant. No other edit to mod, wire, string_table,
name_resolver, payload, Class/Function/Property header helpers. No Cargo/lock,
workflow/status/README/HANDOFF/server/parser/vendor or dependency edits. New
tests are sole Type decoder callers. Existing Class/Function/Property and queued
TypeTable/ValueParameter/TypeAlias may later consume caller-selected inline Type
bodies; none migrate in this child. Existing APIs/types/enum variants unchanged,
additions only; no struct-literal source break to existing callers. Existing
binary/header/wire/table/resolver tests remain unchanged. Fixture notices copied
byte-identically from metadata_wire; Unit.class/provenance preserved unchanged.

## Review and execution gates

EXPECTATIONS.md freezes literal bytes/values/cuts/budget/error outcomes BEFORE
implementation or runtime; no expectations produced by tested implementation.
Own architect initial plan review passed; SAME correctness/adversarial reviewers
and architect must approve final contract. Then root may grant exact scope.
No source/export/publication/compiler/runtime until root explicit grant.
No local product/parser/worker/server/tests/bench/oracle under #96. Eventual fmt
and compile-only Clippy need separate root admission. Original unchanged Linux
Native Rust CI is future runtime executor (fmt, locked workspace all-target
Clippy -D warnings, locked workspace tests). SAME3 source/fix/finished-original-CI
reviews, push own branch/open ATTACH PR, passing required CI, exclusive expected
head/base merge lease, reviewed expected-SHA guarded merge, owner/root exact-main
original attempt1 proof. Never bypass/force/settings changes or unchanged rerun
to green; preserve first failures. No cleanup of unrelated caches/artifacts.

#68/#18 remain OPEN. T008/T008a HOLD, broaderC13/B3/B4 GAP, B11 UNVERIFIED,
#7/#83/#96 OPEN, Darwin UNKNOWN. Data-only does not establish semantics, parity,
speed or RAM superiority. Contract review is readiness, not delivery evidence.

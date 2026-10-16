# PropertyTypes source admission amendment — 2026-10-14

Root explicitly superseded the historical serialized source/export hold through
parallel source grant SHA2563e787f529721ff4dc9b8912db689076513b4746f6f1de74bd5f1fbf5bc491c3a
at base6b965cfa8008df51deba5ff4a56be31e9b30d16b. Only24 new paths and own
additive module export admitted; helpers and all other existing paths readonly.
Local pinned fmt and compile-only locked workspace all-target Clippy permitted;
local native runtime/tests and commit/publication/CI/merge remain separately held.
Historical sealed contract below is preserved verbatim. No completed runtime
validation or source review is implied by source admission.

# T018/#18/#68 Property inline Type occurrences — preparation v1

TEMP CONTRACT/DATA ONLY, 2026-10-14. No source/runtime/export/publication admission.
Owned attached checkout /Users/anil/.codex/worktrees/t018-property-types-contract/ktlsp,
branch codex/t018-property-types-contract, clean assigned immutable HEAD
efbfb991014f926ab2031d489a53a12ef386ed7b, tree763035b56d29014b643abdd8af2937e0fa2266b7,
sole parent3a93659d6fa6ad252298684ead15efd32ee6ab7b. Root-clear file SHA256
d5cb0c595334579fba102e7fe3a9137b714320b577f7725b77a61a2763e46247 verified.
Later main/PR239 is not this preparation cut. ClassSupertype owner+root delivery/
release and root's exact grant are required before future source/export work.

## Smallest additive API

New metadata::property_types, using existing conventions:

```rust
pub use super::string_table::Limits;
pub use super::property_header::{Error, ErrorKind};
#[derive(Debug, PartialEq, Eq)]
pub struct SelectedType<'a> {
    pub field_number: u32,
    pub span: std::ops::Range<usize>,
    pub header: super::type_header::TypeHeader<'a>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct PropertyWithTypes<'a> {
    pub property: super::property_header::PropertyHeader<'a>,
    pub types: Vec<SelectedType<'a>>,
}
pub fn decode(bytes: &[u8], limits: Limits) -> Result<PropertyWithTypes<'_>, Error>;
```

Caller selects ONE unframed supported mv=[2,3,0] Property body and independently
establishes header/version/kind/container. Class field10 and Package field4 are
caller selection context only. Bytes alone cannot prove these identities.
Return complete PropertyHeader unchanged, plus ALL field3/wire2 return_type and
field5/wire2 receiver_type bodies in ONE vector in global wire order. Preserve
duplicates, empty bodies and exact field identity 3/5. span is body-only half-open
Property-relative; property.raw and header.raw borrow pointer-identically from
the full input and exact child slices, even empty slices at their encoded start.
No copied raw, extra field vector or merged singular-message value.

Occurrences are explicitly UNMERGED: protobuf singular Type merge/full recursive
initialization is not implemented. Existing scalar presence/default/low32 signed
int32/last-recognized-wins/wrong-wire rules survive both parent and each child.
Nullable preserves full-u64 nonzero raw boolean, without semantic meaning. All
ten Type Options must be asserted, including None; conflicting classifiers and
inline+ID coexist as uninterpreted data. Property IDs9/10, Type IDs, deeper Type/
Argument/Annotation bodies, and unselected parent bodies stay opaque. No lookup,
inline-ID precedence, type graph, names/binding, flags inheritance, nullability
meaning, consumers/providers, compatibility/parity/performance claims. Public
struct literals remain caller-constructible, without raw/typed consistency proof.
Use existing-style shortcut documentation: occurrences are unmerged; upgrade when
merged Type interpretation is implemented. Inherited slow-path raw64 limitation
remains explicit; no universal buffer-dependent JVM fast-path equivalence.

## Algorithm and accounting, exact existing helper reuse

1. bytes.len()>min(limits.bytes,MAX_BYTES) => Wire(Limit(bytes))@0.
2. ONE string_table::Budget::new(limits), node(1,0).
3. property_header::decode_with_budget(bytes,&mut budget.wire) TO COMPLETION.
   Complete parent wire validation and required-name checking precede children.
4. Second parent scan: wire::field, then work(1,field.span.start) for EVERY field.
   Only (3|5,Value::Bytes(raw)) selects a child: span=end-raw.len()..end;
   node(2,span.start); type_header::decode_with_budget(raw,&mut budget.wire);
   add span.start EXACTLY ONCE to only child-helper wire Error.offset; convert to
   existing Property Error. budget.push the single Vec at parent field tag start.
5. Return only after complete second scan. No partial result/reset/refund/retry.

Both helper functions already pub(super) at the assigned base. No helper exposure
change, public decoder call/reset, new helper/error/dependency or redesign needed.
Node depth->nodes->work; field records->record work->reads->dispatch; push checked
owned-byte charge->work charge->try_reserve_exact->push, exactly as existing helpers.
Node and push errors already Property-relative: never rebase them. Parent error
coordinates stay unchanged. First-pass parent wire/budget failure overrides an
earlier encoded bad child; MissingName@len follows only clean first scan and
overrides child faults. During rescan an immediate child failure precedes a later
rescan budget failure. Selected malformed child precedes its storage charge. Allocation
refusal inherits Wire(Limit(allocation)) at parent tag; static only, no forced
allocator-failure claim. Cursor-validated spans bound offset sums by input len.

Let B=parent bytes, P=parent fields, K=selected bodies, b_i/c_i=child bytes/fields,
S=size_of::<SelectedType<'_>>() on actual target, Q=0 if K=0 else next_power_of_two(K).
Exact successful minima: bytes=B, records=2P+sum(c_i), nodes=1+K,
depth=1 if K=0 else2, owned_bytes=Q*S,
work=1+2(B+2P)+sum(1+b_i+2c_i)+Q*S.
Both parent scans and every child reread are charged; bytes ceiling applies once
to supplied input, not cumulatively. Logical vector growth1,1,2,4,... charges
each requested slot exactly once under checked helper arithmetic. S stays symbolic
until admitted actual-target Rust tests. Requested storage excludes allocator
surplus/churn, stack and RSS. Classfile/annotations/header-d1/payload/flat-Class/
resolver are separate operations/budgets; no whole-pipeline bound or performance
evidence. Hard caps: bytes/owned16MiB; records/nodes65535; work64Mi; depth2.
Callers tighten only, including usize::MAX. No claim every hard ceiling is
independently reachable before another ceiling. Work is accounting, not time.

## Pins, genuine coverage and future validation

Held Kotlin v2.3.20 commitd57eb4a26ed01ab03cf195da686c8047032ff16f;
metadata.proto blob18cfd13e9e512c491f2f62ec241f49c81d6e5e81, SHA256
a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2.
Property lines358..427: optional Type return_type=3, receiver_type=5;
required name=2, optional IDs9/10. ProtoBuf.java blobfc424d5e7aac8661deb862c8e791d4fcf779c194,
SHA256da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716.
Patched CodedInputStream.java SHA2567fbb3aeebd3c3d5414d7171bc3ba2785de50b40863a4f381c4b43986e15784fe;
protobuf2.6.1 commitbba83652e1be610bdb7ee1566ad18346d98b843c remains inherited wire policy.
Actual held copies are hashed as data, no download/research restart/runtime.

Reuse unchanged licensed metadata_constructor/Pair.class.bin (2983 bytes): two
Property bodies d1[234..257] and [259..282], each field3 Type at parent[4..6],
bytes38 00 /38 01 => type_parameter Some(0)/Some(1), all other Type Options None.
Parent names5/6, flagsSome(131590), other parent flagsNone. Genuine receiver
coverage ABSENT in these two Properties; authored receiver positives are labelled.
Compiler provenance UNVERIFIED; distribution version is not compiler proof.
PROVENANCE.json/data_check_output.json independently bind actual class CP89/d1,
held archive entry (when available), schema and notices. Preserve all notices.

Future genuine integration test MUST use actual Rust parse_class -> annotation
Metadata header/decode d1 -> typed payload/table -> flat Class Property selection ->
new property_types decoder. Check k1/mv[2,3,0], actual d1 bytes/hash/selection,
BOTH complete parent/child tuples, exact spans/order/field identity and borrowed
pointers relative to the actual selected Property slices. Literal tests cannot
substitute for this pipeline; no pipeline executed in preparation. Actual class
and d1 upstream storage remain separately budgeted. No resolver/name claim needed.

EXPECTATIONS.json + EXPECTATIONS.md freeze independent literals/full tuples,
prefix partitions, exact minima/one-less and error sequences before source.
Data check does only held-byte transcription/hash/slicing/arithmetic, no product
decoder, protobuf/class parser, fixture encoder, Rust/JVM/compiler/oracle/replay.
No new framework. Source/test/fixture/spec reach is exact in SOURCE_ALLOWLIST.json.
Existing Class/Package member-name and standalone Property/Type/TypeArgument users
remain regression controls; new integration tests are sole new API consumers.
Cargo/dependencies/lock/config/workflows/vendor/parser/server/lib/public-status
and every existing helper/test/fixture remain READONLY.

## Gates and honest status

Retain SAME architect/correctness/adversarial reviewers for preparation and later
admitted source/fixes/completed original CI. Preparation approval admits nothing
else. Future source waits ClassSupertype owner+root delivery/release AND exact
root grant; mod.rs export serialized after239 then FunctionTypes22 then
ClassSupertype25. Docs243 six public paths, vendor241->NullableTwo, LNT binarylib
and W3 auxiliaryworkflow remain other owners'. No source ownership inferred.
Later root-admitted pinned fmt, locked all-target Clippy -D warnings and original
locked workspace hosted tests remain required; no local native runtime under#96.
Push only own branch, create/attach PR, require passing originalCI and SAME3,
exclusive root expected-base/head lease, assertions-enabled standalone read-only
guard actual exit0/full matching COMPLETE PASS then SEPARATE ordinary expected-head
merge. Hold through owner+root original exact-main proof/preservation/release.
No bypass/admin/force-main/settings/output tuning/reruns; preserve first failures.
T008/T008a HOLD; broaderC13/B3/B4 GAP; fullB11 UNVERIFIED; #96/Darwin UNKNOWN;
#18/#68 remain open; full official Kotlin LSP parity is not achieved.

# Current source stage — admitted, runtime validation pending

Coordinator admitted revision2 source/API/compile-only/publication/original Linux
CI on assigned fa26a992a54f5a7c81dc34fe1fc51ba3253e8e54, tree
3e597c898389aead6688c79157296de770d24dcc, soleparent0a0af. Integrated this exact
base once, preserving incoming Type.Argument/Constructor/helper exports. Reader,
frozen tests and additive module export now exist. Local pinned formatting and
locked workspace all-target Clippy passed; no local runtime/tests ran. Actual
SAME3 source and completed original Linux CI reviews remain required. No merge
lease is granted. The original preimplementation contract below is preserved as
historical acceptance; its stage restrictions describe preparation before this
explicit source grant. Existing helper/Cargo/lock/workflow/server bytes unchanged.

# T018 selected JVM method-signature extension — CONTRACT/TEMPDATA

Frozen source base 0a0af012d7a5a258357b5a53407a31b2359bc09f (Constructor PR206),
managed worktree /Users/anil/.codex/worktrees/t018-jvm-method-signature/ktlsp,
branch codex/t018-jvm-method-signature-contract. Initial tracked/untracked status
clean. No product source, test implementation, runtime, publication, push or CI.
Root must explicitly grant source and later serialized metadata/mod.rs export.
This contract consumes ONLY caller-selected Constructor/Function extension100.

## Exact API and reach

New metadata::jvm_method_signature module; one decode accepts EITHER selected
unframed Constructor OR Function body. No parent-kind selector is needed: both
schemas use the identical extension100/wire2 message. Caller independently
selects supported header mv=[2,3,0], k/container, and parent slice; bytes cannot
enforce any of those. Do not call public Constructor/Function decoders internally
(they start separate budgets). Parent name/flags/parameters are outside this API;
not even required Function.name is enforced. Existing raw parent APIs unchanged.
No production caller, migration, name lookup, omitted-value inference, Kotlin JVM
lowering, descriptor parsing, constructor identity, linkage, overload/type checks,
server providers, full initialization or parity claim.

```rust
pub use super::wire::{Error, ErrorKind};
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
    pub records: usize,
    pub work: usize,
    pub owned_bytes: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub struct SignatureBody<'a> {
    pub raw: &'a [u8],
    pub span: std::ops::Range<usize>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct JvmMethodSignature {
    pub name: Option<i32>,
    pub desc: Option<i32>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct MethodSignatureExtension<'a> {
    pub raw: &'a [u8],
    pub signature: Option<JvmMethodSignature>,
    pub bodies: Vec<SignatureBody<'a>>,
}
pub fn decode(bytes: &[u8], limits: Limits)
    -> Result<MethodSignatureExtension<'_>, Error>;
```

Limits Default mirrors Constructor:16MiB bytes,65535 records,64Mi work,16MiB
logical owned bytes; hard clamped, callers tighten only. No other public method,
error kind, abstraction, dependency or shared helper changes. All errors are
wire::Error, offset relative to entire supplied PARENT. All spans are body-only,
parent-relative, computed from validated field.end minus borrowed raw.len(), never
trusted caller spans. Complete raw preserves unknown/wrong-wire fields, duplicate
occurrences and cross-field order. bodies preserves every recognized occurrence
including empty bodies, in encounter order, pointer-identical into raw.
Public caller-created structs have no validated raw/typed-equivalence guarantee.

Exclusive future allowlist:
- crates/ktlsp-binary/src/metadata/jvm_method_signature.rs (NEW)
- crates/ktlsp-binary/tests/metadata_jvm_method_signature.rs (NEW)
- crates/ktlsp-binary/tests/fixtures/metadata_jvm_method_signature/README.md (NEW)
- crates/ktlsp-binary/tests/fixtures/metadata_jvm_method_signature/EXPECTATIONS.md (NEW)
- crates/ktlsp-binary/tests/fixtures/metadata_jvm_method_signature/PAIR_SIGNATURE_EXTRACTION.json (NEW)
- crates/ktlsp-binary/tests/fixtures/metadata_jvm_method_signature/selected.hex (NEW)
- crates/ktlsp-binary/tests/fixtures/metadata_jvm_method_signature/merged.hex (NEW)
- crates/ktlsp-binary/tests/fixtures/metadata_jvm_method_signature/pair_parent.hex (NEW)
- crates/ktlsp-binary/tests/fixtures/metadata_jvm_method_signature/LICENSE.txt (NEW)
- crates/ktlsp-binary/tests/fixtures/metadata_jvm_method_signature/NOTICE.txt (NEW)
- crates/ktlsp-binary/tests/fixtures/metadata_jvm_method_signature/PROTOBUF_LICENSE.txt (NEW)
- crates/ktlsp-binary/METADATA_JVM_METHOD_SIGNATURE_SPEC.md (NEW)
- crates/ktlsp-binary/METADATA_JVM_METHOD_SIGNATURE_REVIEW.md (NEW)
Only future explicitly root-serialized shared change: additive
`pub mod jvm_method_signature;` in metadata/mod.rs. Every existing helper,
Constructor/Function/name-resolver/classfile source, Cargo/lock/workflow/vendor/
parser/server/public-status/config/export otherwise READONLY. Existing licensed
Pair fixture reused via include_bytes; do not duplicate its class data.

## Pinned source and merge behavior

Existing Kotlin v2.3.20 source pin d57eb4a26ed01ab03cf195da686c8047032ff16f:
core/metadata.jvm/src/jvm_metadata.proto SHA256
5a80e63021b800afd2297b3713c4cfc12739cbc8e62c726578e1c5d1dfc771f9;
core/metadata.jvm/src/org/jetbrains/kotlin/metadata/jvm/JvmProtoBuf.java SHA256
b1ad864ae872315c06740d6551d2d7233060f5bff3e9ad9b8f80f531018bc3a1.
Local unchanged copies from prior StringTableTypes evidence are reference DATA.
JvmMethodSignature fields1(name)/2(desc) optional int32; generated parser tags8/16
set scalar presence and readInt32. Init numeric defaults0 do not set presence;
absence remains None (no automatic index0 lookup). ExplicitSome0 distinct.
Generated builder mergeFrom2433..2445 only overwrites when other.hasName/hasDesc.
Extension declarations3957..3988 are singular MESSAGE100 on Constructor/Function.
Protobuf2.6.1 pin bba83652e1be610bdb7ee1566ad18346d98b843c,
java/src/main/java/com/google/protobuf/GeneratedMessageLite.java SHA256
14c63c0c074008f24dcacf101ee09b19691bfaacbfc7422b7ddf9c2aa9bc964f,
parseUnknownField573..592 starts existingValue.toBuilder before readMessage;
617..619 stores the merged singular value. This independently confirms cross-
occurrence merging, rather than silently last-winning the whole child message.
Existing Kotlin relocation/patched-runtime limits remain; no universal JVM claim.

Recognize parent(100,wire2) only. For each recognized body, visit its whole wire
in parent coordinates; recognize child(1,wire0)->name and(2,wire0)->desc. Convert
u64 as u32 as i32 (signed low32, not zigzag). Recognized scalar last wins BOTH
within and across bodies; missing/wrong-wire fields NEVER clear previous values.
No signature enum exists. Child field3 varint99 is unknown data, not enum parsing.
Parent enum/unselected data remains raw/opaque. All unknown/wrong-wire fields stay
exactly in raw and occurrence.raw; no unknown vector/rewrite/normalization.
Only recognized extension100 bodies recurse one flat level. Other bytes, including
parent Type/ValueParameter payloads and child wrong-wire scalar byte payloads,
remain opaque even when containing malformed nested protobuf.

Absent selected extension =>signatureNone,bodiesempty. Present-empty =>Some with
nameNone/descNone and one empty body. An empty later occurrence cannot erase any
prior value. Wrong-wire100 never marks presence. Unknown extensions101 etc do not
mark presence. Valid bytes after a signature MUST be scanned; malformed later
parent/selected-body data aborts the entire call, no partial result. Parent framing
of a selected field completes BEFORE child validation. Child validation completes
BEFORE appending that occurrence via Budget::push; scalar mutations are internal
and never observable on error. Finish each selected child then resume parent;
first encountered structural/resource error wins, never scan past it to prefer a
later error. Success validates parent wire plus selected-child wire ONLY.

## One aggregate parent + selected-child budget

Use existing unchanged string_table::Budget::new with bytes/records/work/owned
mapped from Limits and other fields Default; never call node/integers. One Budget
spans full parent and EVERY selected body. wire::field(...,&mut budget.wire) then
wire.work(1,tag_start) BEFORE dispatch for every completed parent and child field,
including unknown/wrong-wire. Child cursors use original bytes and exact checked
body start/end, so following bytes cannot rescue truncated child input.
No per-body reset/refund/root/node/depth/extra conversion charge. Scalar assignment
and signature presence bookkeeping are unmetered; scan nesting is exactly one.
Parent wire already charges selected body bytes once; child visits charge them
AGAIN. Append SignatureBody only after full body scan using budget.push, with
parent extension tag_start as its capacity/error offset. Do not reserve from a
declared length, collect all wire fields, concatenate child bodies or copy raw.

Let N=parent bytes,F=parent fields,G=sum selected child fields,L=sum selected body
lengths,B=recognized extension occurrences,S=size_of::<SignatureBody>(),
C(B)=0 ifB0 else next_power_of_two(B). Complete success requires recordsF+G,
workN+L+2(F+G)+C(B)S,ownedC(B)S. No logical extra heap for merged signature/raw.
Capacity growth at len0/powers2: slots=max(len,1), checked len+slots,
checked slots*S, aggregate owned subtraction, work(slots*S), try_reserve_exact,
then append; allocator surplus changes no logical charge. Aggregate record/work
caps apply even to empty and overwritten bodies. No whole-composition/RSS/CPU/
walltime/churn/allocator-surplus/stack bound; name resolution and class/header/
d1/table/parent-selection operations remain separately budgeted. No hidden claim
that their separate limits make one end-to-end bound.

Precedence: input length gate Limit(bytes)@0 first; each field attempt record,
record-work, wire reads/structural validation, dispatch; selected-body scan,
then capacity checked arithmetic/owned, capacity-work, allocation, then parent
continuation. No early success. Empty bytes with all0 succeeds absent. Opaque
parent fields incur no allocation. Wire rules unchanged: tags/lengthslow32,
varintslow64, accepted nonminimal<=10bytes, field0 invalid before wire check,
groups3/4 unsupported,wires6/7 invalid; signedlow32 lengths<0 NegativeLength at
length start; tenth continuation MalformedVarint at tenth byte. Failed single
read charges1 then Truncated@end; failed bulk read charges available+1 at bulk
start before Truncated@end. All requested allocation failures return Limit
(allocation) at extension tag; dynamic allocator failure remains static-review
only. Literal expectations in EXPECTATIONS.md freeze exact cuts and thresholds.

## Genuine existing fixture, intended Rust composition

Reuse unchanged metadata_constructor/Pair.class.bin2983bytes SHA256
8fed0fa88eb2df5723812a95330dee6bc91fd66b13906acb7906f9a813da9736,
org.jetbrains.kotlin:kotlin-stdlib:2.3.20 kotlin/Pair.class. Provenance and retained
Apache2.0 LICENSE/NOTICE/protobuf license come from Constructor fixture and
existing t017c2. JAR SHA2560ae12504a5040ebaf37703908483420d1a5624dd1d93f357665f8c77c848a01e.
Compiler provenance UNVERIFIED; distribution2.3.20-release-208 is no compiler proof.

Data-only independent CP/annotation transcription confirms mv[2,3,0],k1,d1
SHA256a4c2bf2d0bcd52aac574d156b413e3926cec249c5c2a23259d126c68872afa8e,
Class d1[47..285],Constructor field[76..103],parent body[78..103],extension100
field[96..103],signature body[99..103]=08 07 10 08. Parent body spans signature
[21..25]. Selected signature nameSome7/descSome8, one body. Exact selected-call
limits bytes25/records6/work41+S/ownedS. Existing Constructor separately decodes
flagsSome70 and parameters unchanged; don't edit/reinterpret its raw extension.

String table d1[1..47], existing typed decoder then delivered
name_resolver::Resolver::new(table,d2,limits).get_string(7/8) must return exact UTF16
`<init>` and `(Ljava/lang/Object;Ljava/lang/Object;)V`. Compact Record ordinal4,
range9 covers indices5..13; it has no transform/predefined/explicit string, so
indices7/8 select corresponding unchanged d2. Assert actual delivered resolution
in future Rust test (not yet executed here). Class method_info[1925..2036] has
name_index8,descriptor_index9, whose encoded CP UTF8 content spans[163..169] and
[172..211] equal those strings. Future Rust test uses ktlsp_binary::parse_class/ClassFile::utf16 and
actual Member.name/descriptor refs, finds exactly one matching tuple and compares
literal UTF16; it does not manufacture a Member, rely on known constructor naming,
or infer descriptor from metadata types. This is encoded-byte equality evidence,
not constructor symbol identity, linkage, source parameter mapping or full parity.

Pipeline test: ktlsp_binary::parse_class -> annotations::decode -> metadata::decode ->
payload::decode typed table/opaque Class -> wire::decode(&decoded_d1, ...) bounded flat Class
selection -> selected
extension decode -> actual delivered resolver -> encoded classfile Member UTF16.
Use existing APIs unchanged with SEPARATE other-operation budgets. No new JVM,
compiler, Gradle, Maven, oracle or artifact survey. Data fixture check succeeded;
Rust composition remains unexecuted until explicitly admitted hosted CI.

## Reviews, publication and implementation gates

SAME3 retained: /root/architect,/root/correctness,/root/adversarial. Actual reviews
and fixes/rechecks must be recorded in local REVIEW before READY. Unposted child
issue draft only; query for duplicate signatures returned broad parent/unrelated
closed infrastructure, no selected extension child (read-only2026-10-14).
No source/export/issue/push/PR/CI until explicit root grant. Later workflow requires
source review, compile-only fmt + cargo clippy --locked --workspace --all-targets
-- -D warnings; original unchanged Linux CI fmt/Clippy/cargo test --locked
--workspace; SAME3 final source + actual completed CI review; root exclusive
expected-head/base lease; separate fresh read-only EXIT0 complete guard before
separate ordinary expected-head merge. Any malformed/nonzero/timeout/mismatch
suspends lease. No admin/bypass/force/main/settings/unchanged rerun-to-green.
Owner+root verify actual squash tree/sole parent and exact-main original CI.

Parents#18/#68 OPEN; T008/T008a HOLD; broaderC13/B3/B4 GAP; fullB11 UNVERIFIED;
#7/#83/#96 OPEN, Darwin UNKNOWN. Full Kotlin metadata/semantics/LSP parity pending.

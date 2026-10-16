# Current contract: remaining legal method targets

This admitted contract supersedes the historical method-return/formal-parameter contracts below.

# T017 remaining legal method target forms — contract v1

Root admitted exact source/API/publication/original hosted CI on 2026-10-14. Local format and compile-only Clippy only; local native runtime remains prohibited. Exact base cff36d17cd5cd1386059ecc91572013fdb417b48, tree4e432a34ecd8c0eb27f563ec6f0dc483202e2472. Owned managed worktree /Users/anil/.codex/worktrees/t017-method-targets/ktlsp; branch codex/t017-remaining-method-targets. Root reserved architect and separate correctness/adversarial slots. Same three review final source and completed original CI.

## Scope and fixed format

Pinned source: JVMS SE23 §§4.7.20–21, tables4.7.20-A/C, §4.7.20.1:
https://docs.oracle.com/javase/specs/jvms/se23/html/jvms-4.html#jvms-4.7.20
Complete the four remaining legal method_info target forms together. Successful target tags are exactly01,12,14,15,16,17 hexadecimal; the other250 u1 values fail Malformed("method type annotation target") immediately after tag. No method legal UnsupportedTag remains. Field0x13 and class/Code/Record dispositions remain inherited. Both visibility names, ordering, duplicates and present-empty distinction preserved.

| tag | target_info widths | retained value |
|---|---|---|
|01|u1|TypeParameter(raw index)|
|12|u1,u1|TypeParameterBound{type_parameter_index,bound_index}|
|14|empty|Empty|
|15|empty|Empty|
|16|u1|FormalParameter(raw index)|
|17|u2 big endian|Throws(raw index)|

Indices are encoded data only. No Signature, descriptor, type-parameter/bound, source parameter, JVM slot, receiver/static/special-method, Exceptions table range/existence, bytecode, linkage, applicability or Kotlin semantics validation. Forged ClassFile controls with nonsense or absent Signature/Exceptions remain accepted where earlier extension decoding does not consume them. An opaque malformed Exceptions attr remains opaque. Recognized declaration/parameter/default extension failures still precede method type decoding. New functionality has no server consumer or advertised capability.

## Exact public API decision and compatibility

Keep MethodTypeAnnotation.target_type:u8. REMOVE formal_parameter_index:Option<u8>; replace it with target_info:MethodTargetInfo. In existing method_type_annotations module, add only:

```rust
#[derive(Debug, PartialEq, Eq)]
pub enum MethodTargetInfo {
    Empty,
    TypeParameter(u8),
    TypeParameterBound { type_parameter_index: u8, bound_index: u8 },
    FormalParameter(u8),
    Throws(u16),
}
```

All other public fields/function signatures unchanged. Successful target_type/info pair mapping is the table above. Empty deliberately represents the empty union body for both14/15, distinguished by raw tag. No compatibility mirror, accessors, generic framework, dependency or separate module/export. Root confirmed preserving old field access is not mandatory. All three contract reviews and root admission are recorded above; final source and CI reviews remain required. BREAKING: external field reads, literals and exhaustive patterns referencing formal_parameter_index stop compiling; derived Debug/layout/size changes. All repo callers are confined to tests/method_type_annotations.rs (rg at base). Public method spec and crate README updated; root status docs readonly. Storage equations use size_of UPDATED MethodTypeAnnotation; no hardcoded platform byte size.

## Ordered parsing, budget, error positions

Reuse existing Budget/Reader/vector/annotation without changes. One aggregate budget created by public decode; declaration/extensions/field pass first, then method pass. Byte cap16MiB, node cap65535 and work cap67108864 unchanged. No reset per root/attribute/method; no new allocations for fixed info enum. Existing four method vectors reserve requested slots via vector, requested node admission before allocation and observed extra capacity debit before retention. Inherited spare capacity/allocator/RSS limits are not strengthened.

Target work1 before target read. For01/16 work1 before index read. For12 work1 before first u1 read THEN work1 before second u1 read (not a combined debit before first read). For17 work2 before Reader.u16 (atomic take2, existing u16 behavior). Empty14/15 no info work. Then existing path length read, work(1+2L), path reservation, full kind/index reads before path validation, annotation root depth1, exact attribute exhaustion, lazy wrapper reservation2. Unknown/illegal tags do not read info/path.

New info work equals encoded info width; other accounting identical. For private decode_methods with one visible attr and one root: attr name text work30, target work1, info widthw, path length work1, descriptor LA; work4; total36+w for minimal empty annotation. Full public decode adds two preceding scans of visible name =>96+w. Node count for one-method one-attr root N:5+N+O; independent of target width. Existing total storage/capacity equation unchanged except UPDATED root size.

Error has no public offsets. Positions below refer only to byte-relative evidence. Minimal one-root payload: count at0..2, tag at2, info at3..3+w, path length at3+w, descriptor at4+w..6+w, pair count at6+w..8+w; total8+w. Every proper prefix of each minimal valid payload is Truncated under fresh budget, both visibility names. For17 one available info byte still Truncated via atomic u16. A path kind without argument is Truncated even if kind>3. First error wins in root/attribute/member input order; no pre-scan, partial successful result or mutation of original ClassFile.

Exact private budget controls freeze BEFORE implementation:
- 01/16 at bytes[00,01,tag]: remaining31 => Limit(work);32 => Truncated. bytes plus index, remaining32 => Truncated(path length); plus path0 => Limit(work).
- 12 at [00,01,12]: remaining31 => Limit(work);32 => Truncated(first index). At [00,01,12,ff]: remaining32 => Limit(work);33 => Truncated(second index). At both indices remaining33 => Truncated(path length); plus path0 => Limit(work).
- 17 at [00,01,17] or [00,01,17,ff]: remaining32 => Limit(work);33 => Truncated(info). Both info bytes remaining33 => Truncated(path length); plus path0 => Limit(work).
- 15 at [00,01,15]: remaining31 => Truncated(path length); plus path0 => Limit(work).
Work32/33 above includes attr30+target1+reached fixed bytes. Private controls test helper directly with preloaded Budget, no public budget API.

Public exact-boundary controls use340 ignored method attrs with UTF16 name length65535 and one ignored filler name length87345 (three scans) plus visible root with path[(0,0),(1,0)], elementx string at CP7. Existing formal test total340*3*65536 +3*87346+104+string_length. Base340*3*65536+3*87346+103 =67108861; therefore total67108861+w+string_length. Choose string lengths(2-w,3-w,4-w) for limit-1/exact/+1: w0=>2,3,4;w1=>1,2,3;w2=>0,1,2. Field-only endpoint still succeeds all. Node boundariesN65529/65530/65531 for each new target retain overshoot-conditional first two and unconditional +1 failure. Raw byte boundary ignored class payloadMAX_BYTES-(8+w)+delta, delta=-1/0/+1. Whole combined mixed declaration/default/field/method accounting existing tests retained, include all six mixed roots sharing one call.

## Frozen literal acceptance vectors

CP fixture follows existing base:1visible,2invisible,3LA;,4x,5Integer80000001,7UTF16[NUL,D800]. Authored literals (not production generated) stored lines name|hex:

```
type_parameter0|00 01 01 00 00 00 03 00 00
type_parameter255|00 01 01 ff 00 00 03 00 00
bound0|00 01 12 00 00 00 00 03 00 00
bound255|00 01 12 ff ff 00 00 03 00 00
receiver|00 01 15 00 00 03 00 00
throws0|00 01 17 00 00 00 00 03 00 00
throws65535|00 01 17 ff ff 00 00 03 00 00
throws258|00 01 17 01 02 00 00 03 00 00
all_steps|00 01 12 fe fd 04 00 00 01 00 02 00 03 ff 00 03 00 00
invalid_kind_missing_index|00 01 01 ff 01 04
invalid_kind|00 01 01 ff 01 04 00
invalid_argument|00 01 17 ff ff 01 00 01
zero_descriptor|00 01 15 00 00 00 00 00
wrong_cp|00 01 12 00 01 00 00 05 00 00
trailing|00 01 15 00 00 03 00 00 00
illegal_field|00 01 13
unknown|00 01 ff
count_eof|ff ff
```

First9 successes preserve exact target-info/path/descriptor and empty elements. Remaining outcomes respectivelyTruncated,Malformed(path),Malformed(path),Malformed(annotation constant index),Malformed(UTF8 index),Malformed(trailing bytes),Malformed(method target),Malformed(method target),Limit(annotation nodes). Repeated six-target mixed roots preserve input order; duplicate descriptor roots accepted. Mixed valid+malformed in both orders errors; malformed-before-later tag wins, illegal-tag-before-truncation wins. All256 tags tested with fresh minimal width-appropriate bytes; six successes and250 exact malformed. Raw indices controls01/16=0,1,255;12=(0,0),(0,255),(255,0),(255,255);17=0,1,258,65535 with absent/opaque invalid Signature/Exceptions and invalid method name/descriptor/flags. Both visibilities, duplicate attrs same name via distinct CP index, different visibility ordering, absent/empty methods, prefix exhaustion, path255, shared all values/depth64/65, per-byte finite mutation no crash; reuse existing controls without duplicate framework.

Earlier unsupported* fixture files retained as historical literal inputs; current tests change expected remaining01/12/15/17 errors to Truncated because those payloads end before required remaining fields. Formal unsupported_receiver and ordering payloads similarly nowTruncated; earlier malformed descriptor precedence remains. Do not rewrite bytes to make tests pass.

## Genuine fixture bounded policy

Exactly one already primary-pinned Guava33.4.8-android archive (SHAa1f8a4bafe5a77232d333356843e194599b30837574a7f4be1063daaaaab102c,size2936569) independently inspected as auxiliary Python data, no new network/compiler/native/JVM. Scan1932 class entries, method targets counts01=0,12=969,14=1021,15=0,16=1479,17=0. This bounded artifact contains genuine12 only among newly added forms;01/15/17 coverage explicitly authored synthetic, no ecosystem absence claim or broad hunt.

Select minimum(class size, archive entry path) among176 newly-supported candidates:com/google/common/base/NullnessCasts.class,size835,SHAefdd5084585676be44665f177c6fa6bb315ff5e57417e7ccf56db7a5da5a0b8d,major52/minor0,CP30,fields0,methods2. Method0uncheckedCastNullableTToT descriptor(Ljava/lang/Object;)Ljava/lang/Object;,flags8. Visible attr nameCP23,header729,payload735,length17,exact0002120000000018000016000000180000. Two roots:payload offsets2 tag12 info0000 empty path CP24/no pairs;offset10 tag16 info00 empty path CP24/no pairs. CP24Lorg/jspecify/annotations/Nullable;. Method1 empty output. Retained existing Guava Apache2 license and factual new NOTICE, original upstream source header unavailable/uninspected. Primary byte equality pin does not establish authentication or compiler version. Existing genuine14/16 tests remain.

## Exact proposed owned source path list (11 files)

1. crates/ktlsp-binary/src/method_type_annotations.rs
2. crates/ktlsp-binary/tests/method_type_annotations.rs
3. crates/ktlsp-binary/METHOD_TYPE_ANNOTATIONS_SPEC.md
4. crates/ktlsp-binary/METHOD_TYPE_ANNOTATIONS_REVIEW.md
5. crates/ktlsp-binary/README.md
6. crates/ktlsp-binary/tests/fixtures/method_type_annotations/remaining/README.md
7. crates/ktlsp-binary/tests/fixtures/method_type_annotations/remaining/authored.hex
8. crates/ktlsp-binary/tests/fixtures/method_type_annotations/remaining/NullnessCasts.class.bin
9. crates/ktlsp-binary/tests/fixtures/method_type_annotations/remaining/provenance.json
10. crates/ktlsp-binary/tests/fixtures/method_type_annotations/remaining/NOTICE.txt
11. crates/ktlsp-binary/tests/fixtures/method_type_annotations/formal_parameter/README.md (narrow API output-name correction only)

Reuse license by relative link to existing ../LICENSE.txt; no copied bytes needing another license file. No lib.rs or sharedhelper/export/module slot requested. No Cargo/lock/workflow/dependency/status/rootdoc edits.

## Delivery gates

SAME3contract approvals, root exactsource/API/path admission; source implementation/review/recheck; root-admittedfmt and compile-onlyClippy only locally. NO local native runtime/tests/parser/worker/bench under#96. Hosted original unchanged LinuxNativeRust workflow must run cargo fmt --all -- --check;cargo clippy --locked --workspace --all-targets -- -D warnings;cargo test --locked --workspace. Record first failures, no unchanged retry-to-green. SAME3approve actual finalsource andcompletedoriginalCI; root stableintegrationcut then exclusive expectedhead/base mergelease. Issue/PR with bodyfile, attached PR, ordinary conditional squash, independently verify mergedtree==reviewedtree,soleparent==admittedbase,original exactmain attempt1 all3/rawcounts/workflow/executor. No semantic/fullparity/RSS/Darwinfix claims. No cleanup untilrootgrant.


---

# Historical contracts (preserved)

# T017c3c: bounded method type annotation data

Current finite source contract v3, 2026-10-13. Root admitted the target0x16
source/API delta after v1/v2 auxiliary reviews; integration/PR/CI/merge remain held.
Original method-return contract provenance below remains historical evidence.
Child proposal under #71/#17; parents remain OPEN. Frozen initial source
8548ead597146a4ca52d3a4a2ec6ffd724fac7a3, tree
3f98c1ec916437445fb6e499a85caa04c5959e63, inspected at clean detached private
/private/tmp/ktlsp-method-type-source. Root must assign the eventual integration
base and explicitly grant shared edits and publication; no moving-main chase.

## Format and disposition

Same pinned JVMS Java SE23 §§4.7,4.7.16.1,4.7.20–21,4.7.20.1–2, tables4.7-A/C,
4.7.20-A/C,4.7.20.2-A as the delivered field contract:
https://docs.oracle.com/javase/specs/jvms/se23/html/jvms-4.html#jvms-4.7.20.
This is a METHOD type annotation API; supported encoded targets are
METHOD_RETURN0x14 (empty target_info) and METHOD_FORMAL_PARAMETER0x16
(one u1 encoded formal_parameter_index). It covers both
RuntimeVisibleTypeAnnotations and RuntimeInvisibleTypeAnnotations at method_info.

Every legal method target has an explicit disposition:

| Target | Meaning | Initial outcome |
| --- | --- | --- |
| 0x01 | method/constructor type parameter | Error::UnsupportedTag(0x01) |
| 0x12 | method/constructor type parameter bound | Error::UnsupportedTag(0x12) |
| 0x14 | method return/newly constructed object type | decode empty target_info, path and annotation |
| 0x15 | method/constructor receiver | Error::UnsupportedTag(0x15) |
| 0x16 | method/constructor formal parameter | retain u1 encoded ordinal, decode path/annotation |
| 0x17 | method/constructor throws type | Error::UnsupportedTag(0x17) |

Every other u1 value (250 values) in a METHOD attribute yields
Error::Malformed("method type annotation target"). In particular class0x00/10/11,
field/Record0x13, Code0x40–4b and unassigned tags are illegal METHOD contexts.
UnsupportedTag is the existing Error variant, documented at this API as a legal
unimplemented METHOD target. No global Error changes. On unsupported target,
return immediately after reading its tag; its target_info, path, annotation and
remaining bytes are unconsumed/unvalidated, including missing/invalid indices.
An attribute containing supported+unsupported entries cannot return partial
success. The WHOLE decode call errors and drops accumulated output; original
ClassFile and its raw Attribute bytes remain available, unchanged. A malformed
supported entry encountered earlier wins over a later unsupported target. An
unsupported tag encountered first wins over missing/invalid later bytes. No
pre-scan/reordering or silent target skip.

Existing declarations accept only45.0–45.3 or46..69 minor0 and reject all other
major/minor first with UnsupportedVersion{major,minor}. A recognized FIELD or
METHOD type attr requires major52..69 minor0, even count0/truncated attr: before
its payload read, pre52 returns Malformed("type annotation version"). Without
recognized field/method type attrs, earlier supported versions retain inherited
acceptance. Class type attrs remain opaque even pre52 or duplicate/malformed;
Code/Record interiors are opaque. Field0x13 remains decoded with its original
wrong-target error. A nonmethod0x14 attr is NOT decoded as a method annotation.

Type path u1 length0..255; ordered steps read BOTH u1 kind and argument_index
before checking. Kinds0/1/2 require index0; kind3 retains any u1 index; otherwise
Malformed("type annotation path"). Missing either byte is Truncated even if the
available kind is invalid. Paths are data; no descriptor/Signature applicability,
dimensions, type argument/bound upper bounds, receiver, bytecode or linkage claims.

At most one attr per visibility per method, comparing UTF16 name content (not CP
index). Duplicate visibility returns
Error::Malformed("duplicate type annotation attribute") before the second payload
read, matching the existing field label. Both may coexist in either order. Preserve all attr/root/path/element
ordering and repeated descriptors/paths; absent differs from present count0.
Repeated identical root records are accepted; duplicate ELEMENT names remain the
inherited error. Method output includes an empty entry for every absent method.
All shared annotation Value variants, exact numeric bits and borrowed UTF16
including NUL/surrogates are retained through annotations::annotation(...,1).
Depth and descriptor/name/element checks remain exactly inherited.

Method name/descriptor/access_flags are NOT consumed by the new method pass,
including count0, supported0x14/0x16 and unsupported tags. A forged ClassFile may
therefore succeed with name0/descriptor0 or malformed/void/nonvoid descriptors;
this is encoded data, not Java language legality. Hand-authored controls cover
()V,()I,()Ljava/lang/Object;, <init>()V and <clinit>()V with the SAME accepted0x14
payload, and name/descriptor indices0/out-of-range. This does not assert all such
source annotations are legal. Constructor0x14 denotes the newly constructed
object, not a semantic void result. Parsed ClassFile already underwent structural
reader checks, which are a separate caller operation.

The WHOLE combined API still runs existing extensions first. A recognized
parameter/default extension consumes and validates method name/descriptor and
its own special-method/default shape before any method type decode; its failure
wins. This conditional inherited consumption is unchanged. Attribute names are
consumed during declarations, extensions, and their field OR method type pass;
annotation descriptors/elements/values are consumed only where reached. Unrelated
CP entries, names/descriptors and class structure remain unverified; no broad
forged ClassFile revalidation. CP0/wide holes/wrong types/out-of-range checks are
exact inherited ones at consumed references.

## API, exact shared patch and reach

New module crates/ktlsp-binary/src/method_type_annotations.rs uses existing
annotations::{Annotation,Visibility,Budget}, type_annotations::{TypePathStep,
FieldTypeAnnotations} and Error/ClassFile. All new result structs derive
Debug/PartialEq/Eq, with the following exact public interface:

```rust
pub struct MethodTypeAnnotation<'a> {
    /// Encoded target_type; successful decoding emits 0x14 or 0x16.
    pub target_type: u8,
    pub formal_parameter_index: Option<u8>,
    pub path: Vec<type_annotations::TypePathStep>,
    pub annotation: annotations::Annotation<'a>,
}
pub struct MethodTypeAnnotationAttribute<'a> {
    pub visibility: annotations::Visibility,
    pub annotations: Vec<MethodTypeAnnotation<'a>>,
}
pub struct MethodTypeAnnotations<'a> {
    pub field_types: type_annotations::FieldTypeAnnotations<'a>,
    pub methods: Vec<Vec<MethodTypeAnnotationAttribute<'a>>>,
}
pub fn decode<'a>(class: &'a ClassFile<'_>)
    -> Result<MethodTypeAnnotations<'a>, Error>;
```

No permanent return-only wrapper, generic target framework or invented union
layouts. u8 retains the actual supported tag; future targets require a separate
accepted contract. This endpoint creates one Budget, calls new crate-private
field type_annotations::decode_with_budget(class,&mut budget), then private
method decode_methods(class,&mut budget). fields/extensions remain wrapped once;
out.field_types equals existing type_annotations::decode when both succeed.
Methods aligns one-to-one with input methods. Root admits the new public
formal_parameter_index field: source compatibility breaks for external literals
and exhaustive patterns; derived Debug and platform layout/size can change.
Decoded0x14 has None,0x16 has Some(encoded u1); no public Budget change.

Original PR145 shared delta (already delivered, unchanged by this child):
1. lib.rs additive `pub mod method_type_annotations;` only.
2. type_annotations.rs public decode body becomes
   `decode_with_budget(class, &mut Budget::default())`.
3. Mechanically extract its ORIGINAL body excluding local Budget initialization
   into `pub(crate) fn decode_with_budget<'a>(class: &'a ClassFile<'_>,
   budget: &mut Budget) -> Result<FieldTypeAnnotations<'a>>`; use `budget` rather
   than `&mut budget` for the same extension-helper and decode_fields calls.
   Preserve extension call FIRST, decode_fields SECOND, construction and errors.
4. Only visibility of existing `fn vector<T>` changes to `pub(crate) fn vector<T>`.
   Reuse it for all new method-pass Vecs; leave vector/extra_capacity behavior,
   decode_fields body and original unit tests byte-for-byte otherwise unchanged.

No change to annotations.rs/Budget/old Vecs/annotation_extensions.rs/errors,
metadata header/wire/payload/string/name modules, Cargo/lock/dependencies,
parser/vendor/server/project/import/artifacts/workflow or root status docs.
Exclusive current files: existing method module, tests/method_type_annotations.rs,
tests/fixtures/method_type_annotations/**, METHOD_TYPE_ANNOTATIONS_SPEC.md,
METHOD_TYPE_ANNOTATIONS_REVIEW.md. Review package outside repository only now.

Read all existing callers before source: rg found field endpoint only in
binary/tests/type_annotations.rs; shared field helper/vector have no external
callers yet. Existing declaration/extension/metadata tests remain required
unchanged regression users; no shipped server/capability caller. Existing field
endpoint continues accepting opaque METHOD payload[255] and legal-unsupported
method targets while NEW combined endpoint rejects them explicitly. Existing
method results/extensions and field output shape/error order must remain intact.
Only choosing new API adds method errors/work/capacity rejection. No input writes,
data loss, payload filtering or new exposure. New focused tests are first callers.

## Aggregate budgets, reservations and exact precedence

One Budget: declarations → extensions → fields → methods. No reset per attr or
member; same64 depth,16MiB all top-level raw payload bytes,65,535 nodes,64Mi work.
Bytes/F/M/A are charged only in declarations, including ignored payloads; no raw
payload or member/attribute-container double charge by later scans. Existing
Budget and all inherited Vec allocations remain unchanged. Type paths iterative,
not extra recursive depth. Existing annotation depth64 succeeds/depth65 errors
before descent. Bounded nesting/drop remains acceptance, not tested at this stage.

Use B=F+M+A+P+R+E+V+N from existing declarations/extensions, with same definitions
as delivered field spec (#130): F/M members, A raw attrs, P parameter entries,
R declaration/parameter roots, E pairs, V default/pair/array values, N nested@
roots. Total nodes = B+Qf+Cf+Tf+Lf+Of+Qm+Cm+Tm+Lm+Om.
Field terms unchanged. Qm=initial requested outer-method slots M; Cm=2 wrapper
slots per method with any fully decoded retained type attr; Tm=u2 root counts;
Lm=u1 path counts; Om=sum observed NEW method Vec capacity beyond requested.
Roots/paths capacity charge IS their structural charge, not a second debit.
No second method member or raw attr debit. Qm includes every input method even
with no type attrs; wrappers are lazy after exact exhaustion of first type attr.

Every new method Vec uses EXISTING field vector helper: nodes(requested) before
empty Vec::try_reserve_exact; count0 returns Vec::new without allocation;
allocation failure Limit("annotation allocation"); checked observed-requested
underflow same allocation label; observed extra capacity nodes before returning/
retaining Vec, node excess Limit("annotation nodes"). No later push beyond the
requested count; no hidden growth. Outer requestedM, lazy wrapper requested2,
root requestedcount, path requestedlength are the only four new kinds. Shared
private extra_capacity synthetic tests already exercise overshoot/underflow/huge
requested counters; do not duplicate helpers or allocator injection framework.

Precedence AFTER all field decoding succeeds:
- Reserve outer methods Qm before first method/name scan.
- Visit methods and attrs in input order; Budget::text attr.name, even ignored.
- Recognized name: version check, duplicate visibility, u2 count, root Vec debit/
  reserve (and observed-extra debit), then per root work1 BEFORE target u1.
- Target0x16: work1 BEFORE index u1, retain Some(index). Target0x14: None.
  Then either supported target reads path length u1, work(1+2L), path Vec debit/reserve and extra,
  each whole kind/index pair then validate; shared annotation depth1.
- Unsupported legal target: UnsupportedTag(tag) immediately; illegal target:
  Malformed("method type annotation target") immediately. Neither reads path.
- After all roots: exact attr exhaustion BEFORE lazy wrapper reservation.
  Trailing => Malformed("type annotation trailing bytes"); wrapper reserve2
  only after first successful payload exhaustion, then push visibility/root Vec.
- Duplicate check precedes second payload read, but first malformed/unsupported
  attr prevents reaching later duplicate. A failed later attr/target/member
  exposes no partial output. All errors leave raw ClassFile untouched.

Limits can win before EOF: root/path count charged before record reads, target
work before target read; path work before path allocation/step reads. Allocation
failure label precedes future EOF, observed-extra failure precedes use/retention.
Budget normal failed over-limit debit stores the exceeded counter then errors;
checked-add overflow leaves old counter unchanged. Existing Budget is never reused
by public endpoints after an error. No new public counter/error offsets: Error
has no offsets. Prefix positions below are evidence-relative only.

New method work = sum(1+UTF16 attr-name length) across all method attrs, plus
one per target inspected, plus1 for each reached0x16 index BEFORE read, plus1+2L per supported path,
plus shared annotation work.
Field and method names are scanned three times total each, class attrs twice;
there is no fourth scan of either member kind or method descriptor/path pass.
Allocator arithmetic adds no work units. Existing extension shape work unchanged.

Exact independently derived boundaries, before runtime:
1. Method-only input M1/A1/F0, one visible attr with X empty-path/no-pair0x14
   roots: nodes5+X+O, where O actual observed extra capacity from Qm,Cm,Tm.
   X=65,529/65,530/65,531 is limit-1/exact/+1 at O0. Success at first two is
   conditional on observed overshoot; assert successful capacity equation and
   lengths, permit only annotation-node failure if O overshoots. +1 ALWAYS node
   failure. u2countffff with no roots fails at nodes65,538 BEFORE EOF/wrapper.
   Adding one path step to exact input must fail. 32,768 methods no attrs already
   debit65,536 via M+Qm: field endpoint can succeed, new API must node-fail.
2. Mixed: F1/M1/A5 (class declaration1, field type1, method parameter/default/type3),
   P1/R2/E2/V(3+X)/N0 gives B15+X. One field root/path adds Qf1+Cf2+Tf1+Lf1=5;
   one method root/path adds Qm1+Cm2+Tm1+Lm1=5. Total25+X+O. X=65,509/65,510/65,511
   (array integer values in default) yields limit-1/exact/+1 at O0; assert actual
   successful capacities only, unconditional +1 rejection. Earlier extension and
   field API accept logical exact/+1 controls absent unexpected earlier resource
   allocation; no fabricated observations of capacity on failed outputs.
3. Raw bytes: ignored class payload size16MiB-8 + minimal method payload8 = exact;
   ±1 in ignored payload yields limit-1/pass, exact/pass, +1 annotation-bytes
   failure in declarations before any type error. Earlier phases count once.
4. Work method-only: ignored method names at CP22 length65,535 repeated340,
   ignored filler CP25 length87,346 once, visible attr name length29 with root
   path[(0,0),(1,0)], descriptorLA;, elementx, empty string CP7.
   340*3*65,536 + 3*87,347 + 103 = 67,108,864 EXACT.
   103=3*30+target1+path5+descriptor4+element-name2+empty-string-ref1.
   A method string value consumed only once: append one UTF16 unit => exact+1
   annotation-work error in final method pass (field endpoint still succeeds).
   For limit-1 reduce filler by1 (-3), add string length2 (+2); all succeed.
   Separate filler+1 yields+3 and new method work failure, not an exact+1 claim.
5. Record limit is shared annotation nodes65,535; no independent Reader records
   reset is used by type decoding (Reader.records starts0 and u1/u2 do not count).
   Depth64/65 covers arrays and alternating@/array values, root at1; target/path
   introduce no recursion. Checks must include failed count/work before truncation.

Storage: same inherited LOGICAL storage equation in TYPE_ANNOTATIONS_SPEC,
plus exact new method backing capacity:
capacity(outer)*size_of<Vec<MethodTypeAnnotationAttribute>>() +
sum(capacity(wrappers))*size_of<MethodTypeAnnotationAttribute>() +
sum(capacity(roots))*size_of<MethodTypeAnnotation>() +
sum(capacity(paths))*size_of<TypePathStep>().
Sum existing logical slots+field/method accounted new capacities <=2*65,535
(using inherited spare M <= charged M); new field+method accounted backing
capacity slots <=65,535. Multiply platform size_of for bytes, never a fixed RSS
claim. Inherited Vec spare capacity, allocator churn/rounding/transient overhead,
RSS, stack/input ClassFile storage are outside owned-byte guarantees. Observed
overshoot is known only AFTER reserve and charged BEFORE retention, not before
allocation. No whole-operation owned-byte cap introduced; no old Vec rewrite.

## Authored bytes and acceptance frozen before runtime

Minimal forged CP:0 Unusable,1 visible type name,2 invisible type name,3 LA;,
4 x,5 Integer0x80000001,6 s,7 UTF16[NUL,D800]; major52/minor0, no fields/class attrs,
one method name0/descriptor0/access0. Expanded controls reuse field test CP8Float,
9Long/10Unusable,11Double/12Unusable,13LE;,14ONE,15V,16[[I,17visible declaration,
18visible parameter,19AnnotationDefault,20value,21()[I,22Ignored,23Code,24Record.

| Authored hex (also evidence/authored-bytes) | Exact outcome within budgets |
| --- | --- |
| 00 00 | one present empty attr |
| 00 01 14 00 00 03 00 00 | target_type14, empty path, LA;, no pairs |
| 00 01 14 01 03 ff 00 03 00 00 | path[(3,255)] |
| 00 01 14 04 00 00 01 00 02 00 03 ff 00 03 00 00 | all four ordered steps |
| 00 01 14 00 00 03 00 01 00 04 49 00 05 | x Int0x80000001 |
| 00 01 14 00 00 03 00 01 00 04 73 00 07 | x String[NUL,D800] |
| 00 01 14 01 04 | Truncated (index missing) |
| 00 01 14 01 04 00 | Malformed("type annotation path") |
| 00 01 14 01 00 01 | Malformed("type annotation path") |
| 00 01 14 00 00 00 00 00 | Malformed("annotation constant index") |
| 00 01 14 00 00 05 00 00 | Malformed("UTF8 index") |
| ff ff | Limit("annotation nodes") before root EOF |
| 00 00 00 | Malformed("type annotation trailing bytes") |
| 00 01 13 | Malformed("method type annotation target") before absent path |
| 00 01 01;00 01 12;00 01 15;00 01 17 | UnsupportedTag(exact tag), no target_info validation |
| 00 01 16 | Truncated after index work charge |
| 00 02 14 00 00 03 00 00 16 | Truncated at missing index, no partial success |
| 00 02 16 14 00 00 03 00 00 | index20 decoded, then Truncated at missing second target |

Every strict prefix of minimal8-byte scalar13-byte/all-steps16-byte and valid
nested/max255-path payloads is Truncated if no count/node/work limit exhausted.
Minimal offsets:0 count,2 target,3 length,4 descriptor,6 pairs; step starts4+2*i,
descriptor4+2L,pairs6+2L. No offsets in Error. Both visibilities same byte outcomes.
Malformed descriptor in first supported root + later16 must descriptor-error
first; valid first root then missing16 index is Truncated. Extra trailing byte vs missing wrapper
nodes must trailing-error first. Same-visible duplicate at different CP index:
valid first then empty second => duplicate; truncated first then later duplicate
=> Truncated; unsupported first => UnsupportedTag. Reached16 missing index is Truncated. Distinct visibility/member
records independent. Count0 ignores trailing only if exactly2 bytes, not3.

Eventual tests MUST cover:
- Frozen exact hex outputs/errors above; all256 target values at missing suffix
  (14/16 Truncated, four legal UnsupportedTag, all250 illegal Malformed), both names.
- Alignment, empty/absent/both order, duplicates by name content, repeated roots,
  raw identity; fields/extensions preserved; class/Code/Record remain opaque;
  old public field/extension endpoints keep opaque method regressions unchanged.
- All paths/kind/index boundaries0/1/255, kinds4..255 invalid; all shared Values,
  float/NaN/integer bits, V/array class descriptors, enum and nested/empty arrays,
  UTF16 NUL/surrogates, duplicate elements and all illegal element tags.
- Consumed CP0/out-of-range/wideholes/wrongtypes/names/descriptors vs explicit
  unconsumed method refs; void/nonvoid/constructors data and inherited extension
  conditional validation/error precedence. Global version vs class CP size vs
  declarations vs extensions vs fields vs methods ordering. Invalid first attrs
  before duplicates. Empty count version gate and accepted opaque class attributes.
- Every-byte valid prefixes, exact trailing/count/tag/path/CP errors; deterministic
  byte mutations of minimal and one nested/path payload, finite every-byte*256
  probe in admitted Linux CI (controlled result/error, not exact or exhaustive).
  Exact selected mutation outcomes are table-based tests, separate from crash probe.
- All node/bytes/work/depth boundaries above, root/path capacity debit before EOF,
  work before missing target, exhaustion before lazy wrapper, field failure before
  method outer allocation, synthetic inherited helper tests unchanged. Test exact
  failed ordinary debit/checked overflow semantics through shared-helper synthetic
  tests only if necessary without changing Budget visibility/implementation;
  public errors/precedence are sufficient, no counter introspection framework.
- Licensed unmodified precompiled method-return class, exact fixture evidence below.

## Genuine precompiled METHOD_RETURN provenance

Primary artifact com.google.guava:guava:33.4.8-android:
https://repo.maven.apache.org/maven2/com/google/guava/guava/33.4.8-android/guava-33.4.8-android.jar
SHA256 a1f8a4bafe5a77232d333356843e194599b30837574a7f4be1063daaaaab102c,
2,936,569bytes; independently downloaded primary HTTPS bytes match original cache
under misleading33.4.8-jre parent. Embedded pom/manifest say android. No signature
or trust-chain authentication claim. Primary archive source manifest Build-Jdk-Spec11
is distribution metadata only; compiler identity UNKNOWN.

Unmodified com/google/common/collect/AbstractIterator.class,2361bytes,
SHA2566af0f4300e51fcb4d4720371f113e8f50e39b8f8e8c5f7f654a1297b275e0087;
headercafebabe000000340056 (major52/minor0,CP86),2fields/7methods.
Method index1 computeNext,flags0x0404,descriptor()Ljava/lang/Object;: visible type
attr nameCP63,header offset1542,payload offset1548,length8,0001140000400000.
Method index2 endOfData,flags0x0014,same descriptor: header1663,payload1669,same8bytes.
Both root target14,empty path,annotationCP64Lorg/jspecify/annotations/Nullable;,
zero pairs. Other5method entries empty. Field index1next has0x13 payload at1422
0001130000400000; class0x11 attr at2335 is opaque. Whole combined fixture can
succeed with methods.len7,field_types.fields.len2,only two method attrs and one
field attr. It is encoded annotation data, no semantic nullability interpretation.

Exact Apache2.0 LICENSE reused from delivered primary-pinned Guava field fixture,
SHA256cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30.
Tagged source https://raw.githubusercontent.com/google/guava/v33.4.8/guava/src/com/google/common/collect/AbstractIterator.java
6446bytes,SHA25628706379fd657d41e45c75c07252e84d112c1b180f613b32a439978197ef84dd.
NOTICE retains exact original source copyright/license header plus LF,
SHA2566cf6300dbf3319096c021edb3775119a8d774e029b7c1bc5128fdd639307c768.
Manifest2224bytes,SHA25696bf5ca30e73f08607b1e2066883d5903c8c2986f1dca99c1d947623cf51ad16.
Auxiliary independent Python ZIP/byte layout scripts inspect data only, never use
ktlsp/native parser or generate expectations by executing proposed decoder.
Native/JVM/compiler/Gradle/Maven/oracle never ran. Method-bearing fixture passes
provenance gate before any implementation. Strings mixed target inspection is
auxiliary corroboration only; authored mixed hex is sufficient eventual product
fixture (no extra Strings class/license scope needed).

## Formal-parameter source delta and frozen genuine fixture

Root-admitted auxiliary v1 core ad12be7397b7d66e352d220b7c205a0d3518ddb171571c382b1a4fe082e9efbd
and v2 core eccf2f0e100cdc092c14bcaae09dd5f86eba75daee53ac4b8f95667ba82b974c
freeze independent raw bytes, API compatibility, budgets and genuine data before
source. See tests/fixtures/method_type_annotations/formal_parameter/README.md
and provenance.json. Both visibilities preserve target/index/path/annotation/input
order; repeated same-index roots and mixed14/16 orders are accepted. Current
root-admitted public Option<u8> is None for14 and Some(encoded ordinal) for16.
Callers can construct inconsistent public literals; no validator is added. The
source break for external struct literals/exhaustive patterns is intentional.
Only the sole record constructor and focused test caller need repository updates;
existing export/helper/Budget/Cargo/server have no changes.

Ordinal0..255 is encoded data, not a JVM slot, synthetic-parameter adjustment,
descriptor parameter bound or Kotlin source mapping. JVMS23§4.7.20.1 explicitly
does not require direct descriptor-position correspondence. Long/double widths
never shift it. Method refs remain unconsumed unless earlier parameter/default
extensions validate them. Constructor/synthetic/void/forged descriptor controls
are encoded-only acceptance, not source/JVM legality or nullability semantics.

Minimal16 payload9bytes:00 01 16 00 00 00 03 00 00. Offsets count0,target2,
index3,pathlen4,descriptor5,pairs7; add2L to descriptor/pairs with L path steps.
Every strict valid prefix is Truncated under nonexhausted budgets. Indexwork1
precedes missing-index EOF; with index present but absent path length,Truncated
precedes path work. Shared path kind/index,CP,depth/trailing/duplicate and earlier
phase precedence stay unchanged. Other legal targets01/12/15/17 remain Unsupported.
Existing unsupported16/mixed raw fixtures are unchanged; their expected errors
are now Truncated. New22authored files freeze parameter outputs/errors explicitly.

Logical node equations remain5+X+O and mixed25+X+O. Each reached16 root adds
one raw byte charged once by declarations and one work unit BEFORE index read.
Method-only minimal visible payload uses97work,6+Onodes,9rawbytes; invisible103work.
Raw boundary ignoredclass16MiB-9 plus minimal9 is exact; ±1 remains old byte gate.
Work fixture replacing14 with16 uses340*3*65536+3*(F+1)+104+S:
F87345,S1/2/3 gives limit-1/exact/+1. Original14 budgets remain independently
asserted. Depth64/65 and all annotation values/bits/UTF16/nested data share helpers.
Selected unchanged genuine class expected9+Onodes,34rawbytes,228work; no runtime
result claimed. All existing capacity overshoot qualifications remain.

Storage equation above MUST use size_of<UPDATED MethodTypeAnnotation> with its
Option<u8>; no fixed old record-size/ABI claim. Sum of new accounted capacities
still obeys existing slot bound; element size can change without extra node/work
charges. No RSS/stack/whole-operation owned-byte guarantee or native size measurement.

## Held source gates and residual limits

Root admitted only finite source/API ownership, local fmt and compile-only Clippy,
and optional finite issue publication. SAME correctness/adversarial reviewers and
architect must review the source, fixes and exact final source manifest. Then
STOP at held head/tree/path inventory for ONE root integration base. No push/PR/
hosted CI/merge yet; no moving-main chase. No local runtime/test/parser/worker/
server/bench/JVM/compiler/Gradle/Maven/oracle execution; #96 OPEN/UNKNOWN.
Eventual root-approved unchanged Linux Native Rust all3 and actual source+CI
reviews/expected-head exclusive merge are later gates, not source preparation proof.
Parents71/17 remain OPEN;T008/T008aHOLD;B3/B4GAP;B11UNVERIFIED;7/83/96OPEN.
No full metadata/semantics/LSP parity or official speed/RAM claim.

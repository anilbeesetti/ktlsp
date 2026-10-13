# T017c3b proposed contract: field type annotations

CONTRACT ONLY, revision 2, 2026-10-13. Frozen delivered source/object:
`b910df05ba8942350700e2985d497511459952b7`. Child of
[#71](https://github.com/anilbeesetti/ktlsp/issues/71) and
[#17](https://github.com/anilbeesetti/ktlsp/issues/17); both parents stay open.
Delivered prerequisites are bounded ClassFile/declaration annotations and
PR118/#115 parameter/default extensions with the shared Budget. Root owns stage
admission, shared files, final integration base, CI and expected-head merge window.
This draft authorizes neither source changes nor execution/publication.

## Primary format and finite scope

Pin [JVMS Java SE 23 chapter 4](https://docs.oracle.com/javase/specs/jvms/se23/html/jvms-4.html),
specifically §§4.7, 4.7.16.1, 4.7.20–21, 4.7.20.1–2 and tables4.7-A/C,
4.7.20-A/C and4.7.20.2-A. This source is the same edition pinned by existing
binary modules. Only field_info RuntimeVisibleTypeAnnotations and
RuntimeInvisibleTypeAnnotations are decoded. Each payload starts with u2 count;
each type_annotation contains u1 target_type, target_info, type_path and the
existing annotation descriptor/pairs/element_value encoding. For fields the only
accepted target_type is0x13 and target_info occupies ZERO bytes.

Global supported versions remain45.0–45.3 or46–67 minor0; unsupported major/minor
returns existing UnsupportedVersion before scanning. A recognized FIELD type
attribute requires52.0–67.0, including an empty payload/zero count. Pre52 returns
Malformed("type annotation version"). Class/method type attrs remain raw/opaque,
including malformed payloads, duplicates and pre52 placements; their format is
not validated by this field-only slice. Code and Record interiors remain opaque.
Other targets in a FIELD attr return Malformed("field type annotation target")
immediately after the tag, without attempting any other union layout. This is
wrong field context, not a claim that other contexts/targets are implemented.

Path length is u1, hence0..255. Each ordered step has two u1 values. Kind0(array),
1(nested type),2(wildcard bound) requires argument index0. Kind3(type argument)
retains any encoded index0..255. Kinds4..255 and nonzero indices for0..2 are
Malformed("type annotation path"). Read BOTH bytes of a step before validating
the pair: missing index is Truncated even if the available kind is invalid.
Paths are encoded data: no field descriptor/Signature traversal, dimension,
nesting, wildcard/type-argument applicability or upper-bound validity is inferred.

At most one attr of each visibility per field, comparing attribute-name UTF16
CONTENT rather than CP index. Both visibilities may coexist in either order;
preserve attr/annotation/path/element order and repeated annotation descriptors or
paths. Absent attrs differ from present zero-count attrs. Fields with no attrs
still occupy aligned empty entries. No filtering, sorting or deduplication.

Reuse annotations::annotation(..., depth1), Value and Visibility. Preserve exact
borrowed UTF16 descriptors/names/strings, NUL/unpaired surrogate units and numeric
CP bits. Keep existing checks for consumed CP indices/types/wide holes, descriptor
and element-name validity, duplicate element names, tags, nested arrays and depth.
No annotation-class lookup, @Target/@Retention interpretation, annotation-default
application, nullability interpretation, Kotlin metadata, generics or linkage.

## Ownership, API and callers

Eventual exclusive files: NEW crates/ktlsp-binary/src/type_annotations.rs,
tests/type_annotations.rs, tests/fixtures/type_annotations/**,
TYPE_ANNOTATIONS_SPEC.md and TYPE_ANNOTATIONS_REVIEW.md inside the binary crate.
Root-gated shared changes ONLY: binary src/lib.rs module export;
annotation_extensions.rs crate-private decode_with_budget extraction only. Existing annotations.rs,
shared tests and all inherited allocation sites remain unchanged; new focused
tests verify integration and existing suites remain required regressions.
No Cargo/lock/dependency/header/wire/signatures/nesting/ZIP/project/server/vendor
changes. Root retains shared HANDOFF/PLAN/TICKETS/status ownership.

Proposed public types in new module, all deriving Debug/PartialEq/Eq:

```rust
pub struct TypePathStep { pub kind: u8, pub argument_index: u8 }
pub struct FieldTypeAnnotation<'a> {
    pub path: Vec<TypePathStep>,
    pub annotation: annotations::Annotation<'a>,
}
pub struct FieldTypeAnnotationAttribute<'a> {
    pub visibility: annotations::Visibility,
    pub annotations: Vec<FieldTypeAnnotation<'a>>,
}
```

Target0x13 is implicit in FieldTypeAnnotation; no generic target union/API is
invented for pending contexts. New public combined result and entrypoint:

```rust
pub struct FieldTypeAnnotations<'a> {
    pub extensions: annotation_extensions::AnnotationExtensions<'a>,
    pub fields: Vec<Vec<FieldTypeAnnotationAttribute<'a>>>,
}
pub fn decode<'a>(class: &'a ClassFile<'_>) -> Result<FieldTypeAnnotations<'a>, Error>;
```

Derive Debug/PartialEq/Eq consistently. fields is aligned one-to-one with
class.fields. This is ONE combined operation, no field-only standalone public
decoder/reset. Existing annotations::decode and annotation_extensions::decode
remain unchanged public interfaces/output types. No result-field addition.
Private new-module decode_fields(class,&mut Budget) returns the aligned fields;
it is not another publicly callable fresh-budget endpoint.

Exact root connection: add `pub mod type_annotations;` in lib.rs. Mechanically
extract existing annotation_extensions::decode body into
`pub(crate) fn decode_with_budget<'a>(class: &'a ClassFile<'_>, budget: &mut Budget)
-> Result<AnnotationExtensions<'a>>`. Existing public decode delegates with
&mut Budget::default(). Remove only extracted body's local Budget initialization;
pass budget instead of &mut budget at existing callsites; retain every other
check/loop/allocation/error order. No visibility change to existing helpers.
New type_annotations::decode creates ONE Budget, first calls this extension helper
(existing declarations FIRST, extension class/field/method traversal SECOND),
then calls private decode_fields with SAME Budget and returns wrapped extensions
plus fields. No duplicate declaration result or new copy/redecode. Field pass
scans FIELDS ONLY and charges every field attribute name read, no class/method
rescan, raw payload/F/M/A double charge or resetting counters. Existing method
extension errors precede field-type errors in this new endpoint.

Frozen callers: annotation_extensions::decode is imported only by
existing tests/annotation_extensions.rs. Existing AnnotationExtensions literals
are internal to that module, but external callers can construct/destructure it.
Adding a public field would break exhaustive patterns/struct literals even at
crate version0.1.0. That shorter alternative is unnecessary and is NOT preferred.
A crate-private body extraction + NEW public result is a few more lines but
preserves existing public output/source/error compatibility and existing opaque
type-attribute acceptance. Existing APIs still use their original fresh budget.
Only callers choosing the NEW combined endpoint can now reject malformed field
type attrs or exhaust extra field-scan/new output capacity nodes. When both
endpoints succeed, out.extensions equals annotation_extensions::decode, including
declarations. New field result adds no server/import/artifact consumers or LSP
capabilities. annotations::decode callers are tests/annotations.rs,
tests/metadata_header.rs, tests/metadata_wire.rs and extension tests;
metadata/header consumes Annotation/Value/DeclarationAnnotations. These consumers
and existing opaque method[255] regression must remain unchanged/pass. There are
no existing callers of this new public endpoint; focused tests become its first.

Consumed references: attr.name is consumed on every scan as before; annotation
descriptor, element names and values are consumed by shared decoder. Field.name,
field.descriptor, access flags and unrelated CP/class structure are deliberately
UNCONSUMED by this API, including recognized count0 attrs. Forged ClassFile with
invalid field name/descriptor may succeed; parsed inputs already passed parse_class.
No promise of whole-ClassFile verification is made.

## Whole-operation structural budgets; narrow new-output capacity policy

ONE Budget flows through declarations, extensions, field types and all nested
annotation/value decoding. Existing limits: all visited raw top-level attribute
payloads16MiB, nodes65,535, work64Mi units, annotation/value nesting64. All raw
payload bytes (including ignored class/field/method attrs) are charged ONCE during
declaration pass, no attribute-header bytes added. New paths do not consume depth:
they are an iterative u1-bounded vector. Array/nested annotation descent retains
existing MAX_DEPTH/depth-before-descent/drop bound.

Exact node equation extends #115: F+M+A+P+R+E+V+N+Q+C+T+L+O. F/M all members, A all raw
top-level attrs, P encoded parameter entries, R declaration/parameter annotation
roots, E pairs including nested annotations, V values including default roots and
array entries, N nested @ annotation roots, T field type-annotation roots, L path
steps. Q=reserved aligned field-output vector slots(F); C=reserved per-field
visibility-wrapper slots(2 for each field with any successfully retained type
attr,0 otherwise); O=observed extra NEW Vec capacity beyond requested counts.
T is charged from u2 count before root Vec allocation/loop; L from path u1
before path allocation/loop. No second F/M/A charge. Annotation payload's pairs,
values and nested annotations use existing charges; no duplicate root charge.
One field+one attr+X empty-path/no-pair annotations has5+X+O nodes:
X65,530 equals65,535 if O=0; X65,531 exceeds even if O=0. Adding one path step
to the first boundary input exceeds. Tests record actual NEW Vec capacities ONLY for successful outputs and
independently account O; failed decodes expose no capacities. Test the boundary
with synthetic direct reconciliation/node accounting, and use public decoding
for success invariants/+1 rejection; never assume unconditional boundary success
or observe failed-decode O. Do not depend on allocator rounding strategy.
Counts fit u2 and total raw bytes remain below16MiB; useful independent boundaries.

New work: every third FIELD attr-name read uses Budget::text (1 CP reference plus
UTF16 length), even ignored attrs. Per root, charge1 for target inspection before
reading target; then after reading path length L charge1+2*L for length/step
validation before traversal. Shared annotation/value text/reference/duplicate-name
charges remain unchanged. Allocator multiplication/capacity accounting uses checked
arithmetic but adds no work units. No field descriptor scan, uncharged post-decode
path applicability loop, global third scan or reset per member/attr.

Frozen Budget has no owned-allocation counter. Preferred MINIMUM proposal reuses
its existing crate-private nodes/work/text plus annotation/value and changes NO
shared Budget or inherited Vec allocation sites. All NEW field-pass vectors use
one module-private fixed-reservation helper
`fn vector<T>(count: usize, budget: &mut Budget) -> Result<Vec<T>>`: check/charge nodes(count) BEFORE
try_reserve_exact(count) on an empty Vec; count0 returns Vec::new; checked node
addition, reservation failure => Limit("annotation allocation") for allocation
failure and existing Limit("annotation nodes") for node excess. If actual
capacity>count, charge nodes(capacity-count) BEFORE returning/retaining the Vec.
That reconciliation is a tiny private `fn extra_capacity(requested: usize,
observed: usize, budget: &mut Budget) -> Result<()>`, called with actual capacity
by vector and independently unit-tested with synthetic requested/observed counts
(no injected allocator or growth strategy). Checked observed-requested underflow
returns Limit("annotation allocation"); node excess uses existing node label.
No push beyond original requested logical count, no later implicit growth. This
helper replaces four new output allocation kinds only: aligned fields(F), lazy
per-field visibility wrappers(2), roots(count T), paths(count L). The root/path
capacity charge is also their T/L structural node charge, never duplicated. Q/C
are deliberately additional output capacity slots, not duplicate member/raw attr
charges. Wrapper reserve occurs only after first recognized payload parsed and
exhausted successfully, before first push. Subsequent visibility reuses it.
Outer fields reserve occurs at decode_fields entry, before first field/name scan.

This scopes requested-capacity-before-allocation and observed-capacity-before-
retention accounting to NEW field-pass vectors. Nested Annotation/Element/Value
vectors are inherited storage governed by existing logical node limits, not this
new capacity accounting. No new independent budget/reset or16MiB owned cap.
Whole operation shares bytes/nodes/work/depth, but WHOLE-operation owned-byte or
retained-capacity-before-allocation guarantee is explicitly NOT established.
Actual allocator rounding is only visible after allocation; transient overhead,
RSS, stack and input ClassFile storage are excluded. Root admitted this CONTRACT-ONLY narrowing, not the exact formula/API or source
stage. Exact helpers/API/nodes/acceptance still require root approval before
source admission. Do not claim a whole-operation owned-byte limit.

Precise EXISTING logical Vec storage (not capacity) is:
(F+M)*size_of<Vec<AnnotationAttribute>>() +
M*size_of<MethodAnnotationExtensions>() +
Qdecl*size_of<AnnotationAttribute>() +
Qparam*size_of<ParameterAnnotationAttribute>() +
P*size_of<Vec<Annotation>>() + R*size_of<Annotation>() +
E*size_of<Element>() + Varray*size_of<Value>().
Qdecl+Qparam<=A (disjoint recognized attrs); Varray<=V. Nested annotation roots
are inline in Value, not separate allocated root vectors; their Element arrays
are already E. Thus existing stored element count<=F+2M+A+P+R+E+V<=2*MAX_RECORDS.
Including NEW accounted capacity, total existing logical slots + new capacity
also<=2*MAX_RECORDS using the extended equation. Multiply exact typed counts by
platform size_of for logical bytes; maximum type size times that count is a
conservative logical-storage bound. NEW Vec retained backing capacity alone is
<=MAX_RECORDS*max(size_of<Vec<FieldTypeAnnotationAttribute>>(),
size_of<FieldTypeAnnotationAttribute>(),size_of<FieldTypeAnnotation>(),
size_of<TypePathStep>()). This does NOT bound existing implicit spare capacity,
allocator overhead or RSS. Public Rust Vec API leaves growth strategy unspecified;
do not assume a generic2x/4x capacity factor or add upstream compiler archaeology.

Compared alternative, NOT preferred and NOT admitted: add private owned:usize
and crate-private Budget::vec<T>(count),16MiB cumulative requested/observed element
capacity counter, then replace ALL existing annotation/extension Vec sites. Exact
sites: declaration fields/methods, declaration wrappers/root annotations, Element
pairs, Value arrays; extension method output, visibility wrappers, parameter slots
and parameter roots, plus all NEW field-pass vectors. It would cover inherited
capacity but changes declaration-only decode acceptance and error precedence,
eagerly reserves declared counts before discovering truncation, and needs shared
ownership/regressions beyond this field feature. It is necessary only if root
retains a NEW whole-operation owned-byte cap; that is separate shared hardening.
Same allocator-overshoot-before-retention limitation applies. The minimum field
contract avoids that broad refactor and its unrelated compatibility changes.

## Errors, offsets and precedence

Reuse existing Error unchanged: Truncated; Malformed(static label); Limit(existing
annotation labels plus new allocation label); UnsupportedVersion. No new global
error variant, no UnsupportedTag for field target/path bytes. Existing Error has
NO offsets; test/evidence prefix positions below are payload-relative records,
not advertised positioned diagnostics or guessed absolute offsets.

After global declarations and existing extensions succeed, visit fields/attrs in
input order. Reserve aligned fields(Q) FIRST; then name lookup; ignored names skip. Recognized field attr: version,
duplicate visibility, u2 annotation count, nodes/root Vec reserve. Per root:
work1, target u1+validate0x13, path length u1, path work1+2L, nodesL/path Vec reserve,
read each kind+index pair and validate, shared annotation at depth1. After all
roots require exact payload exhaustion FIRST: extra bytes =>
Malformed("type annotation trailing bytes"). ONLY after exhaustion succeeds,
lazily reserve wrapper slots(C) before the first retained wrapper. Duplicate checks precede second payload reads even if malformed;
first malformed attr prevents reaching later duplicate. Count/node/allocation/work
limits can precede EOF because counts/work are charged before reading records.
All-or-error; no partial returned output, no raw/input mutation.

## Independently authored expectations BEFORE runtime

Minimal forged CP: [0 Unusable,1 Utf8 visible type attr name,2 Utf8 invisible type
attr name,3 Utf8 LA;,4 Utf8 x,5 Integer0x80000001,6 Utf8 s,7 Utf8 UTF16[NUL,D800]].
Class major52 minor0; one field with unconsumed name0/descriptor0 and selected attr.
No other fields/methods/attrs. The bytes below are hand-derived from JVMS layout
and existing annotation helper source, not decoder-generated outputs.

| Payload hex | Expected exact result (within all budgets) |
| --- | --- |
| 00 00 | one present visibility attr, zero annotations |
| 00 01 13 00 00 03 00 00 | one root, empty path, descriptor LA;, no pairs |
| 00 01 13 01 03 ff 00 03 00 00 | path[(3,255)], same annotation |
| 00 01 13 04 00 00 01 00 02 00 03 ff 00 03 00 00 | all four steps in order; no applicability rejection |
| 00 01 13 00 00 03 00 01 00 04 49 00 05 | element x=Value::Int(0x80000001) |
| 00 01 13 00 00 03 00 01 00 04 73 00 07 | x=String UTF16[NUL,D800] |
| empty; 00; 00 01; 00 01 13 | Truncated |
| 00 01 14 | Malformed("field type annotation target") before absent path |
| 00 01 13 01 04 | Truncated (read whole step before validation) |
| 00 01 13 01 04 00 | Malformed("type annotation path") |
| 00 01 13 01 00 01 | Malformed("type annotation path") |
| 00 01 13 00 00 00 00 00 | Malformed("annotation constant index") |
| 00 01 13 00 00 05 00 00 | Malformed("UTF8 index") from existing utf16 helper |
| ff ff | Limit("annotation nodes"): F1+A1+Q1+T65535=65538 before root EOF (C not yet charged) |
| 00 00 00 | Malformed("type annotation trailing bytes") |

Every strict prefix of the minimal 8-byte success is Truncated; offsets0 count,
2 target,3 path length,4 descriptor,6 pair count. For variable paths: each pair
starts4+2*i; descriptor starts4+2L and pair count6+2L. Nested values inherit shared
Reader positions. No decoder offset field is introduced. Both attr names use the
same payload expectations with only visibility changed. Two same-visible attrs
(second empty or truncated), including equal name text at different CP indexes,
must return duplicate after valid first; different fields/visibilities independent.

## Acceptance and deferred execution

1. NEW API alignment/order/empty/absent/raw preservation; both visibilities/repeated
   types/paths; multiple fields and old declarations/method results preserved.
   Class/method invalid/duplicate type attrs remain opaque, including existing
   method[255] fixture, Code/Record interiors unchanged. Forged field refs stay
   unconsumed; consumed attr/annotation/value refs reject0/out-of-range/wide holes/
   wrong types. No global CP validation accidentally added.
2. Versions45.3/48/49/51 fail recognized field attrs,52/67 pass;68/nonzero newer
   minors/preview reject globally. Paths0/1/255, all four kinds, kind3 indices0/255,
   other kinds with nonzero indices and invalid kinds4/255; all255 wrong target
   bytes reject. A syntactically valid path incompatible with a descriptor is
   retained deliberately. Duplicate attr names via independent CP slots.
3. All shared Value variants, exact float/NaN/narrow bits, class V/array descriptors,
   enum/nested/empty arrays, surrogate/NUL data and duplicate elements. Shared
   depth64 accepted/depth65 rejected in field values, with bounded drop.
4. Every-byte prefixes of independently authored valid scalar, max-path and nested
   payloads; trailing/count/target/path/CP/version/duplicate and declarations-first/
   extensions-before-types precedence, including before-allocation count failures.
   Small deterministic single-byte mutations of fixed valid payloads must return
   controlled result or error in eventual admitted Linux CI; no exhaustive claim.
5. Independently calculated mixed declaration/parameter/default/type/path node
  65,535/+1 boundaries;16MiB raw-byte exact/+1 accounting with all scans sharing
   payload charge once;64Mi work exact/+1 with extra FIELD-only scan exhaustion,
   and unchanged class/method-only exact extension-work tests. No budget resets.
6. NEW fixed reservation helper count0, checked node/capacity addition, private
   extra_capacity synthetic exact/overshoot/underflow checks, exact
  65,535/+1 node budget and extra-capacity reconciliation without depending on
   allocator rounding. Verify outerQ/lazyC/T/L/O charges precede retention/growth.
   Platform size_of determines exact logical storage, never a claimed RSS bound.
   All existing/new structural counters are shared. Existing annotation/extension
   tests and metadata header/wire consumers pass unchanged; no new declaration-only
   allocation limit or error change.
7. One LICENSED unmodified PRECOMPILED field-type fixture with origin/artifact/
   member identity, archive/class/license/source-notice SHA256, compiler version
   only if known, actual target0x13 bytes and independently expected outputs. No
   javac/JVM/Gradle/Maven fixture generation or product dependencies.

Darwin #96 remains OPEN/UNKNOWN. No local Cargo/build/compile/fmt/native/parser/
server/test/benchmark/JVM/oracle execution at this stage. Source-only contract
reviews do not prove runtime/crash safety. Later root may admit source and optional
fmt/compile-only Clippy; runtime validation remains unchanged existing Linux
Native Rust CI all3 on exact reviewed combined head/tree and exact merged main.
Same two reviewers plus architect review source, inherited workspace/default/
excluded dependency graph, execution effects and resource controls. Root assigns
ONE final integration base, then expected-SHA merge window after all required CI;
no main chasing/admin/bypass/force-main. Close only intended child using explicit
ClosingIssuesReferences; keep parents open. T008 HOLD; B3/B4 GAP, B11 UNVERIFIED.
Full official Kotlin LSP parity and semantics remain pending.

## Precompiled fixture evidence (auxiliary, not published)

Existing cache file:
`/Users/anil/.gradle/caches/modules-2/files-2.1/com.google.guava/guava/33.4.8-jre/cb7510a733595f02fe633ffff62d40cfc3056e9/guava-33.4.8-android.jar`.
Path's parent says jre, but ZIP pom.properties/manifest both identify
com.google.guava:guava:33.4.8-android. Record this discrepancy; do not infer origin
authentication from the cache. Intended primary artifact URL:
https://repo.maven.apache.org/maven2/com/google/guava/guava/33.4.8-android/guava-33.4.8-android.jar.
Archive SHA256 a1f8a4bafe5a77232d333356843e194599b30837574a7f4be1063daaaaab102c.
Unmodified entry com/google/common/base/Suppliers$MemoizingSupplier.class,
2,893bytes(0xb4d); class SHA256
3b7ea59a28c1ef4c3a44996c78aeb18e543ded6258d7c9a1841af8b3973b10ec.
Header bytes ca fe ba be 00 00 00 34 00 74: major52,minor0,CP count116.
Manifest SHA25696bf5ca30e73f08607b1e2066883d5903c8c2986f1dca99c1d947623cf51ad16;
Build-Jdk-Spec11 is only distribution metadata, not verified compiler identity.

Hex inspection: fields_count5; field index3 name CP0x23 `value`, descriptor CP0x0c
Ljava/lang/Object;, Signature CP0x48 `TT;`; visible type attr name CP0x49, length8;
payload file offset0x80d is `00 01 13 00 00 4a 00 00`. Expect out.fields.len()==5, only index3 with one visible attr/root, empty path/elements, annotation
descriptor CP0x4a Lorg/jspecify/annotations/Nullable;. This is data, no nullability
interpretation. Class/method type attrs remain ignored.

Primary tagged source corroborates the field:
https://github.com/google/guava/blob/v33.4.8/guava/src/com/google/common/base/Suppliers.java.
Copyright(C)2007 The Guava Authors; Apache2.0. Exact LICENSE primary:
https://raw.githubusercontent.com/google/guava/v33.4.8/LICENSE.
Cache and primary ZIP have no LICENSE/NOTICE entry found by unzip listing.
Primary archive was separately read over HTTPS into auxiliary evidence and its
SHA256 equals the cache archive; both unmodified class entries cmp-identical. This
resolves the cache-content discrepancy against that pinned primary artifact; no
signature/trust-chain authenticity claim. Downloaded primary LICENSE SHA256
cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30; tagged
Suppliers.java source SHA2569f2e41265455b7e71dc08275421a03aaee0be7a9866d14ef829af321d91fc778.
Exact LICENSE and source copyright header are retained in auxiliary NOTICE.txt
(SHA256285cfd39471bf1c1ddee5b40ee4b3cd61ad87e0f899446e449245957942e5c61).
Retain both unchanged with the fixture on eventual admitted publication.
Complete machine-readable auxiliary provenance is evidence/provenance.json;
hand-authored pre-runtime hex fixtures are evidence/authored-bytes/*.hex. Their downloads used ordinary require_escalated network approval,
not a retry of denied Git metadata/DNS mutation. No compiler/JVM/Gradle/Maven,
native product reader/parser, worker or oracle ran to obtain this byte evidence.

## Preserved environment failures

Managed fetch exact b910 failed exit255:
`error: cannot open '/Users/anil/Developer/personal/ktlsp/.git/worktrees/ktlsp5/FETCH_HEAD': Operation not permitted`.
Coordinator explicitly allowed isolated /private/tmp fallback; same-SSH default
sandbox clone failed exit128 before fetching: `ssh: Could not resolve hostname
github.com: -65563`, then fatal could not read repository. Neither was an
auto-review rejection. No retry/credential/transport/executor change occurred.
Coordinator then confirmed existing frozen git object read-only is sufficient.
Managed checkout stays at61142f6; preparation uses git show b910 only. No product
source edits or execution; auxiliary drafts/byte/hash evidence only.

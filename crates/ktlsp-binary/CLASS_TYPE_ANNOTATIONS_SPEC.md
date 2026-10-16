# Encoded CLASS type-annotation data

Frozen contract SHA256 `28050fbc6c546cd2618c29e8d223eb9f246885b16e1f88de122c365bac2ee794` was admitted for source/API/publication/original CI by root. Child [#199](https://github.com/anilbeesetti/ktlsp/issues/199). Additive Rust data API only; parents #71/#17 stay open.

## Primary format, ordering, raw indices

Primary format is [JVMS SE23 chapter4](https://docs.oracle.com/javase/specs/jvms/se23/html/jvms-4.html#jvms-4.7.20),
§§4.7.20/21, 4.7.20.1/2, tables4.7.20-A/C. Only class.attributes
RuntimeVisibleTypeAnnotations / RuntimeInvisibleTypeAnnotations are decoded.
Each has u16 count, then target tag/info, u8 path length and two u8 per step,
then the existing annotation encoding. All three class targets are implemented:
- 00: u8 type_parameter_index
- 10: big-endian u16 supertype_index
- 11: u8 type_parameter_index followed by u8 bound_index

All other 253 tags, including legal field/method/Code tags, immediately return
Malformed("class type annotation target") after the tag; never UnsupportedTag.
No union-body read after an illegal tag. A class attribute with a mixed legal/
illegal sequence fails the whole operation with no partial output.

Supertype raw index0xffff denotes the encoded superclass sentinel; other values
are encoded interface indices. Retain the u16 unchanged, including0/1/255/256/
65534/65535. Do not check interface count, superclass existence, access flags or
class-vs-interface legality. Parameter/bound raw indices0..255 likewise are not
checked against Signature, generic declarations or implicit Object bounds.
Structural encoded decoding success does not establish those applicability rules.
Do not normalize a sentinel to None, a CP reference or an interface ordinal.

TypePathStep from existing type_annotations is reused unchanged: kinds0/1/2 need
index0, kind3 accepts0..255; kind4..255 or nonzero index for0..2 is
Malformed("type annotation path"). Read both step bytes before validation, so
invalid-kind/missing-index is Truncated. Keep order/repeats and path length0..255.
No semantic type-path applicability checks, descriptor/Signature traversal or
generic-bound inference. Paths do not consume recursive annotation depth.

At most one attr per visibility, comparing UTF16 name content, even different CP
indices. Both may coexist in either input order; retain attr/root/path/element
order and duplicates. Present zero-count attr differs from absent. Version gate
before duplicate check, duplicate check before second payload read; first payload
failure wins before a later duplicate. Exact payload exhaustion is required,
otherwise Malformed("type annotation trailing bytes").

Global versions inherited from declarations:45.0..45.3 or46..69 minor0 only;
UnsupportedVersion first. Recognized class type attr requires major52..69,
even count0/empty/truncated payload: pre52 -> Malformed("type annotation version").
Absent class type attrs do not impose52. Field/method type attrs, Code/Record
interiors and non-type attrs remain opaque to this new phase, including duplicate,
malformed or pre52 type payloads elsewhere. Declaration attributes everywhere are
still validated by the mandatory earlier declaration phase.

## API and callers: additive and class scoped

All new public types derive Debug/PartialEq/Eq:

```rust
pub enum ClassTypeTarget {
    TypeParameter { index: u8 },
    Supertype { index: u16 },
    TypeParameterBound { parameter_index: u8, bound_index: u8 },
}
pub struct ClassTypeAnnotation<'a> {
    pub target: ClassTypeTarget,
    pub path: Vec<type_annotations::TypePathStep>,
    pub annotation: annotations::Annotation<'a>,
}
pub struct ClassTypeAnnotationAttribute<'a> {
    pub visibility: annotations::Visibility,
    pub annotations: Vec<ClassTypeAnnotation<'a>>,
}
pub struct ClassTypeAnnotations<'a> {
    pub declarations: annotations::DeclarationAnnotations<'a>,
    pub class: Vec<ClassTypeAnnotationAttribute<'a>>,
}
pub fn decode<'a>(class: &'a ClassFile<'_>) -> Result<ClassTypeAnnotations<'a>, Error>;
```

Use one Budget::default, call annotations::decode_with_budget first, then a private
decode_class(class,&mut Budget) in the new module; return declarations and class.
No public class-only reset endpoint or wrapper over field/method pipeline.
No duplicate declaration decoding/copy or extension traversal. Reuse unchanged
annotations::annotation(...,depth1), TypePathStep, and type_annotations::vector.
No new helper extraction, change to existing helpers, or new shared Budget fields.

Existing declarations/extensions/field/method public result types and behavior
remain unchanged, including continued opaque class type attrs. Old consumers:
annotation_extensions internal decode_with_budget; type_annotations internal
decode_with_budget; method_type_annotations; metadata/header and its tests;
annotations/extension/type/method/metadata_* tests. No existing new API caller;
only focused new tests call it. External struct literals/patterns remain source
compatible. No server/import/artifact consumer/capability is added. New endpoint
can fail on malformed class type attrs or additional work/node charge when the
declaration endpoint succeeds; when both succeed declarations must compare equal.

Consume attr.name on every declaration pass and again every CLASS attr in the new
phase, including ignored names. Existing annotation CP/type/descriptor/name/tag/
duplicate-name/depth checks apply. Borrow exact UTF16 descriptor/name/string
units including permitted NUL/lone surrogates and preserve numeric bits. Do not
consume field/method names/descriptors, class.this_class/super_class/interfaces,
unrelated CP entries or Signature bytes. Forged unrelated fields can succeed;
this is not whole-ClassFile verification. Raw bytes/input remain unchanged on error.

## Budgets and exact failure precedence

One aggregate inherited Budget across declarations then class attributes:
16MiB raw visited top-level class/field/method payloads (including opaque attrs),
65535 structural/new-vector-capacity nodes,64Mi work units,annotation depth64.
Raw bytes/containers/member counts charged once in declarations, not again.
Inherited declaration Element/Value vectors keep logical-node accounting; new
vectors charge requested count before fallible exact reservation and observed
extra capacity before retention through unchanged type_annotations::vector.
This does not bound allocator overhead, transient allocation, owned bytes or RSS.

New root vector charge u16 count T once before target loop; path vector charge
u8 length L once before path loop. New per-class wrapper vector reserves2 only
after first recognized payload is completely parsed/exhausted and before push;
no reservation if absent, reuse on second visibility. Every push stays within
requested logical count. Zero count path/root retains no new allocation.
No aligned outer class vector slot (there is just one class). If O is observed
extra capacity for new roots/paths/wrapper, total nodes are inherited declaration
nodes + T + L + C + O, C2 with any retained class type attr, otherwise0.
Inherited unchanged vector helper already checks node addition/observed subtraction,
reservation failure, overshoot and overflow; do not duplicate it or add allocator
injection/counter-introspection framework.

Work: Budget::text for each new class attr name; charge1 BEFORE target u8 read.
00 charge1 before its u8;10 charge2 before atomic u16;11 charge1 before first
u8, then1 before second u8. After successful path-length u8 read, charge1+2L
before path capacity/traversal. Missing path length therefore Truncated before
path work debit; earlier target-info exhaustion precedes missing info bytes.
Use existing annotation/depth/CP work. Width arithmetic bounded by u8/u16;
existing checked counters handle aggregate overflow. No extra semantic scans.

Overall order: global version -> CP-size -> inherited field/method counts ->
declaration class/fields/methods in existing order (all raw payload/name/container
checks) -> new class attr names -> recognized version -> duplicate -> u16 count
-> root capacity -> target work/tag -> info work/read(s) -> path-length read
-> path work -> path capacity -> read both bytes/validate each step -> annotation
-> exact exhaustion -> lazy wrapper capacity -> next attr. Any error discards output.
Earlier declaration/name/byte failures, including on later opaque attrs, precede
an earlier malformed type payload because declarations finish first.

Hand-derived base: no fields/methods, one visible class attr, CP1 visible name,
CP3 LA;, empty path/pairs. 00 payload9B:4+O nodes,67work;10/11 payload10B:
4+O nodes,68work. Invisible uses4more work (name31vs29 read twice).
Node X no-path/no-pair roots: X+3+O; X65532 reaches65535 iff O0, X65533
always exceeds; path+1 also exceeds exact boundary. count65535 with only count
bytes fails nodes before target EOF. Do not assume allocator-rounded boundary
success; inspect successful capacity and account O; never inspect failed outputs.
Public successful outputs independently reconcile used new capacity; inherited
synthetic helper overflow/overshoot regressions remain unchanged/required.

Byte boundary: ignored class attr MAX_BYTES-L plus minimal class attr L(9/10)
is exact, +/-1 separately frozen. No header bytes added. Work/depth/node boundaries
tested with private new phase precharged Budget (nodes/work public(crate) methods,
no visibility edits) and public aggregate fixtures; synthetic debit MAX then1 and
usize::MAX shows Limit, with the existing same labels. In new-phase private tests
name needs30work then target1. Missing00 info: remaining31=>work Limit,
32=>Truncated. Missing10 info:remaining32=>work Limit,33=>Truncated.
Missing11 first info:31=>Limit,32=>Truncated; first present/second missing:
32=>Limit,33=>Truncated. Fully present info/missing path length yields Truncated
even when no work remains. Nonempty declared path charges work then nodes before
step EOF; missing root target can be preceded by root capacity/node exhaustion.
Exact exhaustion precedes lazy wrapper node exhaustion for trailing data.
Depth64 succeeds if other bounds allow,65=>Limit("annotation depth"), inherited
depth-before-descent; path255 is iterative. Full inherited regression suites stay.

## Independently authored frozen fixture cases

CP contract for literal tests (unrelated base data remains forged/unconsumed):
0 Unusable;1 Utf8 RuntimeVisibleTypeAnnotations;2 Utf8 RuntimeInvisibleTypeAnnotations;
3 Utf8 LA;;4 Utf8 x;5 Integer0x80000001;6 Utf8 s;7 Utf8 units[0,0xd800];
8 Float0x7fc01234;9 Long u64::MAX;10 Unusable;11 Double0xfff8000000001234;
12 Unusable;13 Utf8 LE;;14 Utf8 ONE;15 Utf8 V;16 Utf8 [[I;
17 RuntimeVisibleAnnotations;18 Ignored;19 Code;20 Record. Extra equal names use
new CP slots only in duplicate/UTF16-content tests. Descriptor/pair indices below
are independently chosen literals, never emitted by proposed decoder/compiler.

Every following case is one eventual authored/<name>.hex file; literal data and
expected result are frozen here BEFORE any product execution. Both visibilities
share payloads. Error abbreviations: Truncated; Target=Malformed("class type
annotation target"); Path=Malformed("type annotation path"); Trailing=Malformed
("type annotation trailing bytes"); Zero=Malformed("annotation constant index");
Type=Malformed("annotation constant type"). Success includes borrowed descriptor
CP3 and zero pairs unless specified; preserve exact target/path order.

The 28 literal payloads and exact outcomes are preserved in [fixture expectations](tests/fixtures/class_type_annotations/EXPECTATIONS.md). Additional focused tests cover every promised boundary/control; execution evidence is tracked in [review](CLASS_TYPE_ANNOTATIONS_REVIEW.md).

## Existing genuine fixture and limitations

Reuse exact existing tests/fixtures/method_type_annotations/AbstractIterator.class.bin
via include_bytes!("fixtures/method_type_annotations/AbstractIterator.class.bin").
2361B SHA2566af0f4300e51fcb4d4720371f113e8f50e39b8f8e8c5f7f654a1297b275e0087,
minor0 major52 CPcount86. Existing provenance.json primary archive coordinate
com.google.guava:guava:33.4.8-android,2936569B,
SHA256a1f8a4bafe5a77232d333356843e194599b30837574a7f4be1063daaaaab102c.
Origin https://repo.maven.apache.org/maven2/com/google/guava/guava/33.4.8-android/guava-33.4.8-android.jar
Existing LICENSE.txt Apache2.0 SHA256cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30,
NOTICE.txt SHA2566cf6300dbf3319096c021edb3775119a8d774e029b7c1bc5128fdd639307c768.
No artifact download, JVM/compiler/native parser needed. Existing provenance's
cache discrepancy/compiler UNKNOWN/HTTPS equality-not-signature limits remain.

Independent xxd confirms header at2329 (0x919): CPname63(003f), length10;
payload offset2335(0x91f),hex00011100000000400000. Expected new result class one
RuntimeVisible attr, one TypeParameterBound{parameter_index:0,bound_index:0},
empty path, borrowed CP64 Lorg/jspecify/annotations/Nullable;,zero pairs.
Existing field13 and method14 type attrs are opaque to this new endpoint.
Existing declaration result must equal separate annotations::decode result.
Existing third-party notices stay untouched and linked from new README/spec.
This genuine fixture covers only11, not00/10; those have authored encoded cases.
No nullability/annotation identity/linkage/whole-class applicability/semantic
parity/current performance or fresh official oracle claim follows.

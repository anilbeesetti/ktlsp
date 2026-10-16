# Bounded classfile reader (T017a / #58)

`parse_class(&[u8]) -> Result<ClassFile<'_>, Error>` reads classfile structure as data, using only Rust's standard library. No existing server caller or capability uses it yet. All fixtures in `tests/classfile.rs` are independently authored Rust byte builders, with no copied third-party code or compiler-produced artifacts. No JVM/compiler/oracle was run.

Supported version subset: 45.0–45.3 and 46.0–69.0 (through Java 25). Preview minors, nonzero minors for majors 46–69, newer and older versions return `UnsupportedVersion`. This deliberately excludes some historically valid versions. All six structural/attribute entry gates share this policy. New CP tags are gated at 51 (method handle/type/invokedynamic), 53 (module/package), and 55 (dynamic). Unknown tags return `UnsupportedTag`.

The result preserves raw access flags, CP indices, interface/member order, and borrowed opaque attribute payloads. CP zero/wide second slots are `Constant::Unusable`. Integer and floating constants preserve their bits. `utf16(index)` exposes modified UTF-8 losslessly as UTF-16 code units, including encoded NUL and isolated surrogates; callers decide whether conversion to a Rust string is appropriate.

Java 24/25 admission changes only ordinary version support, not JVM verification,
descriptor grammar, tag introduction floors, attribute applicability or resource
bounds. [JVMS 23](https://docs.oracle.com/javase/specs/jvms/se23/html/jvms-4.html),
[24](https://docs.oracle.com/javase/specs/jvms/se24/html/jvms-4.html) and
[25](https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html) retain the
consumed constant tags, member/descriptor and outer-attribute framing. Unknown
attribute interiors remain opaque; no preview, Q descriptors or Java-source
constructor-order restriction is added. Historical review receipts below and in
`*_REVIEW.md` describe their original reviewed ceilings, not the current policy.

Pinned Eclipse JDT minimal JCL **test-library** fixtures for 68.0/69.0 reside in
`fixtures/modern-class-versions`: unchanged archives, exact extracted Serializable
and method-bearing Object classes, matching source/build/about/EPL-2.0 notices,
and `PROVENANCE.json` with whole ZIP inventory, SHA-256 and selected byte walks.
They are not stock SDKs or evidence of an exact compiler or bundled LSP JDK.
These new test expectations have not yet been exercised; ROOT owns validation.

Validation covers structural typed CP references, method-handle kinds/targets, class/member names and descriptors, array depth (255), parameter slots (255, including receiver for declared instance methods), attribute name indices and lengths, and exact EOF. It does not resolve external classes. Special method names have descriptor/handle restrictions. Attribute names are validated as UTF-8 pool references; their text and contents are otherwise opaque.

Budgets are checked before reading/adding records: 16 MiB input; 65,535 aggregate CP slots (including slot zero and wide holes), interfaces, fields, methods and attributes; 64 Mi UTF-16 units of referenced name/descriptor validation. Shared long-name references consume this work budget again and can return `Limit("validation work")`. Reading and validation are iterative; output allocation is bounded by these budgets and decoded strings by input bytes. Attribute bytes are borrowed, never copied or interpreted. Numeric reads use checked slices before fixed-size conversion. Errors distinguish truncation, malformed structure, unsupported versions/tags and resource limits.

Success is **not JVM verification**. Attribute interiors (Code, BootstrapMethods, Signature, annotations, nesting, etc.) and their internal CP/bootstrap indices are unchecked. Access-flag combinations, duplicate declarations, module structural constraints, linkage and bytecode validity are deferred. Generic signatures, annotation/nesting interpretation and Kotlin metadata need separate children; parent #17 remains open. No Java semantic or Kotlin LSP parity is claimed.

Format contract: [JVMS Java SE 23 chapter 4](https://docs.oracle.com/javase/specs/jvms/se23/html/jvms-4.html). This reader implements only the subset above.

## Borrowed descriptor structure (#358)

`descriptors::FieldDescriptor::parse(&[u16])` and
`MethodDescriptor::parse(&[u16], SlotContext)` validate complete input before
returning private-field, copyable borrowed views. `raw()` preserves descriptor
identity; `dimensions()` is the flattened array rank (0–255), and `component()`
distinguishes all eight primitive kinds from a borrowed lossless UTF-16 internal
object name. No String/Vec/Box or text conversion is performed per descriptor.
The class reader's existing descriptor validator delegates to these same parsers;
parameter iteration uses the sole shared field-type grammar.

`parameters()` produces a fresh allocation-free iterator in encoded order.
`result()` is `None` only for void; V is rejected for fields, array components
and parameters. `parameter_slots()` includes an implicit receiver only when the
caller explicitly selects `SlotContext::WithReceiver`. The receiver is not an
encoded parameter; long/double use two slots, arrays/references one, and totals
over 255 fail. Constant-pool descriptors with no known invocation context can use
`WithoutReceiver`; declaration callers must supply their actual receiver context.

Each supplied slice is capped at 65,535 UTF-16 units before scanning. This is a
decoded-unit cap, not validation of modified-UTF-8 encoded byte length (the
classfile reader checks its u2 byte length separately). Validation is iterative,
linear, at most two scans of object names, and uses checked rank/slot arithmetic.
Each iterator traversal reparses only its validated parameter slice linearly.
Malformed, missing, trailing and oversized input returns `Error`, with no partial
successful view, lossy normalization or recursive allocation. Existing aggregate
classfile validation-work limits remain unchanged.

Format rules: [JVMS §4.3.2 field descriptors and §4.3.3 method descriptors](https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.3.2),
including 255 dimensions and 255 parameter slots with receiver. Names reuse the
reader's internal-name validation. This is structural data only: generic
`Signature` parsing remains in `signatures`; descriptors establish no Kotlin
nullability, generic substitution, linkage, access control or resolved symbols.
No server capability or public feature-parity claim changes.

`tests/descriptors.rs` exercises all three unchanged retained Commons Lang 3.17.0
classfiles from `fixtures/t017c1` through the public API, plus malformed, void,
slot/rank/input boundaries and borrowed-pointer identity. Existing
`provenance.json`, LICENSE and NOTICE remain authoritative and unchanged.
A separate native pure-data consumer on the composed source observed the retained
Builder's four fields, seven methods and four parameters, checked original borrowed
UTF-16 identity, primitive/array/void structure and explicit malformed-input failure.
Its temporary runner was removed after observation; permanent API regressions remain.
Original source `f7d63b099e9901da2419f656d3d510a8dc1d394b` passed
[attempt-1 native CI38876243449/job116891884122](https://github.com/anilbeesetti/ktlsp/actions/runs/38876243449/job/116891884122):
895 workspace passes at synthetic checkout `8344379420d4236ea7e931fb4651b55df487e8aa`,
tree `fb3b5d869fe9ba6a24bcb232087fdc8b8de16ddf`, ordered parents
`[bb6a75f61fd467c9b090210a369c7e1d225713d5,f7d63b099e9901da2419f656d3d510a8dc1d394b]`.
ROOT independently authenticated those original identities and unchanged workflow.
These are component/consumer observations, not final aggregate CI or parity evidence.

## Selected archive classfiles

`parse_zip_class(&index, ordinal, &mut scratch)` is the production composition of
`ktlsp-artifacts` payload validation with `parse_class`. Method 0 borrows the
retained verified artifact; method 8 borrows caller-reusable `Vec<u8>` scratch.
There is no owning self-referential wrapper or payload copy. Hold scratch while
the returned class borrows its attribute bytes. `ZipClassError` distinguishes
stored/deflated validation from the original classfile error; no errors are
swallowed and no different entry is guessed.

The caller supplies an indexed central ordinal, not an archive-name winner.
Duplicate names are not resolved. This API does not implement manifest,
multi-release, signature, class linkage, semantic symbols or server integration.
No extraction, network or Java is used. Payload flags, local placeholders,
descriptors and independent compressed/decoded/work ceilings are specified in
[the artifacts payload contract](../ktlsp-artifacts/ZIP_STORED.md).

`tests/archive.rs` includes the unchanged genuine Commons Lang 3.17.0 JAR in
`fixtures/t017c1/commons-lang3-3.17.0.jar.bin`. Existing `provenance.json` records
the source URL, archive SHA-256 and extracted member hashes; bundled Apache
LICENSE/NOTICE files are unchanged. A temporary explicit canonical Maven
repository feeds lookup, caller SHA-256, directory indexing and ordinal 31 into
`parse_zip_class`. Assertions retain exact Builder identity, class/member flags,
field/method descriptors, interface, attribute structure and decoded bytes.
Default tests are offline and require neither Java nor Maven/ZIP executables.

Consumer-visible regressions also exercise stored/deflated methods, signed and
unsigned descriptors, placeholder policy, signature-valued CRCs, ZIP64 rejection,
malformed/truncated/trailing streams, CRC, exact decoded lengths, compressed and
decoded ceilings, scratch reuse/error clearing and competing descriptor extents.
Two small independent boundary inputs, `fixtures/zip-zero-8388608.deflate.bin`
and `zip-zero-8388609.deflate.bin`, are raw DEFLATE produced as data by Python
zlib level 9 (`compressobj(level=9, wbits=-15)`) from that many zero bytes:

| Input | Compressed bytes | SHA-256 |
| --- | ---: | --- |
| 8,388,608 zeros | 8,157 | `11c267075b9c8783848d51618de3aa1f46ee79ed089538459a7383c7848b8218` |
| 8,388,609 zeros | 8,157 | `3e46a1d86f76a8fdad7b606e977e0a5e30e26e02de550de8c7d077e4579ced49` |

An authorized locked package-test wave passed, and a separate offline
throwaway production API consumer observed `AppendableJoiner$Builder` 52.0,
flags `0x0031`, Supplier interface, four fields, seven methods and 2,510 decoded
bytes matching member SHA-256
`e288160745c56dff019f2af9393039558018308de89509bdd81a8535bfa9b03d`.
This proof is classfile data consumption, not JVM verification or LSP parity.

## Source attachment candidates (#343 / #330 / #17 / #7)

`source_attachment::attach(&class, &source_index, &mut scratch)` consumes a
`ClassFile` and an immutable `ZipDirectoryIndex` backed by a `VerifiedArtifact`.
It requires exactly one class-level `SourceFile` attribute with exactly two bytes
and a nonzero UTF-16 CP index. Missing attributes/candidates, malformed indices,
duplicate attributes, member-level context, unsupported metadata and resource
refusals remain explicit. Publicly constructed or changed classes are checked at
consumed references; this is not a second whole-class verifier.

The class package plus the safe unqualified filename selects exactly one raw
central-directory name. No allocation/copy of a candidate path or entry names,
normalization, suffix search, traversal, extraction, duplicate winner or fallback
occurs. Duplicate exact matches return `Ambiguous` with the first two central
ordinals, even when bytes are identical. UTF-16 must decode losslessly; lone
surrogates are refused, never replaced. Non-ASCII matching ZIP names require the
UTF-8 flag rather than assuming the legacy encoding. Pinned Metadata decoding
reuses the existing header contract (`mv=[2,3,0]`, kinds 1/2/5); nonempty `pn`
overrides the JVM package using a safe dotted source package. Unsupported fields,
versions/kinds and malformed headers refuse rather than selecting the Java-style
path. `JvmPackageName`/`KotlinJvmPackageName` annotations are explicitly unsupported.

Attachment scanning is bounded by 65,535 container/attribute nodes, 16 MiB
attribute bytes, 16 Mi work units and 4,096 UTF-8 path bytes. The reused annotation,
Metadata and selected stored/DEFLATE decoders retain their separate budgets;
there is no combined RSS guarantee. Returned central entry identity is borrowed
from the index; source bytes borrow the verified artifact (stored) or reusable
scratch (DEFLATE), without a redundant payload copy. Scratch is cleared on every
entry/refusal and cannot be reused while its returned source slice is borrowed.

`tests/source_attachment.rs` exercises the actual production boundary, including
the unchanged genuine Commons Lang 3.17.0 source JAR. Binary ordinal 31's
`AppendableJoiner$Builder` SourceFile selects source ordinal 27,
`org/apache/commons/lang3/AppendableJoiner.java`: 11,798 exact bytes, SHA-256
`bb5a9762fa8614c2a49e5ccb770f1378c60d5fdb7b8679039c85bdabe7f57315`.
The source JAR SHA-256 is
`5fdcac21ad329766054a95367d7583dfcdca737d221d5e01a5f2a198c04c6b18`;
the canonical primary Maven URL and acquisition facts are in existing
`fixtures/t017c1/provenance.json`. Source-JAR Apache LICENSE/NOTICE are byte-identical
to the retained notices. Tests are offline, with no JVM, compiler or extraction.
The post-review offline package wave passed formatting, strict locked all-target
Clippy and all 448 binary-crate tests. A separate throwaway compiled Rust consumer
called the public artifact/index/class/attachment APIs and observed source ordinal
27, the exact path above, 11,798 bytes equal to the independently validated source
payload. No local parser/server runtime or whole-workspace execution was used.

This returns only a **path-based attachment candidate**: neither artifact digest
equality nor SourceFile establishes that the binary was compiled from these
source bytes, resolved declaration identity, or line mapping. No decompiler,
filesystem extraction, Java/network runtime, server integration or LSP capability
is introduced. Other Kotlin versions/kinds, JvmPackageName interpretation,
multi-release/source-root policies, full Kotlin attachments and semantic
navigation remain pending; parents #330/#17/#7 remain open.

## Ordered archive class lookup (#353 / 330/17/7)

`class_path::lookup_class(&roots, internal_name, java_release, &mut scratch)`
queries borrowed `ClasspathRoot::Jar(&ZipDirectoryIndex)` and
`ClasspathRoot::Jmod(&JmodDirectoryIndex)` values in caller-specified order.
No caller ordinal, repeated indexing or owning self-reference is required.
`None` means missing; the first matching root wins, retaining its index,
central ordinal and `ClassRank`. Duplicate entries at the winning rank are
ambiguous. Selected CRC/payload/class/name/version failures never fall through
to an older version or a later root. Declared `this_class` UTF-16 must exactly
match the requested Unicode scalar name, without normalization or replacement.
Names are bounded UTF-8 JVM internal names; empty segments, controls, dot,
semicolon, array prefix, backslash and colon are rejected. Archive names are
compared as exact UTF-8 bytes, not platform paths or legacy code-page strings.

JAR selection follows the [primary JAR specification](https://docs.oracle.com/en/java/javase/23/docs/specs/jar/jar.html)
and [JEP 238](https://openjdk.org/jeps/238): only a well-formed actual manifest
main section with case-insensitive `Multi-Release: true` (no value whitespace)
enables versioning. CRLF/LF/CR and continued UTF-8 values are supported;
`Manifest-Version` is required first in its specified case, headers are unique
case-insensitively, physical lines are at most 72 bytes, values at most 65,535
bytes, and the main section ends with an empty line. Named sections are ignored.
Malformed/duplicate manifests error rather than silently enabling/disabling MR.
Manifest archive names are found case-insensitively; multiple candidates error.
The highest canonical decimal `N >= 9` not exceeding the explicit Java feature
release wins, then base; `META-INF` contents are not versioned. Versioned classes
must have major <= N+44; base/JMOD major <= requested release+44. Existing
unsupported class versions/minors remain unsupported, not disguised as missing.
JMOD queries only `classes/name.class` and never consult manifests or MR areas.

Whole-query metadata preflight precedes allocation: at most 64 roots, 65,536
aggregate entries, 16 MiB aggregate scanned name bytes (three bounded passes),
256 KiB each manifest compressed/decoded size and 1 MiB aggregate manifest
compressed+decoded sizes. Main sections allow at most 4,096 headers, with
bounded temporary header/value storage. Selected payload and class output/work
retain the existing archive/class-reader ceilings. Only one class is returned.
Tests use unchanged genuine Commons Lang Builder and its Java 9 MR
`module-info.class`; adversarial archives/classfiles are independently authored
data. Nothing runs Java, reads paths, extracts files, follows manifest
`Class-Path`, validates signatures, resolves modules or verifies API equivalence.
This is ROOT/data-only groundwork, not SDK integration, JVM verification,
semantic classpath resolution or full official Kotlin LSP parity.

## Encoded method type annotations

`method_type_annotations::decode` composes declaration/extension/field data with all six legal method targets (0x01, 0x12, 0x14, 0x15, 0x16, 0x17), sharing one annotation budget. `target_type` retains the raw tag and `target_info: MethodTargetInfo` retains fixed-width indices. **Breaking API change:** `formal_parameter_index` was removed; formal-parameter data is now `MethodTargetInfo::FormalParameter(index)`. External field reads/literals/patterns and derived Debug/layout/size can change.

Indices and paths are encoded data, without descriptor, Signature, bound, receiver or Exceptions validation. Class/Code/Record contexts remain outside this method decoder; no semantic server feature or full annotation-context parity follows. See [the finite contract](METHOD_TYPE_ANNOTATIONS_SPEC.md).

## Resolved nominal Package aliases (#383)

`metadata::resolved_alias_signature::decode` consumes supported decoded kind 2/5,
mv 2.3.0 Package data and returns declaration names plus underlying/expanded
nominal ClassIds, effective nullable values and nullable presence. The shared
`metadata::class_id::ClassId` exposes lossless UTF-16 package/relative-class parts
and resolver local membership; the former alias-owned type path is removed.
Encoded declaration ordinals and decoded-d1 spans are provenance, not symbol IDs.
The genuine checked-in `KTypeParameterBaseKt.class.bin` supplies
`TypeParameterContainer` → `kotlin/Any` for both roles through predefined index 0,
despite empty raw d2 text. Its existing notices/provenance remain unchanged;
compiler provenance is still unverified.

This finite consumer requires one inline Type per role. Inline takes precedence
over a simultaneous table ID; duplicate singular inline messages explicitly
refuse instead of pretending to implement protobuf merging. Missing/table-only
roles, alias parameters, arguments, parameter/alias classifiers, flexible/outer/
abbreviated Types and nonzero Type flags refuse. Flat leaf Types use the existing
hard depth-2 schema budget; no recursive traversal or cap increase is introduced.
Unknown fields retain the existing opaque bounded-wire policy. Complete Package
framing is scanned on success; any error returns no partial signatures.

One existing shared schema/wire/resolver budget covers framing, complete Package
selection, leaf validation, all string/local queries, ClassId conversion and
owned output. Caller limits only tighten hard ceilings. Header/annotations/d1
decoding remain earlier independently bounded stages; already-decoded d1 plus d2
input bytes are admitted together. ClassId conversion follows pinned Kotlin
[`NameResolverUtil`](https://github.com/JetBrains/kotlin/blob/d57eb4a26ed01ab03cf195da686c8047032ff16f/core/deserialization.common/src/org/jetbrains/kotlin/serialization/deserialization/NameResolverUtil.kt#L12-L17)
and [`ClassId.fromString`](https://github.com/JetBrains/kotlin/blob/d57eb4a26ed01ab03cf195da686c8047032ff16f/core/compiler.common/src/org/jetbrains/kotlin/name/ClassId.kt#L126-L138):
split at the last slash before the first backtick, map package slashes to dots,
remove relative-class backticks, retain nested dots and local membership.
No lossy Unicode conversion, alias lookup/expansion, generics/substitution,
subtyping, module linkage, parser/server integration or official LSP parity is
claimed. The existing raw TypeAlias/Types APIs still preserve opaque occurrences;
this separate consumer does not broaden their contracts.

## Class-owned parameter property returns (#387)

`metadata::resolved_class_properties::decode` selects one decoded kind 1,
mv 2.3.0 Class internally and returns one owner context, original ordered
parameter declarations and properties whose inline bare parameter-ID return
Types bind to that owner's declaration ordinals. Class/property/parameter/Type
raw bodies borrow the same decoded d1; field/body spans are d1-relative, while
opaque bound spans stay parameter-relative. Property ordinals count all encoded
Class fields. Identity is this returned owner context plus declaration ordinal,
not the raw ID, spelling or a global ClassId. Owner ClassId and all names use the
actual resolver and retain lossless UTF-16, including lone surrogates.

The unchanged genuine kotlin-stdlib 2.3.20 `Pair.class.bin` is the consumer fixture:
`first`/`second` reference the original Class declarations `A`/`B`, not independent
name copies. Distribution provenance does not establish compiler provenance.
`TypeHeader.nullable` retains encoded marking only; absent marking on `A`/`B`
does **not** establish effective non-nullability.

All Class parameters are read before properties regardless of wire order.
IDs compare exact signed int32 values, not positions; reordered/noncontiguous/
negative IDs work and equal names do not collapse distinct declarations.
Duplicate owning IDs and unknown IDs (including unavailable enclosing scopes)
refuse. Property-owned parameters and parameter-NAME classifiers refuse:
ID lookup can reach the Class parent, but name lookup cannot leave the immediate
property scope. No spelling-based fallback is performed.

One inline return Type is required. Inline wins over an accompanying table ID;
table-only/missing returns and duplicate inline messages refuse. Arguments,
class/alias/name classifiers or classifier conflicts, flexible/outer/abbreviated
Types, nonzero effective Type flags and receiver/context forms refuse, including
wrong-wire significant fields. Existing raw required-field/scalar validators
are reused; last recognized scalars and int32 width are preserved. Bounds,
annotations, accessors and unselected Class tables remain bounded opaque data,
not interpreted semantics. Unknown fields keep the existing bounded-wire policy.
Any selected-property failure returns no partial property vector.

One monotonic schema/wire/name/output budget covers combined decoded d1+d2
admission, all scans, ID comparisons, names, shared ClassId conversion and vector
capacities. Public per-member decoders are not used to reset allowances.
Selected leaves retain the existing hard depth-2 convention, not recursive
Class→Property→Type depth-3 traversal; no cap or dependency is added. Native
class→annotations→Metadata→consumer tests are pure data processing, not Kotlin
execution. This is not general generics, type-table lookup, merging, bounds
interpretation, substitution/inference, source/Java linkage, server integration
or official Kotlin LSP parity.

The complete native binary suite passed481 cases across52 suites on the composed
source; original platform38930044300 independently passed those481 on macOS and
Windows, including all10 property-consumer and6 alias-signature cases. ROOT's
throwaway actual public-API executable observed Pair.first→original A(id0,ordinal0,
d1 declaration54..60/Type238..240) and second→B(id1,ordinal1,62..68/263..265),
with original borrowed pointers and absent nullable marking. The example source
was removed. These are native data observations, not official generics or linkage.

## Selected self-applied Class Function return (#390)

`metadata::resolved_class_function_return::decode(&outcome, class_field_ordinal,
limits)` selects one original field9/wire2 by its ordinal among **all** Class
fields, from the same decoded kind1/mv2.3.0 Outcome that authenticates the owner.
It returns the original owner, ordered declarations, selected Function header/name
and outer/body spans, nominal return header/span, and ordered raw Arguments with
projection presence, leaf headers/spans and checked owning declaration ordinals.
The shared owner/declaration types live in `metadata::class_context`; the context
extraction itself is internal and shared with the property consumer. No caller-
authored owner vector, raw Function body or method-name fallback is accepted.

The unchanged genuine Pair fixture's `copy` is Class field ordinal9 (not Function
list ordinal3), outer149..198/body151..198, return155..169. Its two invariant
Arguments reference original A(ID0, declaration54..60, ordinal0) and B(ID1,
62..68, ordinal1); leaf Types are159..161 and165..167. Declaration variance OUT
does not change argument projection: absent projection means invariant while
remaining absent in the result. Nullable Options preserve marking only.
ROOT actually executed an external native public-API consumer over this genuine
class: owner47..285, selected copy/function151..198, return155..169, both exact
original Argument/leaf/declaration pointers and ordinal bindings above passed.
The function-list ordinal3 was correctly refused; nullable/projection marking
remained absent while declarations retained OUT. Complete native binary tests
passed491 cases across53 suites on the composed source; no JVM/compiler or
parser/server/worker runtime was involved. The throwaway source was removed.

The return must resolve to the owning ClassId, with argument/declaration counts
equal. The common identical-resolver-index case reuses the authenticated owner
without allocating a second ClassId; different indices compare resolved semantic
ClassId parts, including local membership. Identity remains this context plus
declaration ordinal, not a raw ID/name, global method ID or JVM linkage.
Signed/noncontiguous/reordered IDs bind exactly; duplicate/unknown/enclosing IDs,
Function-own parameters and leaf parameter-NAME classifiers refuse.

Inline return/argument Type beats its accompanying table ID. Missing/table-only
roles, duplicate inline messages, classifier conflicts, nested leaf arguments,
aliases/flexible/outer/abbreviated Types, nonzero effective int32 Type flags and
non-invariant/unknown projections refuse atomically. Recognized scalar narrowing
and last-value semantics remain unchanged. Other Functions remain opaque; the
selected Function's receiver/value/context/contract/JVM-extension bodies are
bounded opaque data, not full signatures.

One monotonic existing schema/wire/name/output budget covers combined d1+d2
admission, complete Class framing/context, Function/return/Argument/leaf scans,
binding/comparisons, names/ClassIds and staging/final vector capacity. No public
per-argument decoder resets, repeated leaf decoding, raw-body copies or raised
depth ceilings are used: selected fixed leaves follow the existing depth2 rule.
Property/alias contracts remain unchanged. No general generic reconstruction,
substitution, bounds/type-table interpretation, source/module linkage, server
integration or official Kotlin LSP parity is claimed; #96 remains open.

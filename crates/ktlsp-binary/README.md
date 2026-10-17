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

`parse_jimage_class(&resource, &mut scratch)` consumes one private origin-bound
`ktlsp-artifacts::JimageResource`, preserving whole-image authority rather than
constructing a second verified class artifact. It invokes the existing class
reader and compares declared `this_class` with the selected module-relative
MUTF-8 `.class` path losslessly, including surrogate units. All failure paths
clear scratch. Stored attributes borrow the original image; decoded attributes
borrow scratch. [JImage format/budget/fixture details](../ktlsp-artifacts/JIMAGE.md)
remain separate from SDK selection, classpath/module semantics and server parity.

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

## Selected Class Constructor value declarations (#418)

`metadata::resolved_class_constructor_parameters::decode(&outcome,
all_class_field_ordinal, limits)` selects original Class field8/wire2, counting
**all** fields, in supported kind1/mv2.3.0 Metadata. It returns the existing
`class_context` owner and ordered original Class declarations plus the selected
Constructor's ALL-field ordinal, true outer/body d1 ranges, borrowed complete raw
body, complete raw flags/version IDs and ordered existing
`resolved_class_function_parameters::ValueParameter` values. There is no invented
constructor name, own declaration list, return type, descriptor or JVM index map.

The existing value helper/classifier/finite grammar is shared, not duplicated.
Exact signed ID lookup binds an original Class declaration ordinal. Constructor
own declarations are empty: NAME resolves the string but always refuses with
the existing `UnknownParameterName`, even when it equals a Class name. Pinned
[`MemberDeserializer.loadConstructor`](https://github.com/JetBrains/kotlin/blob/d57eb4a26ed01ab03cf195da686c8047032ff16f/core/deserialization/src/org/jetbrains/kotlin/serialization/deserialization/MemberDeserializer.kt#L274-L290)
creates that empty child scope; ID lookup recurses to its parent whereas NAME
lookup does not. `Classifier::FunctionParameter` is unreachable here.

Admission/framing/Class declarations precede selection; full raw Constructor
framing and packed version IDs precede ordered value semantics. Errors return
no partial result, wrap existing context/value failures and rebase relative
offsets exactly once to d1. A charged original-field scan preserves nonminimal
outer prefixes. One context, resolver and monotonic allowance cover d1+d2,
staging, scans/comparisons, queries and temporary/final requested capacity,
without refunds. All nine existing resource dimensions and flat-leaf depth2
remain bounded; depth1 refuses. These are logical bounds, not RSS guarantees.

Permanent genuine-data source assertions reuse unchanged licensed Pair:
Class body47..285; Constructor ALL-field ordinal5, outer76..103/body78..103,
flagsSome(70), empty version IDs; first outer80..88/body82..88/Type86..88
binds original A ID0/ordinal0/body54..60, and second88..96/90..96/Type94..96
binds B ID1/ordinal1/body62..68. Value flags/type IDs/vararg IDs/nullable marking
are absent; OUT declaration variance is not inferred nullability or projection.
ROOT's actual native public-API consumer independently observed these complete
values on source2aef0924/tree89c7302d, with empty PATH and no Kotlin/JVM runtime.
Producing compiler provenance stays **UNVERIFIED**, distinct from stdlib
distribution2.3.20. Existing fixture notices/provenance and bytes are unchanged.

Authored controls separately cover structural/semantic error order, original
nonminimal spans, empty/multiple Constructors, signed/reordered/duplicate IDs,
duplicate names, NAME refusal, nominal/predefined resolution and aggregate
exact/one-less boundaries measured by this operation's tests, not copied minima.
Inline Type beats retained type-ID; missing/table-only/duplicate inline Types,
all vararg roles, significant wrong wires, classifier conflicts, richer Types
and nonzero effective Type flags refuse. Raw Options/default bits stay data.
Annotations/plugin/extensions/version applicability, bounds/substitution,
source/JVM mapping, overloads, server integration and full parity stay outside
this operation. Existing Function/signature/raw-constructor/#394 APIs are unchanged.
Both independent OWNER+ROOT gates fully read the actual payloads, reconciled
original bytes/pointers and all228 real calls (157 full-value/pointer-equal
successes,71 complete failures), plus22 explicitly authored controls. Actual
genuine minima are bytes859/records101/nodes22/work4996/owned_bytes3200/
entries39/range_slots26/queries6/output_units24; joint exact succeeds and each
joint one-less/depth1 refuses. These are operation-specific logical allowances,
not memory/RSS or compiler-equivalence claims. All647 binary/import data checks
(640 ordinary+7 compile-fail doctests) passed without filtering. The throwaway
source was removed only after both full gates; executed source and original
output remain in `/private/tmp/ktlsp-418-native/actual-consumer-proof.json` and
`consumer.stdout`. Full hosted workspace/419 evidence and integration remain
separate gates, not inferred from this native data smoke.

## Selected Class Function value declarations (#402)

`metadata::resolved_class_function_parameters::decode(&outcome,
class_field_ordinal, limits)` selects an original field9/wire2 by its ordinal
among **all** Class fields in supported kind1/mv2.3.0 metadata. It returns the
shared `class_context` owner/original ordered Class declarations, selected Function
header/name/outer and body d1 ranges, its original ordered `type_parameters`, and
every field6 value declaration in order. Each value retains its separate value-list
ordinal, original outer/body ranges, borrowed raw flags/name index, resolver-derived
UTF-16 name, borrowed Type header and d1 range. Its classifier is a checked
`FunctionParameter`/`ClassParameter` declaration ordinal or an authentic
resolver-derived `class_id::ClassId`, not a loaded/global symbol.
All original bodies borrow the **same retained Outcome**, including declarations;
metadata ranges are not source coordinates.

`ValueParameter` flag accessors read only its retained raw flags: annotations1,
declared-default2, crossinline4 and noinline8. Unknown bits remain raw. No default
expression, inherited availability or `$default` mask mapping is inferred.
Nullable `Option<bool>` preserves absent/false/true marking; absent Class-parameter
marking does not establish semantic non-nullness. Original declaration OUT is
not a use-site projection.

Exactly one inline type3 is interpreted, taking precedence over type-ID5.
Selected Function field4 declarations are completed before values, even when
encoded later. Exact signed ID7 searches own then Class declarations; NAME9 uses
the same actual resolver and requires exactly one own resolved UTF-16 match,
never Class fallback. Duplicate IDs refuse within each scope; cross-scope equality
is valid shadowing. Unqueried duplicate names do not invalidate ID binding.
Unknown ID, unknown NAME and ambiguous NAME are distinct atomic refusals.
Missing/table-only/duplicate inline Types, significant wrong wires, conflicting
classifiers, arguments/aliases/flexible/outer/abbreviated Types, nonzero effective
Type flags and all vararg4/6 roles still refuse. Return/receiver/context/contracts/
JVM bodies, bounds and Class/Function TypeTables stay opaque. Empty value results
mean the Function actually has no values. Original reified/variance/bounds and
nullable Options remain data, not inferred semantics.

One monotonic existing d1+d2/schema/wire/name/output allowance covers complete
Class framing/context, selected Function/own declarations/bounds/value/leaf scans,
scoped ID comparisons, every inspected UTF-16 comparison unit and retained
capacity. Temporary resolved NAME queries consume output/allocation without refund.
No public child decoder resets, detached table/body inputs, secondary index/cache
or additional resolver are used. Depth remains the hard finite flat-leaf depth2,
not recursive type depth; earlier class/annotation/d1 decoding has separate bounds.

Permanent source tests reuse the unchanged licensed Pair fixture: copy at Class
ordinal9/body151..198 has first171..179/type177..179 bound to A(ID0, declaration
54..60, ordinal0), and second181..189/type187..189 bound to B(ID1,62..68,ordinal1).
Both raw flags are Some(2) and nullable marking is absent. Equals at ordinal10/
body200..220 has other208..216/type212..216, name index21 and nominal index22:
the actual resolver supplies kotlin/Any despite empty raw d2, with Some(true)
nullable marking. These are source-derived expectations, **not newly executed
smoke evidence**. Compiler provenance remains unverified. The unchanged licensed
Result$Companion fixture adds a genuine NAME9 `success` control (Class ordinal4,
local body65..69 ID1/name6, value71..77/Type75..77 `4806`) and nominal `failure`
with separate local102..106 and Throwable Type112..114. Original raw-data
observation had separate budgets and is not new aggregate binding evidence.
Genuine Function-local ID-leaf/table-ID/array/vararg coverage remains absent.
No local checks or new native smoke were run by this source owner; ROOT admits
those separately.

Enclosing scopes beyond owning Class ID lookup, table interpretation, arrays/
varargs, substitution, bounds interpretation, applicability, source-to-JVM
parameters, library symbol lookup/source attachment, server integration and full
Kotlin LSP parity remain unsupported. This extension does not broaden the sibling
property/return operations' narrower scope/NAME refusals.

## Selected values and finite scoped/nominal Function return (#411/#415)

`metadata::resolved_class_function_signature::decode(&outcome,
all_class_field_ordinal, limits)` adds a distinct composition. Its `parameters`
component is the existing `SelectedParameters`: original owner, ordered Class
and Function-own declarations, selected Function and every ordered value.
`return_type` retains original outer/body ranges, borrowed header, the existing
`resolved_class_function_parameters::Classifier`, and ordered original Argument
bodies/projection Options/leaf headers and ranges. A nominal `Classifier::Class`
contains an independently resolved `class_id::ClassId`; Function/Class parameter
variants contain checked declaration ordinals in this result, never raw IDs or
global symbols. Bare parameter roots always have no arguments, including no
wrong-wire argument occurrences. Every raw body borrows the same retained Outcome.

The return admits either a bare parameter ID/NAME or a nominal Class with zero or
more invariant inline parameter leaves. Signed ID searches Function before Class;
NAME uses the same actual resolver and unique immediate-own UTF-16 comparison as
values, without parent fallback or normalization. The nominal target is not assumed
to be the owner; argument count is not compared with owning Class declarations
because target arity is unavailable without loading its metadata. Nullable
absent/false/true and projection absent/explicit INV remain encoded presence, not
inferred meaning.

One original Class context, resolver and monotonic budget cover all declarations,
values and return, including independent nominal resolution, temporary NAME
strings, each inspected comparison unit and staged/final capacities. The private
selected-Function path is shared, not two public child calls or copied declarations.
All Class framing and all own declarations complete before values. Any selected
value/return error, late duplicate ID or malformed tail refuses atomically.
Missing/table-only/duplicate inline roles, wrong significant wires, conflicting
classifiers, nonzero effective Type flags, unknown/non-invariant projections,
nominal/recursive/richer argument leaves and varargs refuse.

Permanent source regressions use unchanged genuine licensed Result$Companion
(success4/failure5) and Pair.copy9 fixtures, with same-Outcome pointers and
separate sibling-own identities. ROOT's separately admitted real public native
consumer at reviewed2b739597/treea0c514f9 compiled0/ran0 with empty stderr/PATH.
It ACTUALLY resolved index5 to kotlin.Result, not ownerkotlin.Result.Companion:
returns55..63/92..100, NAME6 leaves59..61/96..98 bind their distinct original
ownT65..69/102..106. Pair.copy retains kotlin.Pair target and original A/B.
OWNER+ROOT fully read original class bytes/annotations/Header/d1/d2, every actual
signature/span/pointer/binding/refusal and all9 authored exact/one-below limits.
Both independently checked all56 printed pointer/raw-original slice pairs;
full output/source/proof remain `/private/tmp/ktlsp-411-412-native/`.
Only then was the throwaway source removed. All630 binary/import data tests
passed, including all11 new signature tests. Previous408 value smoke is NOT
this return evidence. Fixture custody/notices unchanged; compiler provenance
and genuine Function-own ID/shadow/ambiguous-NAME/table/richer-return emission
remain gaps, with authored controls labelled.

The #415 cutover replaces `ReturnType.class_id` outright with `classifier`;
nominal callers must match `Classifier::Class`. It stages the original Type once,
preserves Class-bearing validation/error order, and routes bare parameter roots
through the existing shared classifier. At reviewed947826/tree63289782, ROOT's
separately admitted public native consumer compiled0/ran0 with empty stderr/PATH.
It ACTUALLY bound genuine Pair component1/2 (all-Class ordinals7/8, return bodies
121..123=`3800`/138..140=`3801`) to original Class A54..60/B62..68. Full Pair.copy
and Result controls retain the nominal targets and original scoped arguments above.
Authored signed-ID/NAME/UTF-16/marking/role/atomicity controls remain labelled
separately, not represented as compiler emission.

Independent OWNER+ROOT fully read all477 actual records, including complete
original class bytes/model/annotations/Outcome, every signature,146 pointer pairs,
48 outer slices,28 scoped declaration bindings and20 complete parameter-only
comparisons. Five fully read complete success bases cover all45 exact/one-below
resource boundaries and five joint successes;45 joint one-below and five depth1
refusals retain actual typed errors/offsets. Late NAME mismatch costs255 extra
work units. All637 binary/import checks passed, including all18 signature tests.
Only AFTER both full gates was the throwaway consumer source removed; full source,
commands, output and proofs remain `/private/tmp/ktlsp-415-native/`. See the
[complete current evidence](../../docs/status/development.md#reviewed414415-composed-source-and-complete-native-data-acceptance).
Historical411 evidence remains unchanged. Genuine bare Function-root producer
emission and exact compiler provenance remain unestablished.

The parameter-only API still leaves returns opaque and has unchanged allowances;
the old self-return API still refuses Function-own declarations and retains its
self/Class-ID/count guarantees. Property/context/method-link contracts are
unchanged. Receiver/context/contracts/JVM extensions/bounds/TypeTables remain
opaque: this is not a complete Kotlin signature, target loading, substitution,
applicability, JVM linkage, symbols, server capability or full Kotlin LSP parity.
ROOT's exact reviewed-source compilation and separately admitted native data
observation do not authorize local Kotlin parser/server runtime under #96.

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

## Same-model JVM method identity (#394)

`metadata::linked_class_method::decode(&class, all_class_field_ordinal, limits)`
extracts that supplied ClassFile model's own original class annotations and
Metadata, admits kind1/mv2.3.0, completes Class framing, selects one original
Constructor(field8) or Function(field9), and joins its effective JVM identity to
exactly one original `&class.methods[method_ordinal]`. The public mutable model
is the authority, **not proof of a parse or retained archive bytes**. Consumed
model counts, CP tags/indices, UTF16 sizes, annotations and descriptors are
revalidated; foreign caller-authored Header/Outcome objects are not inputs.

Extension100 occurrences merge in encounter order with last recognized scalar
wins and signed resolver indices. An explicit descriptor is mandatory. A present
extension name wins; an absent name uses required Function.name or literal
`<init>` for constructors, following pinned Kotlin
[`JvmProtoBufUtil.kt` lines79–125](https://github.com/JetBrains/kotlin/blob/d57eb4a26ed01ab03cf195da686c8047032ff16f/core/metadata.jvm/src/org/jetbrains/kotlin/metadata/jvm/deserialization/JvmProtoBufUtil.kt#L79-L125).
Absence is not index0; omitted descriptors are not inferred. All original methods,
including bridges and overloads, participate in full lossless UTF16
name+descriptor uniqueness, including return type. Zero/duplicate tuples refuse.
Constructors require nonstatic `<init>` and void; ordinary Functions cannot bind
`<init>`/`<clinit>`. Actual access flags determine descriptor receiver slots;
metadata formal counts are not asserted equal to JVM arity.

The result borrows the original Member and actual CP name/descriptor/owner, and
owns one decoded-d1 buffer plus necessary lossless ClassId and metadata identity/
spans. `this_class` is resolved through typed Class→Utf8. Owner mapping converts
ClassId package dots to `/` and relative nested dots to `$` separately; local,
quoted or otherwise unproved mappings refuse. No reverse-dollar or flattened-name
guess establishes ownership.

One monotonic schema/wire/name/work/capacity allowance also covers annotation and
header decoding, d1+d2, complete framing, extension merging, owner/tuple comparison
and native descriptor parsing. `Limits.bytes` includes visited original attribute
payloads, decoded d1 and d2 UTF16 bytes; requested capacities exclude allocator
surplus/churn and RSS. Existing raw/property/alias/selected-return APIs retain
their separate contracts. Permanent pure-data checks cover genuine Pair Class
constructor ordinal5→method0/CP8,9 and copy ordinal9→method6/CP50,51; these tests
are source coverage, not an execution receipt. ROOT's separate public-API smoke
and native-data admission remain required. No JVM/compiler, parser/server runtime,
general Kotlin linkage/overload engine or official Kotlin LSP parity is claimed.

## Original Class inline bounds and finite nominal supertypes (#423 / #427)

`metadata::resolved_class_supertypes::decode(&outcome, limits)` is extended in
place by the composed #427 source below, with the separately admitted original
native and hosted execution receipts recorded below. It interprets ALL original
Class parameter inline bounds and Class field6/wire2 supertypes from the SAME
supported kind1/mv2.3.0 Outcome.
`ClassSupertypes` retains the original `Owner`, complete ordered original
`parameters`, flat ordered `bounds` and ordered `supertypes`. Absence of an
encoded bound never supplies implicit Any? or effective nullability.

Each `Bound` identifies its original Class declaration by checked `parameter`
ordinal and carries `ty: Supertype`. Bounds retain declaration order then encoded
bound order. Every `Supertype` carries its original outer/body decoded-d1 spans,
borrowed Type header/raw scalar Options, one resolver-derived nominal ClassId/
local membership and ordered `arguments`. Its `ordinal` counts ALL original
Class fields for a supertype or ALL original TypeParameter fields for a bound.
Repetitions remain separate; nothing is sorted or deduplicated.

Roots are bare nominal Types or nominal Types with invariant/default single-
inline Class-parameter leaves. Arguments reuse the existing signature
representation. Immediate-Class NAME lookup returns a checked original
`ClassParameter` ordinal; signed-ID lookup searches original IDs, never positions.
Callable helper callers explicitly retain their separate immediate-own NAME
scopes, including empty scopes. Root ClassIds are moved, not duplicated.

ANY selected Class supertype-ID field2, parameter bound-ID field6, argument-ID
role, mixed inline/ID representation, significant wrong wire, malformed late
parent/selected child, conflicting classifier, unsupported projection, nonzero
flags or richer flexible/outer/alias/abbreviated/nested-nominal shape refuses the
WHOLE operation. Unknown extensions, unused tables and unrelated nested
declarations remain bounded opaque custody, not semantically validated overlays.

ONE monotonic existing allowance covers combined decoded d1+d2, framing/schema,
one context/resolver, complete owner/declarations/selection, ALL bounds,
supertypes/arguments, classifier/name work and output reservations, including
repetitions. Earlier classfile/annotation/Metadata decoding has separate budgets.
No child reset/refund, copied raw metadata, recursive bound expansion or target
loading is added. These are logical work/requested-storage bounds, not CPU/RSS/
allocator guarantees. Same-Outcome custody is NOT authentication of caller-
constructible models, retained archive bytes or classfile `this_class`.
No legal inheritance, substitution, subtype/hierarchy/classpath/server or
full-parity claim follows.

### Earlier bare-only #423 execution receipt

All six independent component/union static reviews and pinned compiler-only
gates passed at9cc1823. Separately admitted unfiltered binary/import execution
passed674 checks (666 ordinary and8 privacy compile-fail), all69 complete result
rows, with zero failed/ignored/measured/filtered. All nine new checks ran.
The frozen public-API consumer executed once with empty PATH and six Java
environment variables removed: exit0, empty stderr, complete190502-byte stdout
SHA09dc0e1ed5e84cd71d2fccb13a1813191c4bbc0d7424d64540d7654fda17408e.
ROOT personally read every complete actual value and independently matched all
three original class carriers to the existing2.3.20 archive. Genuine Unit and
Result.Companion each resolve their original bare supertype to kotlin.Any;
genuine Pair refuses its abbreviation atomically at72. Its separate original
A/B declaration control and explicitly labelled abbreviation-strip mutation
preserve original declarations, not an invented genuine positive.

Actual joint exact allowance: bytes106, records30, nodes8, depth2, work1710,
owned_bytes1378, entries11, range_slots6, queries4, output_units21. Every
one-short dimension refuses; repeated children cannot reset the shared budget.
Full original spans/options/UTF16/local/raw/pointer equality and typed refusal
records remain retained. This proves this finite data operation, not target
authentication or semantic inheritance. Genuine nonempty-bound interpretation
in this public consumer remains unobserved; its positive coverage is labelled
mutated data. BOTH independent OWNER+ROOT full actual-value gates accepted
these original values. Full consumer source/executable/rlib/fixtures/outputs
and both receipts remain preserved. Cleaned reviewed0f4f043 received BOTH
complete ROOT union reviews, compiler gates and its OWN full original three-host
CI, then PR425's separate guarded integration7afdc68/tree93eee/SOLE1ddf parent.
Both feature OWNERS and ROOT independently accepted OWN original exact-main
39045022100 before lease clearance; this data scope is delivered.

### #427 accepted original values and verified main delivery

ROOT composed the complete nine-path binary-only leaf
`1a88ccc483773c148113a832f76af1e1d8f6661a`, tree
`c1c06a542b9734443e3cf50e1688c416d9fff86a`, sole verified base7afdc68.
Both independent component reviews renewed on that exact head with no remaining
findings; their earlier absent-`bv` assertion defect was repaired to preserve the
existing `[1,0,3]` default AND the absent-field presence bit. Both complete final
reports were recovered from immutable histories because direct revived-agent
URIs returned the earlier rejected review. No old review was promoted.

Portable tests and the standalone external consumer source retain COMPLETE
unchanged ClosedRange1033-byte and ClosedFloatingPointRange1194-byte carriers from
the checksummed original2.3.20 archive, with existing Kotlin/protobuf notices.
Original encoded Class-owned NAME leaves and generic bound/supertype shapes were
fully interpreted by the native public consumer, with labelled mutation controls.
Selected original TypeTable-ID evidence remains unobserved.

The source includes whole-operation exact/joint-one-short/cumulative resource
and original pointer/span/Options/NAME/scope controls. BOTH OWNER+ROOT accepted
the complete separately admitted c03523a native public-pipeline and standalone
consumer outputs, every original raw CP/metadata/name field and all actual
minimum/one-short/isolated/repeated values. Both independently reviewed full-union
source and compiler gates passed; capture-bearing `4b3429c` passed its OWN
original full Linux/macOS/Windows CI. Both of that head's full platform Class
payloads match the accepted native values, including73 original same-buffer
pointer pairs per host; at that capture-bearing head all814 binary files and
build inputs are byte-identical to the accepted native source. Linux default
capture does not expose those full
payloads; no new local r6 native execution is invented.

Observation-only retirement followed separate admission after BOTH current
OWNER+ROOT actual-value gates. All genuine producer/consumer calls, measured
minimum probes, mutations/restoration, borrowing and substantive assertions
remain in the shared proof and separate13-test mutation target. Standalone
executes the shared genuine/resource proof, not that separate registered target.
The cleaned union passed its OWN complete independent source reviews, compiler
gates and original full Linux/macOS/Windows CI, then guarded PR429 integration at
main `21aab03`. All THREE feature OWNERS+ROOT independently accepted the original
exact-main full CI before the exclusive original-base lease cleared. Current
unprinted Class values are carried from the accepted original complete native/
hosted gate through unchanged production and preserved real calls/assertions;
they are not fabricated new main output. Earlier #423 values stay historical.
No local parser/server/Session runtime is authorized. [Exact delivery and limits](../../docs/status/development.md#verified429-cleaned-source-integration-and-original-exact-main-acceptance).


# Bounded classfile reader (T017a / #58)

`parse_class(&[u8]) -> Result<ClassFile<'_>, Error>` reads classfile structure as data, using only Rust's standard library. No existing server caller or capability uses it yet. All fixtures in `tests/classfile.rs` are independently authored Rust byte builders, with no copied third-party code or compiler-produced artifacts. No JVM/compiler/oracle was run.

Supported version subset: 45.0–45.3 and 46.0–67.0 (through Java 23). Preview minors, nonzero minors for majors 46–67, newer and older versions return `UnsupportedVersion`. This deliberately excludes some historically valid versions. New CP tags are gated at 51 (method handle/type/invokedynamic), 53 (module/package), and 55 (dynamic). Unknown tags return `UnsupportedTag`.

The result preserves raw access flags, CP indices, interface/member order, and borrowed opaque attribute payloads. CP zero/wide second slots are `Constant::Unusable`. Integer and floating constants preserve their bits. `utf16(index)` exposes modified UTF-8 losslessly as UTF-16 code units, including encoded NUL and isolated surrogates; callers decide whether conversion to a Rust string is appropriate.

Validation covers structural typed CP references, method-handle kinds/targets, class/member names and descriptors, array depth (255), parameter slots (255, including receiver for declared instance methods), attribute name indices and lengths, and exact EOF. It does not resolve external classes. Special method names have descriptor/handle restrictions. Attribute names are validated as UTF-8 pool references; their text and contents are otherwise opaque.

Budgets are checked before reading/adding records: 16 MiB input; 65,535 aggregate CP slots (including slot zero and wide holes), interfaces, fields, methods and attributes; 64 Mi UTF-16 units of referenced name/descriptor validation. Shared long-name references consume this work budget again and can return `Limit("validation work")`. Reading and validation are iterative; output allocation is bounded by these budgets and decoded strings by input bytes. Attribute bytes are borrowed, never copied or interpreted. Numeric reads use checked slices before fixed-size conversion. Errors distinguish truncation, malformed structure, unsupported versions/tags and resource limits.

Success is **not JVM verification**. Attribute interiors (Code, BootstrapMethods, Signature, annotations, nesting, etc.) and their internal CP/bootstrap indices are unchecked. Access-flag combinations, duplicate declarations, module structural constraints, linkage and bytecode validity are deferred. Generic signatures, annotation/nesting interpretation and Kotlin metadata need separate children; parent #17 remains open. No Java semantic or Kotlin LSP parity is claimed.

Format contract: [JVMS Java SE 23 chapter 4](https://docs.oracle.com/javase/specs/jvms/se23/html/jvms-4.html). This reader implements only the subset above.

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

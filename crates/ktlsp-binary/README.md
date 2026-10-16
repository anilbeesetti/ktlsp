# Bounded classfile reader (T017a / #58)

`parse_class(&[u8]) -> Result<ClassFile<'_>, Error>` reads classfile structure as data, using only Rust's standard library. No existing server caller or capability uses it yet. All fixtures in `tests/classfile.rs` are independently authored Rust byte builders, with no copied third-party code or compiler-produced artifacts. No JVM/compiler/oracle was run.

Supported version subset: 45.0–45.3 and 46.0–67.0 (through Java 23). Preview minors, nonzero minors for majors 46–67, newer and older versions return `UnsupportedVersion`. This deliberately excludes some historically valid versions. New CP tags are gated at 51 (method handle/type/invokedynamic), 53 (module/package), and 55 (dynamic). Unknown tags return `UnsupportedTag`.

The result preserves raw access flags, CP indices, interface/member order, and borrowed opaque attribute payloads. CP zero/wide second slots are `Constant::Unusable`. Integer and floating constants preserve their bits. `utf16(index)` exposes modified UTF-8 losslessly as UTF-16 code units, including encoded NUL and isolated surrogates; callers decide whether conversion to a Rust string is appropriate.

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
The separate native pure-data consumer is reproducible with
`cargo run --locked -p ktlsp-binary --example descriptor_smoke`; it reads the
retained Builder classfile and checks borrowed fields, parameters, return/void,
primitive/array structure and explicit malformed-input failure without Java.

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

## Encoded method type annotations

`method_type_annotations::decode` composes declaration/extension/field data with all six legal method targets (0x01, 0x12, 0x14, 0x15, 0x16, 0x17), sharing one annotation budget. `target_type` retains the raw tag and `target_info: MethodTargetInfo` retains fixed-width indices. **Breaking API change:** `formal_parameter_index` was removed; formal-parameter data is now `MethodTargetInfo::FormalParameter(index)`. External field reads/literals/patterns and derived Debug/layout/size can change.

Indices and paths are encoded data, without descriptor, Signature, bound, receiver or Exceptions validation. Class/Code/Record contexts remain outside this method decoder; no semantic server feature or full annotation-context parity follows. See [the finite contract](METHOD_TYPE_ANNOTATIONS_SPEC.md).

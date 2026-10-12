# Bounded classfile reader (T017a / #58)

`parse_class(&[u8]) -> Result<ClassFile<'_>, Error>` reads classfile structure as data, using only Rust's standard library. No existing server caller or capability uses it yet. All fixtures in `tests/classfile.rs` are independently authored Rust byte builders, with no copied third-party code or compiler-produced artifacts. No JVM/compiler/oracle was run.

Supported version subset: 45.0–45.3 and 46.0–67.0 (through Java 23). Preview minors, nonzero minors for majors 46–67, newer and older versions return `UnsupportedVersion`. This deliberately excludes some historically valid versions. New CP tags are gated at 51 (method handle/type/invokedynamic), 53 (module/package), and 55 (dynamic). Unknown tags return `UnsupportedTag`.

The result preserves raw access flags, CP indices, interface/member order, and borrowed opaque attribute payloads. CP zero/wide second slots are `Constant::Unusable`. Integer and floating constants preserve their bits. `utf16(index)` exposes modified UTF-8 losslessly as UTF-16 code units, including encoded NUL and isolated surrogates; callers decide whether conversion to a Rust string is appropriate.

Validation covers structural typed CP references, method-handle kinds/targets, class/member names and descriptors, array depth (255), parameter slots (255, including receiver for declared instance methods), attribute name indices and lengths, and exact EOF. It does not resolve external classes. Special method names have descriptor/handle restrictions. Attribute names are validated as UTF-8 pool references; their text and contents are otherwise opaque.

Budgets are checked before reading/adding records: 16 MiB input; 65,535 aggregate CP slots (including slot zero and wide holes), interfaces, fields, methods and attributes; 64 Mi UTF-16 units of referenced name/descriptor validation. Shared long-name references consume this work budget again and can return `Limit("validation work")`. Reading and validation are iterative; output allocation is bounded by these budgets and decoded strings by input bytes. Attribute bytes are borrowed, never copied or interpreted. Numeric reads use checked slices before fixed-size conversion. Errors distinguish truncation, malformed structure, unsupported versions/tags and resource limits.

Success is **not JVM verification**. Attribute interiors (Code, BootstrapMethods, Signature, annotations, nesting, etc.) and their internal CP/bootstrap indices are unchecked. Access-flag combinations, duplicate declarations, module structural constraints, linkage and bytecode validity are deferred. Generic signatures, annotation/nesting interpretation and Kotlin metadata need separate children; parent #17 remains open. No Java semantic or Kotlin LSP parity is claimed.

Format contract: [JVMS Java SE 23 chapter 4](https://docs.oracle.com/javase/specs/jvms/se23/html/jvms-4.html). This reader implements only the subset above.

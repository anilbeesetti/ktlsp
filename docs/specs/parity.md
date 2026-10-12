# Native Kotlin LSP parity contract

## Target and evidence

Baseline: official `kotlin-lsp/v263.6379.0`, release source `4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748`. Research also observes master `5c707583b37f43f9d54a85f823b2bb5f8182999c`; master-only features are follow-up scope, not silently attributed to the release. The release binary and public mirror need not expose identical implementation details. Source presence establishes a contract, not successful runtime behavior.

Parity means correct observable behavior on a published fixture corpus, not matching method names or returning empty success responses. A green unit-test suite alone never establishes official parity. Until a recorded differential run exists, each implemented feature is native-tested only.

## Rust-only boundary

All first-party production and test implementation is Rust. Runtime and build have no JVM/JDK, Kotlin compiler process, JNI bridge, bundled Java runtime, C/C++ parser, or hidden Gradle/Maven invocation. Transitive native build scripts are audited before adoption. Reading `.class`, JAR, Kotlin metadata, XML, POM, Gradle files, sources, and exported project models is permitted as data. Rust toolchain/linker/platform libraries are infrastructure, not an alternate-language language-analysis engine.

The user explicitly permits an isolated JVM official-server oracle; it must never enter runtime, build, required CI, or release archives. No oracle is presumed available. Preinstalled Java in the development host does not establish a server dependency.

## Required baseline features

1. LSP lifecycle, stdio framing, document synchronization, versioned edits, URI handling, cancellation, progress, workspace configuration and file changes.
2. Completion, completion resolution and additional/import edits; hover with KDoc and bounded type parameters; definition, type definition, implementation, references, document highlights.
3. Document/workspace symbols including library search, semantic tokens, folding ranges, inlay hints, signature help, call hierarchy, Kotlin main Run/Debug code lenses with initialization-option gating.
4. Kotlin compiler-level diagnostics, IntelliJ inspections and library-aware quick fixes; safe code actions including applicable intentions, extract variable and inline local variable.
5. Rename with prepare/validation, conflict handling, safe references and versioned workspace edits; move files/packages with package/import rewrites and failure reporting.
6. Full and range formatting (typing/on-type contracts remain unconfirmed), organize imports, respect settings and syntax preservation.
7. Gradle/Maven JVM and experimental Android project model compatibility: modules, source sets, generated sources, dependency scopes, language/API versions, SDK symbols, library sources and reload. Registered-source JPS `.idea`/`.iml` import compatibility is a separately tracked, runtime-unverified baseline requirement.
8. Relevant official custom LSP/command contracts, including import status and refactoring follow-up interactions. Run/debug/test integration is inventoried separately because actual JVM program execution conflicts with the Rust-only runtime boundary; it is not silently excluded from the gap report.

KMP is not required by this baseline because the official README lists it as future work. Master-only contracts are tracked separately. Exact baseline method inventory lives in the research report.

## Protocol requirements

- UTF-16 positions by default. UTF-8 offsets cannot be used as LSP characters. Invalid boundaries, reversed ranges and surrogate splits return an error without mutating the document.
- Apply incremental changes sequentially and atomically per notification; reject stale versions. Preserve CRLF and Unicode exactly.
- Parse failure = -32700; invalid JSON-RPC request = -32600; unknown method = -32601; invalid parameters = -32602; pre-initialize requests = -32002. Preserve numeric/string request IDs.
- Initialize once; shutdown responds with null; exit after shutdown succeeds, exit before shutdown fails. stdout contains only framed protocol messages; logging uses stderr.
- Unsupported providers are omitted from capabilities and produce method-not-found, not fabricated successful results. Optional client capabilities must be respected.
- Bound framing, document size, traversal, nesting and query work. No panic on hostile/malformed text; cancellation and stale snapshots must not publish misleading edits or diagnostics.

## Project model limits

Gradle build logic is executable code. A static importer cannot reproduce arbitrary scripts/plugins and dependency resolution. A native explicit project model and limited declarative import can support useful projects, but **are not full Gradle parity**. All unresolved/dynamic settings must be visible as import gaps. Full build-system parity remains blocked until a Rust-native evaluation strategy or an explicitly permitted externally produced model has verified coverage.

Java SDK classes can be indexed from user-provided class/source artifacts without launching Java. Kotlin bytecode metadata and mixed Java source semantics must be implemented before dependency-aware feature parity can be claimed. Release-confirmed Kotlin compiler plugins such as serialization and AllOpen (Compose support requires additional research) and IntelliJ intention coverage need their own fixtures and specifications.

## Release gate

Every required matrix row must have a fixture, reproducible oracle/evidence, correct native result, edge-case regression coverage, and independent review. Hard gates: no runtime/build JVM; all native source dependencies compliant; safe edits; malformed-input crash corpus; transport/editor integration; Linux/macOS/Windows release builds; public residual-gap report. Performance thresholds and oracle normalization rules must be published before benchmarking. No release may describe full parity while a required row is pending, partial, blocked, or unverified.

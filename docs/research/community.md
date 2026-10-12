# Community server and native Kotlin feasibility research

Later evidence/policy update: [isolated checksummed binary observations](oracle/README.md) now exist, including runtime/source discrepancies. The user permits optional invocation of their existing JDK/build tools, with no bundled Java; [current boundary](../specs/runtime-boundary.md) supersedes earlier strict-zero-Java execution conclusions below. Source inventory and native-engine findings remain valid.

Research date: 2026-10-12. All findings below are based on inspected primary-source snapshots; upstream fidelity/performance claims are distinguished from verified results. No JVM/JDK was installed or launched.

## Source snapshots

| Project | Inspected commit | License / role |
| --- | --- | --- |
| [winlogon/ktlsp](https://codeberg.org/winlogon/ktlsp/src/commit/ffa618469e1ddab9d0492cb345340af2251a4b40) | `ffa618469e1ddab9d0492cb345340af2251a4b40` | MIT; JVM community reference server |
| [Kotlin/kotlin-spec](https://github.com/Kotlin/kotlin-spec/tree/0f762a2314a9304e4b3fc386b1aceef1d56c7e4c) | `0f762a2314a9304e4b3fc386b1aceef1d56c7e4c` | Apache-2.0; language specification and ANTLR grammar |
| [fwcd/tree-sitter-kotlin](https://github.com/fwcd/tree-sitter-kotlin/tree/1852ea17b7f60fb3f9d84e0b1555d56b46b39fb1) | `1852ea17b7f60fb3f9d84e0b1555d56b46b39fb1` | MIT; **C** parser/scanner behind Rust bindings |
| [vyfor/kotlin-parser](https://github.com/vyfor/kotlin-parser/tree/c69140d38e727b1644811f85b1a238379f4136ff) | `c69140d38e727b1644811f85b1a238379f4136ff` | MIT; unfinished Rust parser |
| [Hexay/ktrs](https://github.com/Hexay/ktrs/tree/a703f88d6d4ac9d983a17df66ddf6ac403546c00) | `a703f88d6d4ac9d983a17df66ddf6ac403546c00` | MIT OR Apache-2.0 declarations; includes Apache upstream ports and NOTICE |
| [MenkeTechnologies/kotlinrs](https://github.com/MenkeTechnologies/kotlinrs/tree/0619364ce2ce3a59a69e3a517a9282fdb79ad928) | `0619364ce2ce3a59a69e3a517a9282fdb79ad928` | MIT; partial Kotlin runtime/compiler, custom fusevm backend |
| [JetBrains/kotlin](https://github.com/JetBrains/kotlin/tree/4974e980ac9c7f0d72b6a5fd4a1f851eb6f7871b) | `4974e980ac9c7f0d72b6a5fd4a1f851eb6f7871b` | Apache-2.0; compiler/metadata source reference (HEAD pinned; metadata implementation not fully inspected) |

## Community server: useful behavioral reference, incompatible runtime

The README explicitly identifies this as a community-maintained `fwcd/kotlin-language-server` fork, separate from official Kotlin/kotlin-lsp. It requires Java 21+. `server/src/main/kotlin/org/javacs/kt/compiler/Compiler.kt` uses KotlinCoreEnvironment, IntelliJ PSI, TopDownAnalyzerFacadeForJVM, BindingContext, lexical scopes and Kotlin compiler internals. Its incremental expression mechanism recompiles a tiny artificial file inside a cached compiler environment. These algorithms can inform designs, but importing these libraries or translating only the LSP glue does not yield a Rust implementation of Kotlin analysis.

`KotlinLanguageServer.kt` advertises incremental document synchronization, workspace folders, inlay hints, hover, folding, rename/prepareRename, completion, signature help, definitions/declarations/type definitions/implementations, symbols, references, semantic tokens, code actions, formatting/range/on-type formatting, commands, highlights, call/type hierarchy and code lens. The feature guide additionally documents jar contents, build output location, main class and override member extension requests. This is a reference inventory, not the official parity target: the official server's supported surface must be pinned separately.

Dependency resolution uses custom executable classpath scripts, Gradle Tooling API and Gradle CLI, Maven CLI, local-cache fallback, persistent build-file caches and Kotlin stdlib discovery. Formatting uses a JVM ktfmt port; decompilation uses Fernflower. Those dependencies are excluded under no JVM/JDK.

### Reusable tests

There are 46 Kotlin test source files under `server/src/test/kotlin` (including fixtures/helpers). Useful protocol/behavior groups are completion, hover, diagnostics, signature help, definitions/declarations/type definitions, references, rename, highlights, symbols, semantic tokens, hierarchy, folding, inlay hints, formatting, imports, override members, main class, J2K, workspace cache, scripts, external documentation, JDK/source archive locations and classpaths.

`CompletionsTest.kt` includes extension functions, stdlib members, Java properties/getters/setters and statics, nullable selectors, nested/companion classes, visibility, auto-imports, constructors, trailing lambdas, backquoted identifiers and malformed cursor positions. Rename fixtures cover implicit `it`, explicit lambda parameters and cross-file declarations/usages. These are meaningful later regression cases. Port test *intent* into native JSON-RPC fixtures; preserve MIT copyright/license if copying substantial fixture contents. Existing JVM tests cannot be run natively without adapting their harness.

## Parsing choices

### Tree-sitter excluded by the strict implementation-language requirement

The tree-sitter-kotlin `bindings/rust/build.rs` invokes `cc::Build` on `src/parser.c` and `src/scanner.c`. A Rust crate wrapper does not make this Rust-only. Do not quietly add it to the product. Its grammar/corpus remain useful reference material under MIT.

### kotlin-parser is not a production starting point

`src/lib.rs` comments out the actual parse module and `src/parser.rs` contains only commented-out implementation. Its Cargo description says WIP. It is not a functional full-file parser to adopt.

### ktrs: strongest native syntax and formatting candidate, with measured hazards

The workspace contains Rust lexer, parser, lossless syntax tree, PSI adapter and formatter crates. `ktrs-parser` describes a port of Kotlin compiler parsing on a reimplementation of IntelliJ PsiBuilder. Its API is explicitly unstable. `kt-syntax::KOTLIN_VERSION` says `2.4.20`; this must be matched against the official server's chosen Kotlin language version rather than treated as automatic parity.

Published crates observed through crates.io API on the research date: `ktrs-parser`, `ktrs-lexer`, `ktrs-syntax`, `ktrs-fmt` all version 0.5.1, published 2026-10-09. The public `kt-syntax` facade currently returns 404 at crates.io; pinned git can select the internal crates. Pin exact crate versions/checksums or the inspected git revision, retain lockfile and audit upgrades.

Verified normal dependency tree:

```
ktrs-parser -> ktrs-lexer -> ktrs-syntax -> text-size 1.1.1
ktrs-fmt -> ec4rs 1.2.0 + ktrs-{lexer,parser,psi,syntax}
```

These normal dependencies are Rust. **Do not use** the root CLI (mimalloc -> C), the `kt-syntax` facade (stacker helper), Java integration modules, or custom-rule-set fallback that invokes Java. Upstream tests have a dev dependency on mimalloc that builds C; that is a research-only test dependency and must not be copied into the native product/test graph.

`parse_file(text, FileKind)` requires CRLF normalized to LF; maintain an original-to-normalized offset mapping or avoid silent corruption of edits/UTF-16 positions. `Parse` owns `Rc<Tree>` so it is not automatically safe to send across analysis threads. Tree builders contain unsafe vector writes and invariant panic sites. Treat parser input as editor-controlled, use owned single-worker snapshots, check full token coverage, and audit upstream unsafe code.

Upstream's own `tests/backtracking_memory.rs` explicitly states that nested type-argument backtracking is exponential in time. It only asserts bounded memory. `tests/missed_tokens.rs` demonstrates `{fun<)]<T:@( {})` loses tokens by design to emulate IntelliJ parser behavior. A lossless editor source model cannot use parser-produced text as authoritative in this case. Ordinary syntax errors must become diagnostics, and malformed input must preserve the original document.

Verified native upstream checks: `cargo test -p ktrs-parser -p ktrs-lexer -p ktrs-syntax --locked --lib --tests` passed 31 active tests and ignored 2 oracle/corpus tests. One optional corpus test returned early without corpus data, so this is not a full real-world corpus or official semantic comparison. The two-input backtracking-memory test took 5.87 seconds in debug; sufficiently small malformed input can block request handling.

An independent pure-Rust release-mode consumer at `/workspace/research/ktrs-evaluation` reproduced the issue: 14 short malformed/Unicode fixtures produced no unwind panics but one lost-token tree; nesting `(a<` at depths 8/10/12 produced 32/38/44-byte documents taking approximately 3/28/264 ms. This is enough to reject unguarded parsing as the request-loop default. Direct internal parser entry points bypass the unpublished `kt-syntax` facade's nesting/stack guard; that facade itself uses `stacker`, excluded by strict Rust-only dependency policy. A large-stack Rust thread avoids some stack overflow but does not bound exponential work or enable forcible cancellation.

Adoption recommendation: reuse the lexer sooner if its isolated safety and position contracts pass. Consider the parser behind a supervised native worker with input/nesting/work limits, parser coverage checks and cancellation. An asynchronous timeout alone cannot stop an uncooperative parser running in-process. For the first deliverable, a safe bounded structural parser is defensible; identify it honestly as syntax-only and do not advertise semantic parity. A faithful parser is only one component of semantic analysis.

Formatting: `ktrs-fmt` is a Rust port of ktfmt 0.65 plus google-java-format 1.23.0 and provides Meta/Google/Kotlinlang options. Its README claims byte-identical formatting for 6,121 real files and recognizes grammar/version limits, but that claim was not independently checked against a JVM oracle here. Inspect the official target formatter behavior/settings before selecting defaults. Never report formatting parity from a parser-only smoke test.

License: preserve ktrs NOTICE and relevant upstream Apache-2.0/MIT notices for copied/ported code; root dual-license declarations do not erase notice obligations of Kotlin/IntelliJ/ktfmt/google-java-format ports.

### kotlinrs is a subset runtime, not Kotlin/JVM analysis parity

README labels its implementation as an M0 subset with coarse inferred primitive types and a fusevm bytecode backend. Cargo includes a VM/JIT/AOT/FFI compiler graph. It has an LSP mode, but no JVM .class support and no demonstrated complete Kotlin compiler-equivalent analysis. Reusing its runtime is unnecessary for this language server; cherry-picking syntax ideas would still require a completeness/license audit.

## Language and semantics sources

Kotlin spec `grammar/src/main/antlr/KotlinLexer.g4`, `KotlinParser.g4`, `UnicodeClasses.g4` supply language grammar. `docs/src/md/kotlin.core` describes type system, inference, constraints, overload resolution, control/data-flow analysis, inheritance, declarations, expressions, operators, scoping, annotations and coroutines. The introduction explicitly warns the spec is experimental and that platform behavior differs; it is not a complete Kotlin/JVM/K2 implementation contract. The grammar's default generated artifacts and harness are Java/JAR, so use source rules/reference corpora rather than generated runtime.

Semantic parity requires a real native engine for packages/imports/module visibility; scopes and binding; Kotlin type/nullability/variance model; generic constraints, substitution and inference; extension/member/implicit receiver overload selection; flow-sensitive smart casts; operators/delegation/destructuring conventions; lambdas/function types; annotations; expect/actual/platform semantics as supported by the target; Java interoperability; source/library indexes; symbol-accurate refactors and compiler-equivalent diagnostics. Token scanning and method handlers alone cannot deliver these behaviors.

## JVM class and Kotlin metadata without a JVM

Reading JAR/ZIP/class bytes is feasible in Rust and does not require executing JVM code. Candidate primary-source repositories include [Palmr/classfile-parser](https://github.com/Palmr/classfile-parser/tree/9866243a07f03e1e947930faa6e13f2b899be4b5) and [staktrace/cafebabe](https://github.com/staktrace/cafebabe/tree/c24a69ee5e420919d7a53c012370ac2b2c35e718). These were identified, not yet audited for modern attributes/security/licensing in this research pass.

Full library semantics require Java generic Signature parsing, annotations and nullability, descriptors, nested classes, visibility, source locations plus Kotlin `@Metadata` decoding. Kotlin metadata preserves information erased from JVM descriptors: Kotlin nullability/types/type parameters, extension receivers, top-level/package parts, properties, suspend/default parameters, type aliases and declaration flags. Plan native protobuf/bit encoding/string table/name resolver decoding and explicit metadata version compatibility. A crates.io search found no dedicated `kotlin-metadata` crate; that search is evidence of an ecosystem gap, not proof none exists anywhere. Port/reference official Kotlin metadata schemas with appropriate Apache notices and test against precompiled fixture bytes checked into the native test suite.

A JDK-free product can consume existing class/source archives as data, but cannot magically infer the entire Java standard library. Choose a legally redistributable preindexed API-signature corpus or allow users to point at existing archives; neither installs nor executes a JDK. Synthetic handfuls of stdlib classes are useful temporary fixtures but must be identified as incomplete. Native decompiled source display can initially be signature stubs; full Kotlin/Java body decompilation is a separate large effort if the official target requires it.

## Project import: unavoidable boundary under no JVM/JDK

Gradle Kotlin/Groovy build scripts and plugins are arbitrary executable programs. Static reading of `build.gradle(.kts)` cannot faithfully reproduce source sets, dependency graphs, plugin-generated code and conditional behavior for unrestricted projects. Implementing Gradle's JVM/plugin environment in Rust would recreate far more than an LSP and violate the intended no-JVM design if JVM execution were reintroduced. Maven also has inheritance/profiles/plugins/environment-sensitive execution; simple POM parsing is not equivalent to build execution.

The native server can support standalone sources, a declarative native project manifest with modules/source roots/language settings/platform/API archives/dependency edges, import of precomputed model files, version catalogs/lockfiles/cached artifact metadata and conservative known build patterns. It must report unavailable/incomplete project models explicitly. Resolve artifacts and metadata natively where possible. If a separately executed build exporter is later allowed, that changes the stated end-to-end constraint and needs an explicit user decision; do not silently execute gradlew/mvn/java now.

## Deliverable gates

1. Enforce no JVM/JDK execution or packaging and Rust-only normal dependency/build graphs.
2. Preserve original source text and UTF-16 position correctness through CRLF and astral Unicode.
3. Bound work for malformed syntax, nesting, enormous files and zip/class metadata inputs; recover per request/document.
4. Advertise each LSP capability only when actual supported semantics and cancellation behavior have evidence.
5. Maintain separate syntactic/semantic/project/library/formatting parity rows and independently expected fixtures.
6. Require symbol identity for cross-file references and rename; textual replacement is not a semantic substitute.
7. Distinguish verified tests from upstream marketing claims and unavailable official/JVM differential oracles.

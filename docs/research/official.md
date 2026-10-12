# Official Kotlin LSP research baseline

Later evidence/policy update: [isolated checksummed binary observations](oracle/README.md) now exist, including runtime/source discrepancies. The user permits optional invocation of their existing JDK/build tools, with no bundled Java; [current boundary](../specs/runtime-boundary.md) supersedes earlier strict-zero-Java execution conclusions below. Source inventory and native-engine findings remain valid.

Research date: 2026-10-12. This is a source/release-metadata inventory, **not a runtime parity certificate**. The official server was not built, downloaded, or executed during this research. No initialize response or actual diagnostic/completion output has been captured.

## Pinned evidence and version policy

- Latest GitHub release observed through `GET /repos/Kotlin/kotlin-lsp/releases?per_page=5`: [`kotlin-lsp/v263.6379.0`](https://github.com/Kotlin/kotlin-lsp/releases/tag/kotlin-lsp/v263.6379.0), published 2026-10-03T14:35:13Z, `prerelease: false`; changelog names VS Code extension 0.0.13. GitHub release assets array is empty: binaries are linked to JetBrains download hosting in release text.
- Release tag directly references commit [`4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748`](https://github.com/Kotlin/kotlin-lsp/commit/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748), dated 2026-09-29, subject `LSP-1914 [vscode] force IPv4 stack for server under WSL`. Source export: `/workspace/research/kotlin-lsp-release-source`.
- Current default-branch checkout references [`5c707583b37f43f9d54a85f823b2bb5f8182999c`](https://github.com/Kotlin/kotlin-lsp/commit/5c707583b37f43f9d54a85f823b2bb5f8182999c), dated 2026-10-10, subject `LSP-1964 [lsp] find workspace symbol on resolve by stub id`. Checkout: `/workspace/research/kotlin-lsp`.
- Saved raw metadata: `/workspace/research/kotlin-lsp-releases.json`, `/workspace/research/kotlin-lsp-release-ref.json`.
- Release binary baseline is v263.6379.0. Public source at its tag is separate evidence; binary/source correspondence and advertised capability values still require a downloaded, checksummed runtime transcript. Default-branch additions must not silently become claims about this release.
- Binary candidate: https://download.jetbrains.com/language-server/kotlin-server/263.6379.0/kotlin-server-263.6379.0.tar.gz and adjacent `.sha256` (Linux x64). Other distributions cover macOS/Linux/Windows x64 and arm64. Release download/changelog evidence: [current RELEASES.md](https://github.com/Kotlin/kotlin-lsp/blob/5c707583b37f43f9d54a85f823b2bb5f8182999c/RELEASES.md).

## Architecture and replication constraints

[README](https://github.com/Kotlin/kotlin-lsp/blob/5c707583b37f43f9d54a85f823b2bb5f8182999c/README.md) describes IntelliJ IDEA plus IntelliJ Kotlin Plugin implementation and proprietary JetBrains Air/Fleet parts. It is partially closed-source and an alpha product; this repository is a read-only mirror with documentation-only contribution support. Public modules expose feature APIs, IntelliJ-based providers, workspace importers, and client code. Core server routing, initialize capability assembly, analyzer implementation, persistence implementation, debugger, and the complete IntelliJ compiler/inspection behavior cannot be reconstructed from this mirror alone.

[api.features BUILD.bazel](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/api.features/BUILD.bazel) depends on absent `//language-server/analyzer`, `api.impl.analyzer`, and `api.core.util`, plus `@community` IntelliJ/Fleet and JVM libraries. The checkout has no complete standalone root Bazel workspace. Do not promise that public Kotlin tests can be run independently from this mirror.

Public source [LICENSE.txt](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/LICENSE.txt) is Apache-2.0; the distributed VS Code extension has separate [JetBrains Free Plugin License](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/kotlin-vscode/LICENSE.txt). Preserve provenance for any reused fixtures/source. Prefer independently written Kotlin fixtures and externally observable protocol behavior for Rust implementation. Do not bundle official proprietary binaries into the product.

Full semantic equivalence means replacing Kotlin compiler analysis (name lookup, type inference, overload resolution, nullability/data flow, generics, Java interop, metadata, compiler plugins) and IntelliJ inspections/refactorings, not translating LSP method names. Pure Rust is viable as a long-term compiler-analysis program; a lexical parser or tree-sitter wrapper cannot satisfy this goal alone.

Zero JVM/JDK runtime has an unavoidable effect: arbitrary Gradle Kotlin/Groovy scripts, Maven plugins/code generators, JVM program execution, and JDWP target execution are normally JVM tasks. A Rust server can analyze imported/exported models and read JAR/class/Kotlin metadata without launching Java. It cannot honestly claim arbitrary native Gradle/Maven execution or JVM run/debug parity solely by reading build scripts. The architecture/spec must represent this gap explicitly and either require user-provided resolved metadata or implement a much larger native build/JVM compatibility layer. The user permits an isolated official JVM oracle for research; shipped Rust product runtime/build must stay JVM/JDK-free. Oracle permission does not authorize a JVM dependency in production. External-JDK invocation is not an acceptable product shortcut under the strict Rust-only/no-JVM requirement.

## Release feature and protocol inventory

Evidence classes: **R** release notes explicitly claim behavior; **P** tag-pinned provider/client source exposes it; **U** exact router/capability/runtime behavior is unverified. All entries below carry U until runtime capture. The tag-pinned [Kotlin configuration](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/features-impl/kotlin/src/com/jetbrains/ls/api/features/impl/kotlin/configuration/LSKotlinLanguageConfiguration.kt) registers most of the providers. [Feature API directory](https://github.com/Kotlin/kotlin-lsp/tree/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/api.features/src/com/jetbrains/ls/api/features) exposes corresponding contracts.

| Family | Expected wire methods/behavior | Evidence and acceptance implications |
| --- | --- | --- |
| Lifecycle and synchronization | `initialize`, `initialized`, `shutdown`, `exit`; `textDocument/didOpen`, `didChange`, `didSave`, `didClose`; external file changes; cancellation/progress | Necessary LSP/client lifecycle; exact sync kind, position encoding, registrations, save payload and cancellation policy U. Do not fabricate official capability JSON. |
| Completion | `textDocument/completion`, `completionItem/resolve`; context-aware IntelliJ completion, auto imports, snippets/live templates, post-insert edits | R/P; Kotlin provider sets `supportsResolveRequest=true`, command `jetbrains.kotlin.completion.apply`. Test incomplete code, shadowing, imports, overloads, generics, Java members and escaped identifiers. |
| Diagnostics | `textDocument/diagnostic` provider input; compiler diagnostics, syntax errors, inspection diagnostics and quick fixes | R/P; provider uses `DocumentDiagnosticParams`; public TODO says other `DocumentDiagnosticReport` parts unhandled. Pull vs push advertisement, identifiers, resultId and refresh behavior U. Compiler and inspection diagnostic catalogs must be inventoried using runtime fixtures. |
| Code actions | `textDocument/codeAction`, `workspace/executeCommand`; intentions, diagnostic fixes, import actions | R/P; source actions include organize imports; most IntelliJ Kotlin intentions now exposed. Not an exhaustive finite catalog in this mirror. `codeAction/resolve` advertisement is U: source often supplies commands/deferred ModCommand actions instead. |
| Navigation | `textDocument/definition`, `typeDefinition`, `implementation`, `references`, `documentHighlight` | R/P; source/JAR/JDK navigation and Kotlin/Java cross-language references; type-alias references specifically fixed in releases. Test lexical identity, source attachment vs class-only libraries, writable and read-only documents. |
| Symbols | `textDocument/documentSymbol`, `workspace/symbol` | P/R; hierarchical symbols and library search; workspace-symbol resolve was added on master after the release tag, so it is not established for release. |
| Hover | `textDocument/hover` | R/P; rendered documentation, navigation links, upper bounds on bounded type parameters claimed in v263.6379.0. Test KDoc, aliases, inferred types, overloaded and library symbols, Windows/space paths. |
| Signature help | `textDocument/signatureHelp` | R/P; active signature/argument, named/default/vararg arguments, constructors, lambda arguments. Exact trigger characters U. |
| Semantic tokens | `textDocument/semanticTokens/full`, `textDocument/semanticTokens/range` | R/P; predefined types/modifiers and Kotlin-specific mapping. Full/delta not established by source provider contract. Exact negotiated legend and ordering U; compare decoded token meaning rather than raw type indices. |
| Inlay hints | `textDocument/inlayHint`, `inlayHint/resolve` | P; inferred type, lambda/reference, parameter/default parameter hints; exact capabilities/options U. |
| Formatting | `textDocument/formatting`, `textDocument/rangeFormatting` | R/P; IntelliJ formatter and `.editorconfig`, including configs outside source roots, formatting during fixes. On-type formatting is not established by visible formatting API. |
| Folding | `textDocument/foldingRange` | R/P; classes/functions/blocks, imports, multiline comments with comment/imports/region kinds. |
| Rename | `textDocument/prepareRename`, `textDocument/rename`; file rename/move via workspace file operations | R/P; scoped symbol rename and cross-file edits; provider has `renameFile(FileRename)` and directory provider. Exact `workspace/willRenameFiles`/registration filters U. Preserve package/import references and atomically reject conflicts. |
| Call hierarchy | `textDocument/prepareCallHierarchy`, `callHierarchy/incomingCalls`, `callHierarchy/outgoingCalls` | R/P; functions/properties, overrides, navigation data. |
| Code lenses | `textDocument/codeLens` | R/P; Kotlin main Run/Debug lenses enabled by initialization option. Resolve advertisement U. `intellij.jvm.runMain` is a client command that starts a DAP session, not proof of a server execute-command endpoint. |
| Refactoring | Extract variable, inline local variable, file/package/multiple-file/multiple-package move | R/P; latest release adds extract/inline and improved moves. Commands `refactor.extract.variable`, `kotlin.inline.variable`; move protocol likely file operations, exact runtime U. Must assess side effects, reference identity, collisions, read-only files and edits during deferred actions. |
| Language/plugin support | Up-to-date Kotlin; release history explicitly Kotlin 2.3.0; kotlinx.serialization, AllOpen compiler plugins | R; generated/plugin members require semantics, not matching annotation text. Additional precise versions/features must be pinned from project compiler config and oracle results. |

Release quick-fix blacklist at [blacklist.kt](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/features-impl/kotlin/src/com/jetbrains/ls/api/features/impl/kotlin/diagnostics/blacklist.kt) excludes package mismatch MoveFileToPackageFix and ChangePackageFix, SafeDeleteFix, RenameIdentifierFix; intention blacklist is empty. Do not interpret “most intentions” as every IntelliJ behavior: fixes needing unsupported UI may be omitted or converted.

## Initialization and capabilities

Tag-pinned [initializationSettings.ts](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/vscode-extension-core/src/initializationSettings.ts) exposes built-in initialization options:

- `defaultSdk`: optional JDK-home string for symbol resolution.
- `buildTools`: folder URI -> build-tool id overrides.
- `projects`: configured projects `{type,path,env?,system-properties?,java-home?,project-path?,profiles?,downloadAdditionalArtifacts?,offline?}`. `type/path` required nonempty strings. The common client accepts gradle/maven/bazel/jps/gomodules/gomodules-recursive-scan/json/subproject, **but the Kotlin server registers only gradle/maven/jps/json**, so common client schema must not be used as proof of Kotlin Bazel/Go support. `subproject` recursively resolves nested settings.
- `disableRocksDBWriteAheadLog`: boolean.
- [intellijExtensions.ts](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/vscode-extension-core/src/intellijExtensions.ts) contributes `intellijExtensions:true`, `lazyIntentions:true`; [dap.ts](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/vscode-extension-core/src/dap.ts) contributes `runMainCodeLens:true`.
- Common client reads `initializeResult.capabilities.experimental.indexDir` for clear-cache/restart. Exact experimental schema, command list and per-feature capability values require capture.
- Source comment says a single ill-typed value can cause server to discard its whole built-in initialization object; client sanitizes values. A Rust implementation should have explicit deterministic handling, and compatibility acceptance must distinguish intentional robustness improvements.
- Current master adds `fromWorkspace` and Maven `skipGenerateSources`; these are not release-tag initialization options.

Protocol acceptance should record minimal generic-LSP-client and official-client capability profiles independently. Test UTF-16/non-BMP positions, snippets absent/present, markdown/plaintext, partial progress, resource operations, dynamic registration, workspaces, malformed initialization options and unsupported custom-client extensions. Advertise only implemented features.

## Build systems and workspace model

Tag-pinned [WorkspaceImportLanguageServerExtension.kt](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/features-impl/kotlin/src/com/jetbrains/ls/imports/WorkspaceImportLanguageServerExtension.kt) registers JSON, Maven, Gradle, JPS. README advertises Gradle/Maven JVM and experimental Android Gradle Plugin; KMP is future. Master contains an import-only KMP test, which is not evidence of complete release KMP semantics.

Workspace model covers modules/source sets, production/test/generated/resource/excluded roots, module and scoped/transitive/exported library dependencies, SDK roots, Kotlin/Java language settings, Kotlin facets/compiler-plugin options and output directories. [JSON model](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/workspace-import/src/com/jetbrains/ls/imports/json/model.kt) and checked-in `workspace.json` fixtures are a realistic first native importer contract. Exact versioning/unknown-field policy must be specified separately.

Recent release behavior includes multiple configured projects per workspace/monorepo; automatic build-script reload with always/ask/never; reimport/cache-clear commands; Gradle source downloads by default; Gradle Tooling API 9.5.0; server JRE fallback when JAVA_HOME is missing; Maven configured java-home, settings.xml credentials, compiler args, Windows import; proper Java/Kotlin language version propagation. Dynamic Gradle/Groovy/Kotlin DSL and plugin execution cannot be replaced with a handful of regexes. Static descriptors can be a documented partial mode; parity needs resolved-model provenance.

## Custom protocol, commands, and server/editor boundary

Release-visible direct custom methods:

| Method | Direction and contract |
| --- | --- |
| `intellij/reloadWorkspace` | Client request `{initializationOptions}` -> null; reload without process restart. |
| `intellij/importLog` | Server notification `{type:1|2|3,message,tool?,failed?,succeeded?,started?}`. |
| `intellij/workspaceImportStatus` | Server notification and client request `{}` for snapshot; client schema in `statusBarModel.ts`: `{blockedFolders:[{folderUri,reason,candidates,dismissed}]}`. |
| `intellij/copyToClipboard` | Server notification `{content}`; editor writes clipboard. |
| `intellij/runEditorCommand` | Server notification `{command,arguments?,uri?}`; editor invokes command. |
| `intellij/chooseAction` | Server notification `{title,entries:[{name,command}]}`; editor chooses and executes selected server command. |
| `intellij/showConflicts` | Server request with title/conflict messages+optional locations and labels -> `{decision:'continue'|'cancel'}`; edits cancelled when document changes or request/client cancels. |

Release-visible `workspace/executeCommand` descriptors/client calls include `decompile`, `interpolateFileTemplate`, `exportWorkspace`, `applyModCommand`, `resolveModCommand`, `set-highwatermark-file`, `wait-for-highwatermark`, `kotlin.organize.imports`, `jetbrains.kotlin.completion.apply`, `kotlin.applyCompilerDiagnosticFix`, `refactor.extract.variable`, `kotlin.inline.variable`, and `start_debug_server`. The opaque resolve-location command constant comes from missing core; do not invent its wire name.

Release client uses `intellij.java.resolveBuildCommand`, `intellij.java.resolveClassDocument`, `intellij.java.resolveLaunch`, `intellij.java.resolveBuildToolLaunch`. These are visible compatibility contracts even though their router/core implementations are absent. Source includes shape comments and types in `buildTaskModel.ts`, `buildTask.ts`, `dap.ts`.

**Run/debug** is a distinct protocol subsystem: client gets a DAP TCP port from `start_debug_server`, connects a DebugAdapterServer, and offers launch/attach plus JVM/Gradle configuration; legacy `intellij_debugger` remains supported. Latest release notes claim thread dump, library frames, no set-value on final variables, Gradle 6 launch script compatibility. Previous release added JVM launch.json and debug-console completion/evaluation. This must remain a tracked parity target if “full official product” is intended, but it cannot be certified via LSP handlers alone.

**Build execution in release** is partly editor-owned: release `buildTaskModel.ts` imports Node child-process `spawn`; editor asks server to resolve commands, then runs build tools. Current master moves builds into server `intellij.build` and `intellij/buildOutput`, so do not attribute these master contracts to release.

**Testing UI** is master-only in this mirror: `vscode-testing` and `vscode-jvm-testing` modules were added after release tag; master enables both in Kotlin extension. Master commands: `intellij.jvm.discoverTestsInFile`, `intellij.jvm.discoverTestModules`, `intellij.jvm.discoverTestsInModule`, `intellij.jvm.resolveTestLaunch`; JUnit/TestNG runner launch and TeamCity service-message parsing. Keep research/tickets for these, but mark release support unknown until runtime proves it.

**Editor features** include Tree-sitter-based Enter/indent/pairing/key handling, TextMate lexical grammar, line-comment behavior, status bar, extension installation/conflict/setup/EULA/region/data-sharing UI, template-file creation, decompiled jar/jrt virtual document providers, launch/task integration, reload prompts and debugger/testing UI. These are not standalone LSP methods. Rust-only server can support their protocol contracts; reproducing the complete VS Code UI separately requires an explicit client scope. Do not port region/EULA/policy UI as if it were a Kotlin language feature.

## Master-only follow-up inventory

`git diff 4acdb6e..5c70758` shows 283 files changed. Newly registered Kotlin type hierarchy (`textDocument/prepareTypeHierarchy`, `typeHierarchy/supertypes`, `typeHierarchy/subtypes`), injection inspection/syntax diagnostics, suppression actions, related-items code lenses; new `intellij/qualifiedName`; workspace-symbol resolve, decompiler improvements, changed semantic-token processing, workspace import state request/notification, server-side build, JVM test discovery/runner UI, launch/build resolution architecture changes, `fromWorkspace`, `skipGenerateSources`. Track source-following upgrades separately from the fixed release target. Source links for these must use master pin, not release pin.

## Acceptance fixtures and validation plan

Public tests visible in the mirror are not an exhaustive Kotlin semantic LSP suite. There are 260 tracked paths matching testData/testSources in current checkout. Native tests should use independently authored fixtures where practical, retain upstream fixture attribution where copied, and never report upstream tests as executed without the full build environment.

| Corpus | Visible evidence | Feasible acceptance |
| --- | --- | --- |
| Semantic token encoding | `api.features/test/.../SemanticTokensEncoderTest.kt` | Multiline/range clipping, stable ordering, delta-line/delta-start arithmetic, decode comparison, non-BMP/CRLF; independently port algorithm-level cases. |
| Text edits | `api.features/test/.../TextEditsComputerTest.kt` | Correct edits applying to source; minimal vs equivalent edits, overlapping changes, CRLF and Unicode. |
| Gradle imported models | `workspace-import/test/testData/gradle/*/workspace.json` and GradleProjectImportTest | MultiProjectKotlinDSL/GroovyDSL, CustomSourceSets, IncludedBuild, language versions, dependencies, missing dependencies, broken/empty project, Android; compare native import of model without running Gradle. |
| Maven models | `.../maven/SimpleMaven`, CustomPomName, AnnotationProcessing, Kotlin language-version sources | Scoped dependencies, generated source roots, source attachments, profiles/environment/system-properties, offline handling; dynamic plugin execution remains separate gap. |
| JPS/JSON models | `.../jps/*`, JsonImporterUnitTest, JSON model | Modules, test roots, exported module libraries, Kotlin facets, path/URI normalization, missing references/cycles. |
| Client contracts | TypeScript initialization/settings, showConflicts, startup/port/process, dap/build tests | Serialize documented wire payloads, cancellation and stale edit behavior, rejected capabilities/commands, abrupt process exit; tests are source evidence only so far. |
| Grammar/key handling | `kotlin-vscode/testSources/tokenization/sample.kt(.snap)`, Kotlin key-handler tests | Syntax coverage evidence; these do not establish semantic LSP correctness. |
| Run/debug | release dap/build sources; newer master run tests | Separate DAP transcripts and launch-resolution fixtures, thread/stack/eval/breakpoint behavior; flag no-JVM impossibility instead of returning success-shaped empty results. |

Proposed independently authored differential corpus:

1. Basic declarations/packages/import aliases/star imports, same names across scopes, backtick identifiers, nested/local/anonymous declarations.
2. Generic constraints, variance/projections, overloads, callable references, lambdas/default/named/vararg arguments, receiver/context parameters, delegated properties, destructuring, suspend/coroutines, nullable/platform types, smart casts and exhaustive when.
3. Cross-file/source-set/module lookups and refactors, test visibility, Java source/class interop, extension overloads, type aliases and generated/plugin declarations.
4. JAR/classfile/Kotlin metadata/source attachment/JDK metadata without a runtime; corrupted archive, ZIP traversal, huge compression ratio, malformed classfile and metadata limits.
5. Completion/hover/diagnostic/actions across damaged in-progress code; stale versions, concurrent edits, cancellation, reload and external file events.
6. Actual import/organize/rename/extract/inline/move results applied atomically; symbol binding preserved, no capture/collisions, side effects preserved, comments/string mentions untouched where appropriate.
7. UTF-8 source and negotiated LSP coordinates: astral Unicode, combining marks, CRLF, lone newline, empty file, invalid offset, EOF and multi-edit ordering.
8. Transport: fragmented Content-Length frames, multiple frames per read, arbitrary JSON-RPC ids, invalid headers/JSON/params, unknown request vs notification, cancellation, EOF, bounded input and no stdout logging.
9. Performance/crashes: large/generated source, deep nesting, recursive declarations/cycles, cancellation storms, rapid workspace changes, corrupt persisted cache, process restart and allocation limits. Define quantitative budgets using measured baseline hardware before claiming faster.

Oracle process should fetch the release with SHA-256 verification, record archive hash/platform/launcher args, run isolated fixture workspaces, save initialize/capability and feature/DAP transcripts and then normalize URIs, timing, ids, ordering permitted by protocol, source ranges and opaque resolution handles. Preserve observable action edits and symbol identity; do not normalize away meaningful behavior. The official bundled runtime is research-only and must never become a runtime dependency or release artifact of the Rust product. Empty handlers, guessed capabilities or name-based searches fail parity acceptance even when they return valid JSON.

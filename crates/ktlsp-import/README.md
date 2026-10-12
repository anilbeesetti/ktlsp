# T021a: pinned workspace.json subset

Child [#63](https://github.com/anilbeesetti/ktlsp/issues/63) of T021/#21. Data-only Rust library, not wired into server initialization or advertised as an LSP provider. This is native-tested source-contract compatibility; no official runtime comparison or build tool was run. T021 remains open.

## Provenance

Official baseline `kotlin-lsp/v263.6379.0`, commit `4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748`:

- [model.kt](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/workspace-import/src/com/jetbrains/ls/imports/json/model.kt), Git blob `6f0bfd6b930c3726b26850de3bd06f76345c4a48`.
- [conversion.kt](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/workspace-import/src/com/jetbrains/ls/imports/json/conversion.kt), Git blob `e257a64ee143b39654703b1c42facfd3b298c969`.
- [JpsJavaModule fixture](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/workspace-import/test/testData/jps/JpsJavaModule/workspace.json) and [JsonImporterUnitTest](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/workspace-import/test/test/com/jetbrains/ls/imports/JsonImporterUnitTest.kt) establish original field/macro/root/edge intent. Our small fixtures and implementation are independently authored, not copied upstream tests.

Upstream source notice: Copyright 2000-2025 JetBrains s.r.o. and contributors. Use of that source code is governed by the [Apache 2.0 license](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/LICENSE).

## Public contract

`from_json(absolute_project_directory, bytes) -> Result<ImportedWorkspace, ImportError>`. The official input is **unversioned**. Version/schemaVersion keys, even 1, are unknown fields. No native manifest version is expected in official input.

The complete supported result is `ImportedWorkspace`: ordered modules, content-root boundaries/source membership/exact root type, full dependency order (including moduleSource position), and library-root order/type, together with `project_projection()`. The latter is a validated ProjectModel v1 projection and **alone loses import semantics**. Later consumers must use the whole imported result. This import-local representation does not change ProjectModel v1.

Supported fields/defaults follow the source contract:

- Workspace modules/libraries default empty. sdks/kotlinSettings/javaSettings may be absent or empty; externalSystem absent/null.
- Module name required; type absent or JAVA_MODULE. coordinate/externalProjectPath/externalProjectId absent/null; facets absent/empty. Ordered contentRoots/dependencies default empty.
- Content path required; excludedPatterns/excludedUrls absent/empty. Source roots default empty; each requires path/type. java-source/kotlin-source map to production and java-test/kotlin-test to test, with exact original type retained. Generated/resource/custom root semantics are unsupported.
- Module dependencies retain required name/scope (compile/test/runtime/provided), default-false isExported, and default-false isTestJar. True isTestJar blocks. moduleSource is retained in order. SDK/inheritedSdk/custom variants block.
- Library dependencies require name/scope; only compile/default-false isExported converts. Scoped/exported library references block; they are never downgraded.
- Libraries require name, **type even when null**, and roots. Only null type, default project level and absent/null module/properties are supported. excludedRoots absent/empty. Roots require path; type defaults CLASSES; CLASSES/SOURCES supported. inclusionOptions absent/root_itself; directory archive traversal blocks.

Meaningful unsupported fields return `ImportError::Unsupported` with JSON pointer/reason records and **no partial model**. Unknown fields block even when null, including nested fields in supported objects. Unsupported rich subtrees (SDK/compiler settings/facets) are reported as entire fields; this importer does not validate or translate their internal schema. Ill-typed decoded fields return Schema; fields restricted to a neutral value return Unsupported for any other value; malformed JSON/duplicate keys return Json. If both malformed supported fields and unsupported fields exist, the schema error may take precedence.

Native v1 validation rejects invalid/duplicate IDs, unresolved references, duplicate module/library edges, cycles, source ownership conflicts and incompatible settings/collection budgets. Edges are never coalesced or flattened to pass validation. Missing archives are retained without claiming resolution.

## Paths and resource limits

Paths support `<WORKSPACE>/` substitution and absolute host-native paths, preserving Unicode and normalizing `.`/`..` lexically. No URI decoding, backslash paths, other macros, environment lookup, relative-path guessing, filesystem existence/symlink check, archive scan, SDK synthesis, download or build execution. Parent-relative macro paths are permitted. Caller must supply an absolute UTF-8 non-NUL project directory. The official importer additionally repatriates existing ancestors using real/symlink paths; this library makes **no official path-parity claim** for those cases. Cross-platform/Windows path compatibility needs a separate child.

Before Value construction, a custom serde visitor limits 2 MiB input bytes, 100,000 JSON values (keys are bounded by bytes), and depth 32 including root; duplicate keys are rejected at every depth. The caller project-directory length and aggregate expanded path bytes are each capped at 2 MiB; expanded lengths are charged conservatively before joining/normalizing paths. Reports stop at 128 and set `truncated` if additional fields exist; no model is returned. Conversion also applies native v1 budgets, including its 2 MiB serialized projection limit; expanding long absolute paths can hit that limit. All graph validation is iterative through ktlsp-project.

## Next child

Extend the native/import contract for module SDK identity/default inheritance, scoped/exported library edges, resource/excluded roots, facets/plugins, compiler/language/output settings, special library metadata, and build-system provenance. Then add original-schema fixtures for those translations and oracle comparisons. This subset does not implement full T021, Gradle/Maven/JPS import, dependency resolution or compiler semantics.

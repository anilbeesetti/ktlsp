# Krusty frontend source provenance

- Upstream: <https://github.com/qnox/krusty>
- Revision: `3b0d3b926857564d3fdc65b66d7003564ef6a9ad`
- Source archive: <https://github.com/qnox/krusty/archive/3b0d3b926857564d3fdc65b66d7003564ef6a9ad.tar.gz>
- Source archive SHA-256: `fe2beef88107841c1f1f4b2f196ee1ea684816c4984561bcc15bd3218772fd61`
- Upstream package version: `0.0.1`
- License: Apache-2.0; the upstream `LICENSE` is retained unchanged.

This is a selected-source library named `krusty-frontend` (`krusty_frontend` in Rust), not the upstream compiler package. It has its own empty Cargo workspace. The upstream build script, binaries, workspace members, external integration-test tree, development dependencies, and runtime/toolchain build paths are not distributed or invoked.

## Retained source boundary

The selected roots are `assignable`, `ast`, `ast_validate`, `callable_access`, `compilation_target`, `context_parameters`, `contracts`, `declaration_validation`, `diag`, `diagnostic_wording`, `enclosing_declarations`, `features`, `fir`, `frontend`, `fx_hash`, `inline_parameter_modifier`, `integer_constant`, `ir`, `java_source`, `kotlin_version`, `kt_string`, `language_version`, `lexer`, `lexical_policy`, `libraries`, `lifting_provenance`, `module_symbols`, `name_tree`, `names`, `parser`, `plugins`, `program_entry`, `resolve`, `source`, `spelling`, `symbol_resolver`, `symbol_source`, `synthetics`, `token`, `trace`, `type_engine`, `types`, `value_classes`, and `wide_stack`, with their production children. `ast_validate` supplies the parser's real AST integrity checks through an `impl File`.

FIR, signature inference, retained body checking, default-argument checking, inline-body preparation, and the real Rust plugin implementations remain part of the frontend. Common IR remains because the plugin API and FIR's type-reflection traversal depend on it; retaining IR is not a claim that this package emits artifacts.

The plugin root retains `allopen`, `cli`, `noarg`, `registry`, `sam_with_receiver`, and `serialization` with its children. The JVM root contains only the original `protected_dependency_calls` data record required by common IR. The metadata root retains `contract_decoder`, `decode`, `id_signature`, `semantic`, and their children, plus the original function/property flag constants and `value_parameter_flags`/`DeclaredValueParameter` used by the semantic decoder.

The upstream `kotlin-versions` file and all original `src/features/releases/*.features.tsv` and `*.language-versions.tsv` assets are retained unchanged. They are compiled-in language/version data, not a downloaded or executed Kotlin toolchain. Existing cfg-test source leaves under retained production roots are preserved where those roots still declare them. They are not the upstream standalone test suite: some require excluded compiler, KLIB, JVM, toolchain modules, or external fixtures. The removed root-level test-module declarations do not expose those excluded suites.

## Local extraction changes

- Replaced the package manifest with the selected library and disabled automatic build scripts, binaries, and integration-test discovery. Direct dependencies are `ahash`, `unicode-general-category`, and ZIP 9 with only the Rust `deflate-flate2-zlib-rs` backend; there is no `stacker`, Cranelift, object emitter, JVM helper, C runtime, or development dependency.
- Selected the frontend roots in `src/lib.rs` and reexported `Limits`, `LimitExceeded`, and `with_limits` from the Rust-only resource boundary.
- Selected the real plugin, metadata-reader, and JVM-data children in their module roots; removed the upstream metadata-emission helper functions and their writer-only tests from the metadata root.
- Removed only backend-owned `IrFile.class_declarations`, the associated `RecordRenaming` metadata-record implementation, and the remapping of that removed field. Other common-IR classifier remapping and plugin semantics remain.
- Removed the unselected `backend`, `compiler`, `conformance`, `fir_lower`, `js`, `native`, `wasm`, `klib`, `klib_libraries`, `klib_lowering`, `runtime`, `toolchain`, `dump`, `ast_print`, and `ir_print` source roots and their children, upstream `src/bin`, root-level architecture/backend tests, and the unreferenced `lru` and `language_settings` roots. Unselected metadata writer, JVM backend/classpath/emitter, and plugin-sidecar/codegen-host sources are not distributed.
- Replaced upstream stack-growth dependence with explicit Rust-only request work/depth limits. The embedding worker constructs and destroys thread-affine compiler state on its dedicated Rust thread and returns owned results only. Its fixed stack is capacity, not a proof of stack-growth equivalence. Resource failure, child abort, or timeout must discard the whole request's output.
- Removed `libraries/dependency_body_contract.rs`, its module/reexports, the backend-only `SemanticPlatform::klib_dependency_body_facts` capability, and the sole retained forwarding method in `resolve/dependency_platform.rs`. Its KLIB body/intrinsic/snapshot records served excluded compiler/backend orchestration only; frontend declaration and checking contracts are unchanged.

## Scope and evidence

No Kotlin compiler parity, diagnostic parity, target-backend parity, JVM/classpath completeness, stack-growth equivalence, or executed upstream conformance result is claimed. Empty-platform analysis uses the real semantic checker and language builtins but does not supply an external Kotlin standard library or JDK classpath; unresolved external declarations remain diagnostics. Extraction is not a replacement for full project platform resolution.

Build, test, formatting, and runtime verification are intentionally performed by the integrating project, not during source extraction. Upstream cfg-test sources are retained as source provenance, not represented as an independently runnable frontend-only test inventory.

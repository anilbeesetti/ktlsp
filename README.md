# ktlsp

A Rust-only Kotlin language server under development, targeting feature parity with official `kotlin-lsp/v263.6379.0`. Full parity has **not** been achieved.

## Build and run

The repository pins Rust 1.99.0. With Rust/rustup and the platform linker installed:

```sh
cargo build --locked --release -p ktlsp-server --bin ktlsp
./target/release/ktlsp
```

With no arguments, `ktlsp` reads and writes Content-Length-framed LSP on stdio. Configure an editor's server command to the built executable with no arguments. Use `--help` or `--version` for CLI output. It needs no JVM or JDK for this core operation.

## Current behavior

**Assessed source/status cutoff: `058b002010bf49585e09c2dbc9e9d559a076f9c9`** (PR153 merge, 2026-10-13T11:50:57Z), tree `532eee013a5a172a739e955f8be9d1b7e588c599`, sole parent `a1887d43905cdb37479fe4b54f1d7b24cbfc4df7`. [Exact-cutoff main Linux CI](https://github.com/anilbeesetti/ktlsp/actions/runs/38393067849/job/115281370488), original attempt 1, passed all three unchanged pinned commands: **373 passing tests (364 ordinary and nine compile-fail doctests), zero failures, two ignored subprocess helpers exercised by active parents, and 19 separately counted worker-protocol fault cases**. These counts describe this exact integration, not a sum of deliveries or present-day main validation. Native semantics/full parity remain pending; T008/T008a HOLD, broader C13 GAP, B3/B4 GAP, B11 UNVERIFIED, #7/#83 OPEN and #96 OPEN/UNKNOWN. Linux passes do not establish a Darwin cause or correction.

**Historical source/status cutoff: `64234587e333ebcd32d96600a7a193b06b0ac9df`** (PR135 merge, 2026-10-13T07:17:54Z), tree `3f6828ce85a8b74a9142c4fd6d9893ad107210d3`, sole parent `57411e8605495aec376edb76843fd282f6933b9c`. [Exact-cutoff main Linux CI](https://github.com/anilbeesetti/ktlsp/actions/runs/38361781927/job/115177434337), original attempt 1, passed all three unchanged pinned commands: **326 passing tests (317 ordinary and nine compile-fail doctests), zero failures, two ignored subprocess helpers exercised by active parents, and 19 separately counted worker-protocol fault cases**. These counts describe this exact integration, not a sum of deliveries or present-day main validation. Native semantics/full parity remain pending; T008/T008a HOLD, B3/B4 GAP, B11 UNVERIFIED, #7/#83 OPEN and #96 OPEN/UNKNOWN. Linux passes do not establish a Darwin cause or correction.

**Historical source/status cutoff: `0676144278013bbfee6312124aadc9b79f24afd9`** (PR116 merge, 2026-10-13T03:51:13Z), tree `1deac457a4a09327330834915c0bd966cfc5fd58`, sole parent `aa9e6587bb28c368e2e770fc8fb297c59f83854b`. [Exact-cutoff main Linux CI](https://github.com/anilbeesetti/ktlsp/actions/runs/38343657109/job/115120031161) passed all three unchanged pinned commands: **276 passing tests, zero failures, two ignored subprocess helpers exercised by active parents, and 19 separately counted worker-protocol cases**. These counts describe this exact integration, not a sum of deliveries or present-day main validation. Native semantics/full parity remain pending; T008/T008a HOLD, #7/#83 OPEN and #96 OPEN/UNKNOWN.

The server implements initialization/shutdown/exit, bounded versioned UTF-16 document synchronization with atomic multi-edit rollback, and token-aware folding for multiline delimiters, block comments and raw strings. Accepted open/change notifications replace lexical/structural push diagnostics; close clears them. It advertises synchronization and folding. Unsupported language requests return method-not-found; document symbols, completion, hover, semantic diagnostics, navigation, formatting and refactoring remain unimplemented.

Folding respects `lineFoldingOnly` and `rangeLimit`, with explicit resource limits. It covers a lexical subset; import/declaration folds and full official folding parity remain pending. See [server behavior and bounds](crates/ktlsp-server/README.md).

A separate supervised parser safety adapter has a 64 KiB input ceiling and no server consumer; complete grammar/recovery remains pending. Finite simple/qualified extension-generic, C14, C15 and C16 recovery repairs, selected declaration assertions and reconstructed resource-reply serialization evidence are delivered; broader recovery/Coverage, B3/B4 and active-EOF B11 remain incomplete. C16 covers the frozen ordinary-parameter `@A(` sibling seed in LF/CRLF/lone CR, three valid anonymous controls and a lazy-body negative; it does not complete the broader C13/C14/C15/C16 clauses. One bare enum-entry fixture adds finite acceptance only. Checked normal-return direct-child completion preserves ambiguous `Io`; waits/joins have no hard bound and supply no raw reap receipt or B3/B4 proof. B11 feasibility remains HOLD with its optional invariant skipped.

Separate Rust libraries provide bounded classfile structure, declaration/parameter annotations and defaults, encoded field and method-return type annotations, depth-32 Java Signature ASTs, nesting/enclosing data, pinned Metadata header/d1, flat wire, StringTableTypes schema data and UTF-16 name-record/local-name helpers, typed-table/opaque-payload composition and selected Class flags/fq_name data. Method support is encoded `METHOD_RETURN` only; broader type-annotation contexts and semantic applicability remain pending. Selected Class data leaves nested messages opaque. Names do not reconstruct Kotlin declarations/types; selected malformed protobuf UTF-8 remains an `UnsupportedUtf8` GAP. Composition shares framing/table accounting; header/d1, Class and resolver operations retain separate budgets, without a whole-pipeline RSS promise. Explicit v1 and separate ordered-edge v2 project models retain data-only scope; v1 path budgets and V2 retained-prefix accounting are delivered. Windows/Cygwin test bodies are neither typechecked nor executed by Linux/Darwin checks, so platform correctness remains unverified.

A bounded offline Maven-layout lookup returns retained bytes/provenance; a separate immutable VerifiedArtifact helper checks those bytes against a caller SHA-256 digest, while lookup provenance stays `Unverified`. A separate bounded ZIP central-directory index borrows ordered raw metadata; it validates no local headers, payloads, CRC, decompression, safe paths or JAR semantics and authenticates no digest. These data foundations have no server consumers or live build import, linkage or access-control semantics. Optional standalone tools replay committed observations and record bounded sessions with synthetic Rust peers; no fresh official JVM observation is delivered. Historical benchmark evidence has a committed [provenance erratum](tools/bench/PROVENANCE-ERRATUM.md), without reproducible-build/current-main or official speed/RAM claims. PR137/140/141/143/145/147/149/151/153 are delivered through this cutoff; Function-header #152/PR154 and stored-ZIP follow-up work are outside this assessment. Their later state does not advance it. Limits and exact delivered issue↔PR/review/CI evidence are in [parity status](docs/status/parity.md) and [the development ledger](docs/status/development.md).

The native server, build and core editor analysis require no JVM, JDK, Kotlin compiler, JNI or C/C++ parser. Optional future Gradle/Maven import/build/run/debug may use the user's existing Java tools; no Java is bundled. See [runtime boundary](docs/specs/runtime-boundary.md).

## Development

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Read [research](docs/research/official.md), [specifications](docs/specs/parity.md), [plan](docs/PLAN.md), [tickets](docs/TICKETS.md), and [handoff instructions](docs/HANDOFF.md) before continuing implementation. Unsupported language and project behavior must remain explicit.

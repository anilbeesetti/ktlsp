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

**Current source/status cutoff: `4e451af2dfb4b37d236aab1d725ad51d4749af81`** (PR120 merge, 2026-10-13T02:02:35Z). [Exact-main Linux CI](https://github.com/anilbeesetti/ktlsp/actions/runs/38335007971/job/115092993213) passed all three unchanged pinned commands: **229 passing tests (225 ordinary tests and four compile-fail doctests), zero failures, two ignored subprocess helpers exercised by active parents, and 19 separately counted protocol fault cases**. These counts describe this exact integration, not a sum of deliveries. Native semantics/full parity remain pending; T008/T008a HOLD, #7/#83 OPEN and #96 OPEN/UNKNOWN.

The server implements initialization/shutdown/exit, bounded versioned UTF-16 document synchronization with atomic multi-edit rollback, and token-aware folding for multiline delimiters, block comments and raw strings. Accepted open/change notifications replace lexical/structural push diagnostics; close clears them. It advertises synchronization and folding. Unsupported language requests return method-not-found; document symbols, completion, hover, semantic diagnostics, navigation, formatting and refactoring remain unimplemented.

Folding respects `lineFoldingOnly` and `rangeLimit`, with explicit resource limits. It covers a lexical subset; import/declaration folds and full official folding parity remain pending. See [server behavior and bounds](crates/ktlsp-server/README.md).

A separate supervised parser safety adapter has a 64 KiB input ceiling and no server consumer; complete grammar/recovery remains pending. Separate Rust libraries provide bounded classfile structure, declaration annotations, depth-32 Java Signature ASTs, nesting/enclosing attributes and pinned Kotlin Metadata header/d1 decoding, plus a pinned exported-workspace subset alongside explicit v1 and separate ordered-edge v2 project-model loaders. A bounded offline Maven-layout lookup returns retained bytes/provenance; a separate immutable VerifiedArtifact helper checks those bytes against a caller SHA-256 digest, without archive validation or digest authentication. These data foundations have no server consumers or live build import. Optional standalone tools replay committed observations and record bounded raw sessions with synthetic Rust peers; no fresh official JVM observation is delivered. Selected declaration assertions and known-defect fixtures are delivered without grammar repairs. Historical benchmark evidence has a committed [provenance erratum](tools/bench/PROVENANCE-ERRATUM.md), not current-main performance validation. These subsets do not establish linkage, access control or protobuf/name/type semantics. Limits and merged evidence are documented in [parity status](docs/status/parity.md).

The native server, build and core editor analysis require no JVM, JDK, Kotlin compiler, JNI or C/C++ parser. Optional future Gradle/Maven import/build/run/debug may use the user's existing Java tools; no Java is bundled. See [runtime boundary](docs/specs/runtime-boundary.md).

## Development

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Read [research](docs/research/official.md), [specifications](docs/specs/parity.md), [plan](docs/PLAN.md), [tickets](docs/TICKETS.md), and [handoff instructions](docs/HANDOFF.md) before continuing implementation. Unsupported language and project behavior must remain explicit.

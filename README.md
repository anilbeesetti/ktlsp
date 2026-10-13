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

**Assessed source/status cutoff: `0676144278013bbfee6312124aadc9b79f24afd9`** (PR116 merge, 2026-10-13T03:51:13Z), tree `1deac457a4a09327330834915c0bd966cfc5fd58`, sole parent `aa9e6587bb28c368e2e770fc8fb297c59f83854b`. [Exact-cutoff main Linux CI](https://github.com/anilbeesetti/ktlsp/actions/runs/38343657109/job/115120031161) passed all three unchanged pinned commands: **276 passing tests, zero failures, two ignored subprocess helpers exercised by active parents, and 19 separately counted worker-protocol cases**. These counts describe this exact integration, not a sum of deliveries or present-day main validation. Native semantics/full parity remain pending; T008/T008a HOLD, #7/#83 OPEN and #96 OPEN/UNKNOWN.

The server implements initialization/shutdown/exit, bounded versioned UTF-16 document synchronization with atomic multi-edit rollback, and token-aware folding for multiline delimiters, block comments and raw strings. Accepted open/change notifications replace lexical/structural push diagnostics; close clears them. It advertises synchronization and folding. Unsupported language requests return method-not-found; document symbols, completion, hover, semantic diagnostics, navigation, formatting and refactoring remain unimplemented.

Folding respects `lineFoldingOnly` and `rangeLimit`, with explicit resource limits. It covers a lexical subset; import/declaration folds and full official folding parity remain pending. See [server behavior and bounds](crates/ktlsp-server/README.md).

A separate supervised parser safety adapter has a 64 KiB input ceiling and no server consumer; complete grammar/recovery remains pending. Separate Rust libraries provide bounded classfile structure, declaration annotations, depth-32 Java Signature ASTs, nesting/enclosing attributes and pinned Kotlin Metadata header/d1 and bounded flat raw wire decoding (no schema/names/types), plus a pinned exported-workspace subset alongside explicit v1 and separate ordered-edge v2 project-model loaders. A bounded offline Maven-layout lookup returns retained bytes/provenance; a separate immutable VerifiedArtifact helper checks those bytes against a caller SHA-256 digest, while lookup provenance stays `Unverified`. A separate bounded ZIP central-directory index borrows ordered raw metadata from those retained bytes; it validates no local headers, payloads, CRC, decompression, safe paths or JAR semantics and authenticates no digest. These data foundations have no server consumers or live build import. Optional standalone tools replay committed observations and record bounded raw sessions with synthetic Rust peers; no fresh official JVM observation is delivered. Selected declaration assertions, known-defect fixtures, finite C14/C15 recovery repairs and reconstructed resource-reply serialization evidence are delivered; broader recovery, B3/B4 and active-EOF B11 remain incomplete. PR117/PR118/#125 are outside the assessed evidence. Historical benchmark evidence has a committed [provenance erratum](tools/bench/PROVENANCE-ERRATUM.md), not current-main performance validation. These subsets do not establish linkage, access control or protobuf/name/type semantics. Limits and merged evidence are documented in [parity status](docs/status/parity.md).

The native server, build and core editor analysis require no JVM, JDK, Kotlin compiler, JNI or C/C++ parser. Optional future Gradle/Maven import/build/run/debug may use the user's existing Java tools; no Java is bundled. See [runtime boundary](docs/specs/runtime-boundary.md).

## Development

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Read [research](docs/research/official.md), [specifications](docs/specs/parity.md), [plan](docs/PLAN.md), [tickets](docs/TICKETS.md), and [handoff instructions](docs/HANDOFF.md) before continuing implementation. Unsupported language and project behavior must remain explicit.

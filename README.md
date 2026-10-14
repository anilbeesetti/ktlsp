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

**Assessed source/status cutoff: `b026508e126e64572fd3a4b646fee95304a5768d`** (PR182 merge, 2026-10-14T03:55:38Z), tree `b0db5b2651e31d7ce33657fd4495f9600f5658ca`, sole parent `cbdf087b18cfad3c2bfe67ee4854e558197cd337`. [Exact-cutoff main Linux CI](https://github.com/anilbeesetti/ktlsp/actions/runs/38508764705/job/115672197852), original attempt 1/push, passed all three unchanged pinned commands: **454 passing tests (441 ordinary and 13 compile-fail doctests), zero failures, two ignored subprocess helpers exercised by active parents, and 19 separately counted worker-protocol fault cases**. These counts describe this exact integration, not a sum of deliveries or present-day main validation. Native semantics/full parity remain pending; T008/T008a HOLD, broader C13 GAP, B3/B4 GAP, B11 UNVERIFIED, #7/#83 OPEN and #96 OPEN/UNKNOWN. Linux passes do not establish a Darwin cause or correction.

Historical cutoffs and per-delivery receipts remain in [the development ledger](docs/status/development.md).

The server implements initialization/shutdown/exit, bounded versioned UTF-16 document synchronization with atomic multi-edit rollback, and token-aware folding for multiline delimiters, block comments, raw strings and contiguous whole-line `//` comment groups (PR177/#175). Accepted open/change notifications replace lexical/structural push diagnostics; close clears them. It advertises synchronization and folding. Unsupported language requests return method-not-found; document symbols, completion, hover, semantic diagnostics, navigation, formatting and refactoring remain unimplemented.

Folding respects `lineFoldingOnly` and `rangeLimit`, with explicit resource limits. It covers a lexical subset; import/declaration folds and full official folding parity remain pending. See [server behavior and bounds](crates/ktlsp-server/README.md).

A separate supervised parser safety adapter has a 64 KiB input ceiling and no server consumer; complete grammar/recovery remains pending. Finite simple/nullable/plain-qualified extension-generic, C14, C15 and C16 recovery repairs, selected declaration assertions and reconstructed resource-reply serialization evidence are delivered; broader recovery/Coverage, B3/B4 and active-EOF B11 remain incomplete. C16 covers the frozen ordinary-parameter `@A(` sibling seed in LF/CRLF/lone CR, three valid anonymous controls and a lazy-body negative; it does not complete the broader C13/C14/C15/C16 clauses. One bare enum-entry and one ordinary local two-name destructuring fixture add finite syntax acceptance only; component, binding and type semantics remain pending. PR170/#169 adds finite ordinary `val IDENTIFIER =` sibling recovery after unfinished classifier generics, excluding typed/destructured/var/receiver/modifier/annotation/delegate headers. PR182/#181 continues original cancellation/deadline checks after successful child reaping while required I/O remains pending; gated shared-loop tests and static stage wiring do not demonstrate delayed real-pipe/full-supervisor scheduling. Checked child completion preserves ambiguous `Io`; cleanup waits/joins have no hard bound, and an unjoined scoped-thread panic can prevent ordinary return. B3/B4 remain GAP. B11 feasibility remains HOLD with its optional invariant skipped.

Separate Rust libraries provide bounded classfile structure, declaration/parameter annotations and defaults, encoded field, method-return and formal-parameter type annotations, depth-32 Java Signature ASTs, nesting/enclosing data, pinned Metadata header/d1, flat wire, StringTableTypes schema data and UTF-16 name-record/local-name helpers, typed-table/opaque-payload composition and selected Class flags/fq_name and Function name/raw flags data. Method targets `0x14`/`0x16` retain encoded data and a raw parameter index only; broader type-annotation contexts and semantic applicability remain pending. PR178/#176 adds selected Property name and four independent raw flag Options; PR179/#173 adds raw TypeParameter id/name, reified/variance presence, ordered bound IDs and borrowed opaque upper-bound Type bodies. Selected Class/Function/Property/TypeParameter data does not establish full nested initialization, Kotlin reconstruction or generic substitution. Names do not reconstruct Kotlin declarations/types; selected malformed protobuf UTF-8 remains an `UnsupportedUtf8` GAP. Composition shares framing/table accounting; header/d1, Class and resolver operations retain separate budgets, without a whole-pipeline RSS promise. PR180/#171 adds a separate import-v2 API for all eight library scope/export tuples with ordered sidecar/project-v2 projection; strict v1 behavior stays unchanged. SDK/resources/facets/compiler/build metadata, path/platform parity, live import and semantic module resolution remain pending. Explicit v1 and separate ordered-edge v2 project models retain data-only scope; v1 path budgets and V2 retained-prefix accounting are delivered. Windows/Cygwin test bodies are neither typechecked nor executed by Linux/Darwin checks, so platform correctness remains unverified.

A bounded offline Maven-layout lookup returns retained bytes/provenance; a separate immutable VerifiedArtifact helper checks those bytes against a caller SHA-256 digest, while lookup provenance stays `Unverified`. A separate bounded ZIP central-directory index borrows ordered raw metadata; its separate ordinal accessor checks selected stored (method 0) payloads against local/central headers and CRC under per-call limits. Deflated method 8/flags `0x0808` entries in common pinned artifacts remain unsupported. No decompression, whole-archive/path/JAR validation or digest authentication is delivered. These data foundations have no server consumers or live build import, linkage or access-control semantics. Optional standalone tools replay committed observations and record bounded sessions with synthetic Rust peers; no fresh official JVM observation is delivered. Historical benchmark evidence has a committed [provenance erratum](tools/bench/PROVENANCE-ERRATUM.md), without reproducible-build/current-main or official speed/RAM claims. Genuine licensed fixtures and byte readers already exist, but no genuine JAR-to-symbol/server integration is demonstrated. PR168/177/178/179/180/170/182 are also delivered through this fixed cutoff; historical `5e8d283f`/436 remains in the ledger. Integration base `44f77b92` additionally includes PR174's prospective unconsumed JSON packet, outside the assessed product cutoff. Limits and exact delivered issue↔PR/review/CI evidence are in [parity status](docs/status/parity.md) and [the development ledger](docs/status/development.md).

The native server, build and core editor analysis require no JVM, JDK, Kotlin compiler, JNI or C/C++ parser. Optional future Gradle/Maven import/build/run/debug may use the user's existing Java tools; no Java is bundled. See [runtime boundary](docs/specs/runtime-boundary.md).

## Experimental macOS server check

At frozen `50bef1c6`, a separately reviewed observation on macOS 27.0.1 arm64 with pinned Rust 1.99.0 ran the build, version and server-package test operations below once, using an isolated target directory for the saved executable:

```sh
cargo build --locked --release -p ktlsp-server --bin ktlsp
./target/release/ktlsp --version
cargo test --locked -p ktlsp-server
```

**Known build-child failure:** `rust-objcopy` aborted with SIGABRT because dyld could not load `@rpath/libLLVM.dylib`; debug-info stripping failed even though Cargo exited 0. The release executable reported its version, and 38 server-only tests passed (12 provider, 16 session, 10 stdio). This is bounded package evidence at `50bef1c6`, not current-cutoff/workspace/parser/dependency-crate tests, editor/performance validation or full Mac support. The server has no parser dependency; [#96](https://github.com/anilbeesetti/ktlsp/issues/96) remains OPEN/UNKNOWN. See the durable [Mac observation receipt](docs/status/development.md#bounded-mac-server-observation) for provenance and limits.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Read [research](docs/research/official.md), [specifications](docs/specs/parity.md), [plan](docs/PLAN.md), [tickets](docs/TICKETS.md), and [handoff instructions](docs/HANDOFF.md) before continuing implementation. Unsupported language and project behavior must remain explicit.

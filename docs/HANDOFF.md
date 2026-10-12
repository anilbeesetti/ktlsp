# Resume native Kotlin LSP development

All work required for the next environment is on GitHub main. Clone `https://github.com/anilbeesetti/ktlsp` with full history, inspect current main and open issues/PRs, and read this document plus AGENTS.md, PLAN.md, TICKETS.md, specs/*, research/* and status/parity.md. No local archive, unpublished branch or old agent is required to resume. Discover the new environment's actual tooling/concurrency rather than assuming the previous seven-agent limit.

## Goal, authorization and constraints

Continue toward full official Kotlin LSP feature parity in Rust. The native server implementation, build, core analysis and default native tests must require no JVM/JDK, compiler analysis process, JNI analyzer or C/C++ parser. Bundle no Java. Reading class/JAR/source/metadata is Rust data processing; Kotlin/Java fixtures are test data.

The user explicitly permits optional invocation of their existing JDK/Gradle/Maven tools for project import/build/run/debug, and an isolated official JVM oracle. This does not permit replacing the Rust semantic engine with a JVM analyzer. The oracle distribution is research-only and is neither committed nor required by the native build. See specs/runtime-boundary.md for the current policy; it supersedes the early research's blanket zero-Java execution assumption.

The user authorized ticket creation, branch pushes, PRs and merging passing independently reviewed work. Continue without repeated confirmation for routine authorized delivery. Each implementer owns an isolated worktree and explicit files, obtains two reviewer slots, launches separate correctness/functionality and adversarial edge-case/crash review agents, fixes and rechecks findings, opens/attaches a PR, waits for hosted CI/integration checks and the coordinator's exclusive merge-window grant, merges with the reviewed expected head SHA, and reports URL/SHA/tests/limits to the coordinator. Never bypass protection or label unresolved findings as passing. Shared Cargo.lock integration is serialized through this gate; sandbox approval settings are not implied by delivery authorization.

Use as much real concurrency as the environment supports, with dependency-ordered admission and reserved reviewer capacity. Broad semantic/import/intention contracts need smaller fixture-specific subtickets before implementation. Coordinate root Cargo/lockfile/status edits. Keep user progress updates useful and no farther apart than about a minute during active work.

## Completed work

Research/specifications/workspace/native CI are merged in PR49. T001–T004 are merged in PR50/52/53/54: strict UTF-16 atomic document edits, bounded framed transport, lossless bounded Kotlin lexer with opaque nested standard/multi-dollar strings, and validated explicit data-only project model v1. PR55 publishes the isolated official-server observations and clarified runtime boundary.

T005 is merged in [PR56](https://github.com/anilbeesetti/ktlsp/pull/56), merge commit `8ea54a9bee315713e7fc74c713eb1954901b5657`: runnable native stdio server, explicit lifecycle, JSON-RPC validation, bounded document store, UTF-16 edits, immutable Session queries and CLI help/version. Both independent reviewers approved; local and hosted checks passed. Historical checkpoint `0d54973` had 76 native tests and no language providers.

Current documented checkpoint `19e6242cde54146c5e8f988379e36e771562ac7e` includes PR65 hygiene, PR70 optional recorded replay, PR72 bounded classfile structure, PR76 pinned exported-model subset, PR73/T006, PR75 parser safety, PR74 offline artifact lookup and PR92 revisioned parser results/corpus and PR79 declaration annotations. Recorded workspace runs passed 155 active native tests plus two ignored subprocess helper fixtures invoked by active tests, with local macOS arm64 and hosted Linux execution. The server advertises synchronization and folding, and publishes bounded lexical/structural replacement diagnostics on accepted open/change and clears on close. Only matched multiline delimiters, block comments and raw strings fold; semantic/compiler diagnostics, document symbols and all other semantic providers remain pending. See [server behavior](../crates/ktlsp-server/README.md), [parity](status/parity.md) and [exact delivery evidence](status/development.md). Binary/import/artifact libraries have no server consumers; their bounded data subsets do not establish library semantics or build import.

Local verification of this checkpoint is not consistently green: all eight parser declaration tests also repeatedly returned Timeout under the 500 ms deadline, including an isolated default-harness run with the expected worker/source identity. [Failure evidence](https://github.com/anilbeesetti/ktlsp/pull/79#issuecomment-6126376895) retains both failures and passes. Cause remains unknown; passing 155-test runs and hosted CI do not establish a fix. No deadline/assertion was relaxed. [Diagnosis #96](https://github.com/anilbeesetti/ktlsp/issues/96) remains open without a fix claim. Guarded integration requires strict matching/current validation; stop on a fresh failure.

T001–T006 issues #1–#6 and children #58/#59/#60/#61/#62/#63/#69/#86 are closed. T007–T048 are issues #7–#48; T049 (advertised hierarchy language coverage) is issue #51. Verify current GitHub state before assignment. Parents #7/#17/#18/#20/#21/#44/#45/#46 remain open; closing infrastructure children does not mean semantic parity.

## Research baseline and adoption hazards

Target official release `kotlin-lsp/v263.6379.0`, public tag source `4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748`; master source `5c707583b37f43f9d54a85f823b2bb5f8182999c` was separately researched. Public source is a partial IntelliJ/Kotlin mirror with closed-source core. Do not infer runtime capability behavior solely from provider source.

Community winlogon/ktlsp is JVM-dependent, useful for behavior/test intent rather than implementation reuse. Tree-sitter Kotlin compiles C and is excluded. Internal pure-Rust ktrs 0.5.1 crates are promising, but research reproduced exponential parsing on tiny malformed input and token loss. Before adoption require bounded work or supervised Rust-worker isolation, lossless validation, crash/stack/memory defenses, audited normal/build dependencies and retained notices. Root facade/CLI dependencies differ; do not adopt them blindly. Native formatter smoke is not formatting parity. REA was inspected but not installed because source inspection/protocol capture sufficed; use it only if needed.

Official Linux release archive SHA256 is `ab8ca4455dc2fc5fe1a24db2bccc46c104254d2c465155c4251ee65df8f3f7cc`. docs/research/oracle records captured decoded JSON and independently authored source with provenance. Initialize/folding/shutdown/exit and basic standalone symbols/hover were observed. Global typeHierarchy advertisement does not prove Kotlin language support; standalone Kotlin prepare returned null. Source/binary notification discrepancies are recorded. PR70 provides optional standalone Rust replay of existing observations with genuine baseline differences and matching shutdown/exit; PR73 compares/replays the block fixture with field differences retained. T045 still needs a full corpus, dynamic interactions and process-tree supervision. No fresh JVM ran in this wave. Do not assume old local caches or official binaries exist in a fresh environment.

## Next tickets

1. T006/#6 is delivered. Extend folding/oracle coverage through new accepted children; retain explicit lexical/structural limits and recorded field mismatches. Cancellation/incrementality remain T043/T044.
2. T007 safety child #62 is delivered, with no server caller/full grammar/HIR/semantics. The supervised trusted direct worker accepts 64 KiB versus 8 MiB documents, uses a 500 ms deadline subject to scheduling, and caps cumulative Rust allocation requests at 64 MiB rather than RSS/stack. Conservative delimiter preflight and caller-managed concurrent admission remain limits. [T007b/#83](https://github.com/anilbeesetti/ktlsp/issues/83) has a delivered revisioned result/corpus child #86 (PR92), but named missing-parameter/generic recovery failures still block #83 and T008a; the [bounded declaration corpus](../crates/ktlsp-parser/CORPUS.md) does not establish full syntax parity. T009 scopes, T010 imports, T011 modules and T012+ types/inference/overloads/flow follow accepted prerequisites. AST/CST or handler names do not establish semantics.
3. Split independently ready binary metadata, dependency resolution and existing-tool project import tasks into small accepted contracts and run in parallel. Core build/tests must still work without Java.
4. Continue semantic providers, inspections/quick fixes, safe rename/move/extract/inline/intentions, formatting/import actions, hierarchies/hints, official custom protocols, incremental cancellation/performance, isolated oracle verification and native cross-platform packaging.

No percentage/parity claim is justified while required matrix rows remain partial, pending or unverified. Native-tested and oracle-verified are different. Preserve symbol identity and document versions for edits; no name-based replacement shortcuts.

The current native wave does not run Gradle/Maven or a fresh JVM oracle. Preserve completed research/provenance rather than repeating discovery. Follow-up child proposals in the development ledger remain gated on accepted prerequisites. Native performance baselines do not justify comparative faster/lower-memory claims without equivalent workloads.

## Validation and delivery

Rust toolchain is pinned 1.99.0 with MSRV declared 1.90. Run:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Hosted Native Rust checks must pass. Build/run instructions are in README.md. Do not rerun broad checks without a changed integration/failure/concern that warrants them. The previous Codex PR attachment tool stalled; invocation was attempted, so do not assume old PRs are attached in the app. GitHub PR/merge URLs are authoritative; use attachment tools if available in the next environment.

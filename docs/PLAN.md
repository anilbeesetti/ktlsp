# Delivery plan

Research precedes specification and implementation. Target full official Kotlin LSP parity while retaining a Rust-only runtime/build. Complete work in dependency-ordered waves with up to seven live agents; hundreds of concurrent agents are not available in this environment. More ticket count does not remove the semantic engine dependency chain.

1. Pin sources/releases, audit feature contracts and compiler/project dependencies, and record unknowns. Review the research/specification before coding.
2. Establish repository, Cargo workspace, native-only CI and small foundational tickets: UTF-16 text, framed transport, lossless lexer, explicit project model. Four isolated implementers can run concurrently; reviewer admission is coordinated to keep two slots available.
3. Integrate a runnable stdio server with lifecycle/document synchronization and honest capabilities. Add subprocess and crash coverage. Add syntax-backed folding/document symbols/syntax diagnostics only after parsing semantics support them.
4. Evaluate/adopt a robust full Kotlin parser; implement syntax recovery and stable HIR/scopes/imports. Begin semantics with declarations and types; expand with constraint solving, overloads, CFG/smart casts and library symbols. Treat each language chapter as its own accepted corpus.
5. Build pure-Rust binary metadata, Java interoperability, native dependency/project imports. Surface dynamic Gradle/Maven and plugin gaps rather than guessing resolved models.
6. Implement semantic completion/navigation/diagnostics, then safe refactoring/import actions/formatting and hierarchies/hints. Audit official custom protocols and run/debug boundary.
7. Differentially validate the pinned release, tune incremental performance, package native cross-platform binaries, and publish a row-by-row parity report. Full parity is blocked until semantic, dynamic-build and oracle gaps are resolved.

Every implementation ticket must have a small concrete result, dependencies, ownership, tests, two independent reviews and a PR. Implementers open/merge their passing PRs and report exact SHA/URL/check evidence to root. Dependencies are merged before dependent branches are started or integrated. The coordinator keeps the matrix and issues current and reruns whole-workspace checks on integrated main.

No date estimate or claim of perfect behavior is justified yet. A useful native server can be delivered incrementally; the entire IntelliJ/compiler feature surface cannot be declared complete from initial syntax features.

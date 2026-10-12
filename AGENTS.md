# ktlsp contributor and agent instructions

The user's goal is full official Kotlin LSP feature parity in Rust only. Read README, docs/PLAN.md, docs/specs/* and research/status before changing feature scope. Never claim full parity from a subset implementation.

No JVM/JDK/compiler execution or C/C++ analysis dependency in runtime/build/tests. Do not execute Gradle/Maven. No tree-sitter C dependency. Data artifacts can be read. The user permits the official JVM oracle in a separate test environment only; oracle tooling must never enter the shipped server or required native build/test dependency graph. Coordinate isolated oracle work with root.

Work in a ticket-specific git worktree/branch; never modify another agent's worktree. Coordinate Cargo workspace/dependency/lock changes with root. Keep public feature status honest and capabilities restricted to working providers.

Implementers must obtain two review slots from root then spawn separate reviewers for code/functionality and adversarial edge cases/crashes. Fix actionable findings; have changed behavior rechecked. Run format/clippy/relevant tests and full-workspace tests after integration. Push only your branch, create and attach PR, wait for required CI, then merge the reviewed head with expected SHA after root confirms integration is clear. The user already authorized passing PR merges. Report PR URL, merge SHA, tests and residual limits. Never bypass branch protection or force-push main.

Use structured arguments or --body-file for issues/PRs. Do not expose credential values. Preserve third-party notices if code is reused.

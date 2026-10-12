# Production integration contract — #87

Coordinator transferred parser ownership after #86/PR92 delivery. The final
preparation integrated exact main758e53b17566e4fde908705fc090d516ba4b0065 once
at b3bdc97cd21c181582a31fdf541f3f3ca7c880d9, preserving the independently reviewed
formatting phase70bad3e and both source patches/inventories. Both independent
source-only integration reviews approve that merge.

Controlled validation uses existing Linux Native Rust CI for all three unchanged
pinned commands on the exact published head. Local pinned format and Clippy
passed on the final integrated implementation. Locked metadata/tree and patch
reproduction audits also passed. Earlier Darwin runtime checks are historical;
no new Darwin parser execution is used to obtain a passing result. Issue #96
remains OPEN/UNKNOWN. The PR records exact-head CI, both final behavioral reviews,
finished architect review and expected-head coordinator merge grant. No merge or
full recovery/parity acceptance follows from preparation alone.

Authorized graph changes:
- `crates/ktlsp-parser/Cargo.toml` direct dependency becomes
  `ktrs-parser = { version = "=0.5.1", path = "../../vendor/ktrs-parser-0.5.1" }`.
- Root Cargo.lock removes only ktrs-parser's registry source/checksum; its
  dependencies and every other package remain unchanged.
- Root workspace adds only `exclude = ["vendor/ktrs-parser-0.5.1"]`, explicitly
  approved after Cargo auto-enrollment conflicted with the standalone workspace.

No new package/external dependency, syntax/server/common API/limit change. Existing
supervised worker stays the sole product invocation. The fork and optional
worker-probe have standalone workspaces/locks; default native tests do not enroll
upstream dev dependencies or tests. Original source inventory/checksum/notices
remain preserved. recovery.patch plus the separate formatting.patch reproduces
all 54 final source hashes; no formatting opt-out is used. Locked metadata
confirms nine workspace members and the direct vendor parser path. Normal/build
and all-feature dependency inventories contain only the existing pure Rust
closure; Clippy compiler output explicitly names the vendor parser directory.
Exact-head Linux CI must independently compile/test that production graph.

Production declaration tests replace three parameter-list known-defect assertions
with exact `parse_snapshot`/`tree_for` restoration expectations. Generic recovery
and token-loss Coverage remain known blockers. Tests cover original LF/CRLF/lone
CR/Unicode/backticks, immediate/nearest parents, malformed boundaries/diagnostics,
annotation prefixes, repeated recovery/EOF, valid nested defaults and member
initializer/constructor contexts. Production recovery_limits tests exercise
annotation-probe work, postparse nesting, repeated recovery below preflight
nesting, balanced-delimiter diagnostic overflow and healthy followups. Existing
fault/stale/source mapping/budget tests remain strict.

Both reviewers must recheck production behavior; architect reviews finished work.
Pinned fmt/clippy/workspace tests run in exact-head Linux Native Rust CI during
the coordinator-approved validation turn. #83/#7/T008 remain gated on joint recovery assessment, especially
unfixed generic/default/deep type/annotation recovery; this repair alone is no
syntax/parity unlock.

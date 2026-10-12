# Production integration contract — #87

Coordinator transferred parser ownership after #86/PR92 delivery at0ac012fd;
consumer development integrated main19e6242, then the explicitly released
preparation target583a0adc32c79c1a68257855c4ee7432f0088796 once (merge8d833c12).
Both source-only integration reviews approve the original corpus, graph and
provenance. Execution remains reserved for #95: actual dependency resolution,
production checks, behavioral reviews, hosted CI and expected-head merge grant
remain pending. No parser lib.rs or shared status document was edited.

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
remain preserved. Verify actual resolved normal/build/default-test graph and
workspace membership after the coordinator grants a local check slot.

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
Pinned fmt/clippy/workspace tests and exact-head hosted CI follow the root's stable
integration turn. #83/#7/T008 remain gated on joint recovery assessment, especially
unfixed generic/default/deep type/annotation recovery; this repair alone is no
syntax/parity unlock.

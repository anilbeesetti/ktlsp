# T045a review and verification

Child #59 of #45. Ownership: standalone `tools/oracle/**` and new `docs/research/oracle/comparisons/**` only. Native/root Cargo manifests, lockfile, CI and original official observations remain untouched. Default workspace metadata contains only project/protocol/server/syntax/text crates; replay tooling is optional.

2026-10-12, pinned Rust 1.99.0, macOS:

- Repository `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace`: pass, 76 native tests.
- Optional tool fmt, locked all-target clippy, locked tests: pass, six integration tests covering committed evidence loading, exact semantic differences, malformed evidence, framed Unicode subprocess replay, CLI exit outcomes, timeouts/blocked writes/partial frames/floods/server requests/duplicate IDs.
- Native lifecycle-folding replay reproduces committed report: initialize/folding/captured notifications differ, shutdown null and recorded exit 0 match. No JVM execution/download. Hashes/reproduction command and evidence limits are in comparisons README.

Separate correctness reviewer independently replayed native baseline, verified all original fixture/native-executable hashes, exercised malformed scalar params and caught one P2 (scalar `params` accepted). Shared evidence/live-envelope validation now rejects scalar params while preserving null/object/array. Reviewer independently rechecked numeric/string/boolean evidence and scalar live notifications, six tests/clippy, and exact report reproduction; approved.

Separate adversarial reviewer independently exercised numeric-versus-string IDs, null IDs, result+error, late duplicate response, deep JSON, oversized declared frame, scalar notification params, and late extra notifications. Failures were controlled and extra notifications reported as differences. A child closing stdout/stderr then hanging after exit hit the process-exit deadline (~5.024 seconds); its PID was absent after return, confirming direct kill/reap. No additional actionable findings.

Architect reviewed the plan before implementation and finished work before delivery. Hosted native check results and exact reviewed head are recorded on the PR; hosted default CI intentionally does not run optional tooling tests.

Residual limits: direct-child cleanup only; sequential recorded requests and no dynamic server requests; original URIs and machine-specific logs retained; caller-declared native revision plus externally recorded hashes; decoded originals lack raw-wire/full-tail coverage; no fresh official/macOS oracle run, full corpus, semantic/performance parity or complete T045 claim.

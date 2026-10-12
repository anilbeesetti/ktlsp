# T017a / #58 review evidence

Reviewed source SHA256: `7fa84acd8923291a14370e9ac774aeabb9d7ee2d2f8be2f867bc7b49978c27f0`.
Reviewed permanent test SHA256: `c2f15307ab338d81e18d9760dc6730ee3dde4d5bd4aa7882bbd6dc70feb4e020`.
Base: `0d54973ce66f59362829360261fde6e46b4fc420`.

Separate correctness and adversarial reviewers independently found P2 acceptance of unused method-shaped NameAndType `<clinit>/()V`. Fixed in the shared pool validator, with regression preserving the legal field-shaped `<clinit>/I`. Both independently rechecked the changed behavior and reported no unresolved P0/P1/P2 findings. The owner's historical pre-51 class-initializer correction was also independently rechecked.

Correctness reviewer ran the 11 permanent crate tests and 5 temporary independent probes: descriptor cases, historical/current initializers, method/field NAT distinction and exhaustive modified UTF-8 roundtrips for all 65,536 UTF-16 units. Adversarial reviewer ran 11 permanent tests and 6 temporary independent probes: 96 tag/reference combinations (zero/wide/self/out-of-range), byte mutations, maximal modified UTF-8 strings, repeated long references, exact record and 16MiB input boundaries, and initializer/slot matrices. Temporary probes were removed from the worktree after review. Their evidence was reported to the owner; no JVM/compiler/build tool was invoked.

Finished-work architect review inspected matching hashes and independently ran the 11 crate tests; no remaining actionable findings in scope. Reviews establish this bounded structural subset only, not JVM verification or official differential parity.

Pinned Rust 1.99.0 post-fix validation passed:

- `cargo fmt --all -- --check`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace`: 87 permanent tests, including 11 binary tests and existing subprocess/crash coverage.
- Optimized deterministic mutation/random-byte corpus passed (one test, 0.01s test-reported time on the development Mac). This is a bounded smoke measurement, not a comparative benchmark or portable performance guarantee. The time wrapper includes compilation; reviewer memory telemetry was sandbox-blocked.

Hosted CI, exact PR head and merge evidence are recorded in the PR/coordinator report after creation. Parent #17 remains open. README documents the version ceiling, resource caps and opaque/deferred behavior.

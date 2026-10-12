# Isolated unchanged-source worker probe

This optional standalone Rust workspace references the existing production
supervisor, worker source and parser tests read-only. Its local patch selects the
owned parser fork without changing the product's Cargo.toml/Cargo.lock. Native
product build/tests do not depend on this harness. No source is rewritten or
inserted, and no worker implementation is copied.

Run `cargo test --offline --locked --manifest-path vendor/ktrs-parser-0.5.1/worker-probe/Cargo.toml --tests`.
Six existing integration tests, two worker unit tests (including an invoked
allocation-denial child), and two original probes exercise current gates.
Source recovery covers exact malformed/later declaration extents, kinds, nearest
parents and diagnostics over top/member/local x LF/CRLF/lone-CR with Unicode.
Nested annotation depth <=16 is accepted; tested deeper trees return NestingLimit
and every case has a healthy followup. These are bounded observations, not
performance guarantees or full grammar/official parity.

This remains isolated evidence, not delivered production integration. Equivalent
regressions now live in the parser crate and await execution against the actual
root dependency graph, both integration reviews and hosted CI. Root
cargo test --workspace does not run these standalone tests.

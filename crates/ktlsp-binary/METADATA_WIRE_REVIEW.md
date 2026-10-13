# #108 delivery evidence

Scope and compatibility decisions: METADATA_WIRE_SPEC.md. Source/data identities
and preserved Kotlin/protobuf licenses: tests/fixtures/metadata_wire/README.md.
Parents #68/#18 remain open. Issue #96 remains open with unknown cause.

Coordinator and architect granted this finite implementation/publication on base
4d27673f703fd3c7b0ee57fc7a3cf35c6c21f05f. Header/d1 implementation, existing tests,
Cargo/lock, parser/vendor/worker graph, workflow and server providers are unchanged.
Only additive metadata/mod.rs export and new wire source/tests/fixture/spec/evidence.

Local compile-only validation: cargo fmt --all -- --check and
cargo clippy --locked --workspace --all-targets -- -D warnings passed. No local
native tests, parser/runtime/performance probes, JVM, Gradle, Maven or oracle ran.

Nine wire integration tests are committed for existing Linux Native Rust CI:
independent mixed bytes/borrowed spans, empty/nonminimal framing, every-byte
truncation/section bleed, varint low-bit/nonminimal policy, exact malformed offsets,
aggregate budgets/failed-work precedence, hard byte/record clamping, supported
header composition with data-only Unit.class, deterministic hostile-byte corpus.
These tests have been compiled locally, not yet executed at this evidence stage.

Independent functionality, adversarial and architect completed-source reviews found
no actionable source blockers. The adversarial review's independently expected
section-transition probe was committed; both counters remain shared at clean EOF.
Final approvals remain conditional on recheck and completed Linux CI evidence.
Exact reviewed head/tree, completed existing Linux all-three pinned
workspace commands, test counts/URLs and final approvals will be recorded in the
PR/coordinator report. Only child #108 is eligible for closing. Merge still requires
root's one stable integration turn and expected-SHA grant, then exact merged-main
Linux checks. Pending gates are not represented as passing by this source record.

Limits: flat raw framing/fields only, no embedded schema/required-field/name/member/
type semantics or parity claim. Deterministic low64 follows pinned slow path;
nonminimal ten-byte forms can differ in the pinned JVM fast path. Negative/short
envelopes are intentionally rejected more strictly than pinned LimitedInputStream.
Rust native/Linux checks do not establish Darwin reliability or a #96 correction.

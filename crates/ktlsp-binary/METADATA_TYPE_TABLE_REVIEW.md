# TypeTable directory review record

Contract preparation at cff36d17cd5cd1386059ecc91572013fdb417b48 was independently
approved by architect,correctness/functionality and adversarial reviewers. They
verified pinned schema/reference hashes,exact literal files,54 prefix cuts,raw
signed scalar behavior,capacity/work arithmetic and error offsets. Initial
wrong-wire wording was corrected to accepted scalar forms1/2/5 before source.

The same three reviewers approved actual source at da543e56070a66a11b4989e72aaaf29347af5ec7
(tree f968e90fcfdcc80b0fc9b420b712f2aee5bfa80f) with no actionable findings.
They read the decoder,all eight integration tests,spec and fixture provenance;
checked exact14-path scope and single additive module export; and independently
compared frozen expectations,literal files and license/notice copies. Aggregate
wire/dispatch/capacity admission and raw scalar/order behavior match the contract.
Same reviewers retain changed behavior and completed original CI responsibility.
Runtime approval is pending. Local cargo fmt --all -- --check and compile-only
cargo clippy --locked --workspace --all-targets -- -D warnings passed. No local native
product/parser/worker/server/tests/bench/JVM/oracle runtime was executed.
Requested logical capacity excludes allocator surplus/churn/RSS. Allocation
failure mapping remains static only. Full nested Type initialization,types,
nullability application,linkage,semantics and full metadata parity are excluded.

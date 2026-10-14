# TypeTable directory review record

Contract preparation at cff36d17cd5cd1386059ecc91572013fdb417b48 was independently
approved by architect,correctness/functionality and adversarial reviewers. They
verified pinned schema/reference hashes,exact literal files,54 prefix cuts,raw
signed scalar behavior,capacity/work arithmetic and error offsets. Initial
wrong-wire wording was corrected to accepted scalar forms1/2/5 before source.

Same reviewers retain actual source,changed behavior and completed original CI
review responsibility. Source and runtime reviews are pending. No local native
product/parser/worker/server/tests/bench/JVM/oracle runtime was executed.
Requested logical capacity excludes allocator surplus/churn/RSS. Allocation
failure mapping remains static only. Full nested Type initialization,types,
nullability application,linkage,semantics and full metadata parity are excluded.

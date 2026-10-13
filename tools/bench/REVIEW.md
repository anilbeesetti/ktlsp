# T044 preparation child #64 review evidence

Product checkpoint: `0d54973ce66f59362829360261fde6e46b4fc420`. Scope is tools/bench only, independent workspace/lock; no product source/default native graph changes. Parent #44 remains open.

Architect plan review required bounded writes/cleanup, isolated checkpoint dependencies, precise timing labels, stable edits and raw sample/percentile/batch evidence; implementation and README address these. The later memory re-review below supersedes the initial unavailable result.

Independent functionality reviewer read implementation, ran standalone fmt/clippy/all six tests, exercised the archived server's initialization, malformed JSON recovery, Unicode notifications, unknown dispatch, shutdown and four fatal framing cases. Rechecked all-ten-fixture malformed corpus timing and worker input fix. No unresolved actionable findings.

Independent adversarial reviewer ran all six tests and separate supervisor probes: 100 ms finish deadlines returned in approximately 107–111 ms for silent/partial peers, noisy output failed promptly, oversized input rejected, cleanup bounded. Found minor worker argument acceptance mismatch (`check` extra args and edit corpus aliases); fixed with explicit validation and regression assertions, independently rechecked to exit 1. Validated missing metadata fails as JSON, ten malformed fixtures sum 66 bytes, and standalone locked package versions/checksums all exist in the product lock. No remaining actionable findings within direct-child contract.

Both reviewers rechecked the initial latency-only main.rs SHA-256 `34b7166754b0cd50bc151d0be43fccc2121be0ee26e9e7b254047f66db16cf95`. Initial process.rs SHA-256 `3601a552698b67e2470474ba1fc2123edec04e7a260555d81ec6b39c2cf1541e` was unchanged during that review.

Local Rust 1.99.0 checks passed:

- Product: cargo fmt --all -- --check; cargo clippy --locked --workspace --all-targets -- -D warnings; cargo test --locked --workspace (76 tests).
- Standalone: cargo fmt --manifest-path tools/bench/Cargo.toml -- --check; cargo clippy --locked --manifest-path tools/bench/Cargo.toml --all-targets -- -D warnings; cargo test --locked --manifest-path tools/bench/Cargo.toml (eight tests after RSS integration).
- Owned checkpoint archive release build and a five-repetition whole-harness smoke run passed. Smoke timings were contested/provisional and are not a recorded baseline.

## Required memory completion and re-review

Coordinator clarified that actual memory measurements are required for child #64. Architect approved a small native `wait4` collector instead of a platform `time` wrapper, preserving direct-child supervision and one reaping path. Support is restricted to verified Darwin arm64 SDK layout, with compile-time size/alignment/offset assertions. Missing-child status fails measurement and prevents later signalling; status/RSS are cached after successful or killed-child reap.

Both independent reviewers rechecked the changed supervisor, memory schema/lifetime labels and tests. Functionality ran all eight tool tests plus fmt/clippy, inspected SDK definitions and found no actionable issues. Adversarial ran all five subprocess tests and independent probes with eight 32 MiB peers, cached finishes, killed children, no-zombie checks and all four failure cleanup modes; no actionable findings. Both observed the exact ABI and ECHILD policy.

Current reviewed SHA-256:

- main.rs: `d67665ae845f78d081c75069b68531cb3dd83832657ed8baa39138f7716c1294`
- process.rs: `04591aa48024212435bf04adbeadc36c4d19ba8e5a3282933328516af4e980bd`
- README.md: `663be7778cb101b1d94592e890ca21278108d679b28c8f8336d720f99735df07`

An independent `/usr/bin/time -l` run of the 32 MiB touched-memory peer returned 35,487,744 bytes maximum RSS, matching the adversarial review's wait4 probes. The utility needed approved platform resource access outside the sandbox; the Rust wait4 tests and harness run inside it. An updated owned-checkpoint five-repetition smoke passed all eight latency workloads and returned positive measured RSS for each. Those contested smoke timings are not a baseline.

Docs-only main integration `de9b6b1` introduced no tools/source change; pinned workspace fmt/clippy and 76 tests passed again. Hosted native CI passed that integration head `c922a9fd68e4a370d795d094bcb222f35eed3765` at https://github.com/anilbeesetti/ktlsp/actions/runs/38250558225/job/114815432642; it does not validate the later memory revision.

Quiet-window collection is recorded below; final results-head hosted CI remains the delivery gate. No official oracle, other-platform RSS support, parser/semantic workload, retained/per-operation memory instrument, confidence interval or performance gate was added.

## Recorded quiet collection

Coordinator granted a bounded quiet window and paused active source owners. The prepared archive run completed in 2.392 seconds at 2026-10-12 12:58 UTC. All eight workloads have 101 positive finite latency samples and positive whole-child RSS; startup has 101 lifetime peaks, workers/steady server have one each. Names/counts and load observations found no competing known build/test process. Metadata, byte/case counts, checksums, raw samples and scope limits are preserved in results/.

Integrated main through `8fee6f438945e1c9394ecd7e948893ca6c179134` (classfile, replay, import additions) at `b4836508d686dcc7bd190a84621aacc29fde19ae`; no tools/bench or measured checkpoint-library source changed in that integration. Pinned native fmt/clippy and all 100 workspace tests passed; standalone fmt/clippy and all eight tool tests passed. Memory source head `0b43c7f` hosted CI passed at https://github.com/anilbeesetti/ktlsp/actions/runs/38251687235/job/114819342302. Final results/doc head still requires review and hosted CI before merge.

Both independent reviewers validated the recorded data: all 808 finite positive latency samples, nearest-rank summaries/throughput/table rounding, byte/case counts, RSS observation counts/scopes, binary/source hashes and window observations. Functionality confirmed only provenance metadata was appended after capture. Adversarial also compared all 22 tracked product manifest/lock/toolchain and measured-crate files in the owned archive against checkpoint git objects byte-for-byte. No timing rerun was performed. Result SHA-256 `291a83aeeefd65b0d39b3a8d8fd520b0ea85e6ed6ec72683e67c3cfc50dba93e` is independently verified. Final data/doc-only commit changes no reviewed Rust behavior or native test graph.

Results head `b8d868b9cfabc1def9f625d1b601202435f52b65` passed hosted native CI at https://github.com/anilbeesetti/ktlsp/actions/runs/38253266037/job/114824688747.

## Lexer peak investigation follow-up

Coordinator requested investigation of the large recorded lexer RSS before any optimization claim. Both reviewers independently used native counting-System-allocator probes on the owned checkpoint: requested live bytes returned to baseline after every pass, with bounded peaks near 13.73 MB / 6.55 MB, while whole-child peak RSS reproduced the recorded scale. A plain growing `Vec<Token>` control reproduced it without lexer work; exact-count preallocation was a diagnostic control only. End-of-run allocator-zone accounting is distinct from current RSS. Exact Darwin allocator/VM attribution remains unproven.

Architect and both reviewers checked the narrow follow-up [#85](https://github.com/anilbeesetti/ktlsp/issues/85), its exact reproducer/control source and limits. The issue reserves exclusive syntax ownership for a later coordinator dispatch and permits an evidence-based no-change outcome. No product optimization belongs to this baseline PR.

The results README clarifies 112 total calls (one validation plus ten warmups plus 101 measurements), exact token counts/capacities, and the original capture's missing allocator environment. Later probes inherited `MallocNanoZone=0`; the original setting cannot be independently established. Raw samples, summaries and recorded peaks are unchanged. Documentation corrections require final-head hosted CI; no timing rerun is warranted.

## Source-only frozen-main preparation

Restored the vanished managed checkout from published branch head `04b736e566d200ee8ba03001ba94e70d3df46fe3` into the ticket owner's fresh `/private/tmp/ktlsp-77-source-preparation` clone, without reusing build artifacts. Integrated only frozen main `143c0be503cb71b2f5cd8fb4fe7d7272d4328995` once; the automatic merge had no conflicts. The integration's non-benchmark files come unchanged from frozen main. Only this review note and the harness README's provenance clarification are preparation edits.

Classification: **source-reviewed/runtime-unvalidated**. No new Cargo/build/test/benchmark, allocator/RSS/latency, JVM or CI runs are part of this phase; no push or merge is authorized. Prior review/test/CI records above are historical and do not validate this preparation. Focused independent correctness/adversarial static reviews and architect finished-work review are required before reporting the clean local committed head; final stable integration/review/check/expected-head merge/main gates remain pending. #96 remains open/unknown.

Immutable measurement identities remain product `0d54973ce66f59362829360261fde6e46b4fc420`, harness source `0b43c7f15db440437e6f1cd08d8affa79d701358`, original recording integration `b4836508d686dcc7bd190a84621aacc29fde19ae`, and raw-result SHA-256 `291a83aeeefd65b0d39b3a8d8fd520b0ea85e6ed6ec72683e67c3cfc50dba93e`. Recorded server/harness binary checksums, raw samples/window observations and original environment limitations are untouched. Old measurements establish no current-main or official-LSP superiority. This base update does not require remeasuring the historical checkpoint; any request for new measurements must wait for coordinator admission.

## Final stable integration turn

Coordinator approved one integration of exact main `e2c643d60803f6580cc42a92c722ea337ce1b31a` (parent `da5b3b4e75b41168a8428cc7f9d0d445c42ab64b`, tree `9c748867827238bcce536742d153032bc0cdb189`) into clean prepared head `60c530ac0d0966059346babd859237673d91c36f`. The automatic merge had no conflicts; non-benchmark files match that approved main. Benchmark code, manifests, locks, tests and historical measurements are preserved. Only this review note and the README's final-phase clarification are new owned edits.

The final candidate requires separate correctness/adversarial reviews of the complete inherited delta and executable/test graph, architect review, and completed-log readback of all three unchanged Linux native commands: `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, and `cargo test --locked --workspace`. The actual CI checkout commit/parents/tree must be verified against the reviewed candidate and approved base. The native workflow and optional oracle-tool workflow are inherited unchanged from approved main.

No local native/parser/benchmark/runtime/performance/JVM/oracle runs or new measurements are authorized in this turn. Linux root-workspace CI excludes the standalone benchmark and cannot establish Darwin RSS correctness or current-main performance. Earlier native/tool checks and measurements remain historical evidence only. Merge remains held until the predecessor's exact-main checks pass, the final candidate's reviews/CI pass, and coordinator grants its expected SHA; exact merged-main checks are still required after admission. Close only #64; #44 remains open and #96 remains open/unknown.

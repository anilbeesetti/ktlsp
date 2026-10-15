# T006/T043 bounded lexical analysis reuse

Frozen implementation base: `45619398c7d8d5ca7b59acc5178acb4b208a67ee`.
Child #265; parents: #6 / #43. Native lexical subset only; no official oracle or semantic parity claim.

Owned files: `src/lib.rs`, `src/structure.rs`, new `src/cache.rs` (including six session-boundary unit tests), this review, and the bounds paragraph in `README.md`. No Cargo, lockfile, text, parser, capability or workflow changes.

Acceptance: diagnostics and folding for the latest exact document identity share one `structure::analyze` result, including resource-limit errors. URI/version identify content because Session's documents are private, accepted changes strictly increase versions, and successful replacement/close/reopen invalidates the entry. Fold limit is keyed; line-only and diagnostic-version settings are applied at response time. Existing unknown configuration notifications remain ignored. Rejected edits (including later-invalid UTF-16 batches and aggregate-size rejection) preserve document and analysis atomically. Responses keep existing fields, ranges, ordering and limit/error behavior.

Retention: exactly zero or one entry. A miss drops the old result before analyzing another document. Retain only one bounded URI, version/fold-limit scalars and the existing bounded raw fold/diagnostic records or static error; vector spare capacity is bounded but may exceed record counts. No source, token partition, rendered JSON or per-document map is cached. Alternating documents reanalyze; this is the intentional fixed retention policy, not an LRU or all-document reuse promise.

The six tests cover one analysis across diagnostics/repeated folding, committed/version-only edits, duplicate open/stale/invalid sequential edit rollback, real aggregate capacity rejection, close/reopen with repeated version, fold-limit and current line-only configuration, all 128 document identities/eviction, maximum output counts, cached structural exhaustion and recovery. Existing provider/stdio suites cover unchanged output behavior. Diagnostics-only rendering avoids materializing discarded folding JSON.

Reviews: retained correctness/functionality and separate adversarial reviewers independently inspected source and final documentation with no actionable findings. The architect approved the plan and finished implementation; its concern about discarded fold JSON was resolved by diagnostics-only rendering. Reviews were source-only under the runtime hold.

Local validation: `cargo fmt --all -- --check` passed; `CARGO_BUILD_JOBS=2 cargo clippy --locked --workspace --all-targets -- -D warnings` passed (exit 0); `git diff --check` passed. Hosted execution is pending at source commit; exact original run/head receipts belong in the PR/integration report.

#96 local runtime HOLD applies: no local product/parser/tokenizer/worker/server/test/bench/oracle binaries run; local formatting and compile-only Clippy are permitted, execution tests require original unchanged hosted Linux Native CI. T008/T008a HOLD, C13/B3/B4 GAP, B11 UNVERIFIED, #7/#83/#96 OPEN and Darwin UNKNOWN remain.

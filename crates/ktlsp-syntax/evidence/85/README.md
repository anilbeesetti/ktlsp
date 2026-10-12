# #85: bounded token-vector allocation investigation

Parent T044/#44 remains open. This is a Darwin allocation investigation over the existing lexical API, not semantic or official-server performance parity. No JVM, alternate allocator, native parser, production instrumentation, dependency or public API change.

## Decision and gates

Before strategy selection the latency gate was fixed at candidate median <= baseline median * 1.10 for the two dense fixtures; short/empty/sparse/transition fixtures allow an additional 100 ns. Uninstrumented fresh-child runs must alternate baseline/candidate order and report spread. A coordinated quiet period is required for acceptance, otherwise timing is exploratory.

Before choosing the candidate, the memory gate was fixed at final capacity <= twice the original policy and incremental requested peak <= twice original incremental peak + 96 bytes on each corpus. Incremental peak subtracts the already-built fixture/argument baseline. Sparse source must not trigger source-byte-sized token reservation. Portable tests assert allocation strategy and capacity, never RSS.

Exact-count preallocation isolates growth but is not an available production token count. Capped fourfold geometric growth passes the stated memory gate on `large`, but reserves 1,000,000 tokens versus the original 524,288 (1.907×), while lifetime peak RSS remains high. It was not selected because it uses extra capacity without resolving that corpus's RSS behavior. Density forecasting was unnecessary and would risk overallocating after a dense prefix followed by a sparse suffix.

The selected seven-line change keeps geometric doubling, but allocates a new vector, moves the old tokens with `append`, then drops the old buffer. It caps capacity at `MAX_TOKENS`. Empty input allocates no token storage; one-token input reserves four tokens. No retained cross-call buffers or source prepass. Total copying remains linear in token count. On 64-bit, the largest simultaneous token buffers are 524,288 + 1,000,000 tokens, or 36,582,912 bytes (34.89 MiB); source, diagnostics and bounded scanner contexts are additional. Allocation failure retains standard `Vec` behavior, not a recoverable `SyntaxError`.

## Memory diagnostics

See [metadata.json](metadata.json) for exact sources/binary hashes, Rust 1.99.0, default release profile, System allocator and macOS/Darwin versions. Apple M4 host (the issue's same machine). Historical `0d54973ce66f59362829360261fde6e46b4fc420` and current `09b77a3` have identical lexer source. Candidate is current plus the seven-line growth change, identified by its source SHA256. Original benchmark/historical results were not edited. Reference harness pin: `04b736e566d200ee8ba03001ba94e70d3df46fe3`; measurement source `0b43c7f15db440437e6f1cd08d8affa79d701358`.

[memory.json](memory.json) contains 258 fresh child observations: historical/current lex and four vector controls for each dense fixture, plus seven empty/short/sparse/transition corpora; candidate lex for all nine. Both default (`MallocNanoZone` explicitly unset) and `MallocNanoZone=0`, at total calls 1/5/112. Other inherited `Malloc*`/`DYLD*` overrides: none. The historical original capture did not record these variables; no retroactive allocator-environment claim.

Each result drops before the next call. **Every post-drop requested-live assertion passed**, including the validation call. Counters snapshot before the `ps` diagnostic/output allocations. Requested peak excludes allocator metadata and the old/new overlap internal to `System::realloc`; candidate explicit allocation exposes both buffers. Churn counts full new requested realloc sizes, not copied bytes. Realloc failure leaves counters unchanged.

`post_drop_ps_rss_kib` is an actual `/bin/ps` RSS sample after all results drop while fixture/arguments remain, in KiB; it is neither requested bytes nor the original historical run's residency. `lifetime_peak_rss_bytes` comes from `/usr/bin/time -l`, covers the complete child including setup/validation/output/diagnostic spawn, and is bytes on Darwin. Concurrent project work was possible during memory diagnostics. These observations do not establish Darwin's allocator/VM mechanism or a platform-general gain.

Selected current/candidate observations at 112 calls, `MallocNanoZone=0`:

| Corpus | Original/candidate capacity | Original/candidate requested peak B | Original/candidate post-drop RSS KiB | Original/candidate lifetime peak RSS B |
| --- | --- | --- | --- | --- |
| large | 524288 / 524288 | 13730640 / 20022074 | 497376 / 24800 | 517111808 / 25427968 |
| short_tokens | 262144 / 262144 | 6554431 / 9700161 | 399712 / 16192 | 415531008 / 16613376 |
| 8 MiB whitespace | 4 / 4 | 8389533 / 8389535 | 10128 / 10112 | 10387456 / 10387456 |
| dense→sparse | 4096 / 4096 | 8487743 / 8536897 | 18752 / 18864 | 19218432 / 19333120 |
| sparse→dense | 524288 / 524288 | 28836671 / 35128129 | 113792 / 33616 | 424984576 / 34439168 |

The larger requested peak is the known temporary old+new buffer cost. Both allocator environments satisfy the memory gates. Large comment/string source `String` capacity is approximately 16 MiB because fixture formatting grows it; this is included in baseline and requested peak, even though source length is exactly 8 MiB.

## Reproduce without touching the benchmark

Create owned archives of historical/current/candidate and copy this `probe` directory into each archive at the same relative path (exclude its `target`). The standalone workspace depends only on the archived syntax crate and is outside default native tests. Record actual source hashes and executable hashes; do not relabel an arbitrary binary.

```sh
cargo build --offline --locked --release --features counting --manifest-path "$archive/crates/ktlsp-syntax/evidence/85/probe/Cargo.toml"
probe="$archive/crates/ktlsp-syntax/evidence/85/probe/target/release/ktlsp-allocation-probe"
env -u MallocNanoZone /usr/bin/time -l "$probe" large 111 lex
MallocNanoZone=0 /usr/bin/time -l "$probe" short_tokens 111 lex
```

Probe arguments: corpus, extra calls after one validation, mode. For total 1/5/112 use `0/4/111`. Modes: `lex`, `vecgrow` (ordinary `Vec::push`), `exact` (known-count control), `vec4` (fourfold realloc growth), `veccopy` (allocate-and-move growth control). Vector controls are meaningful for dense fixtures only. Corpora: `large`, `short_tokens`, `empty`, `short`, `whitespace`, `comment`, `string`, `dense_sparse`, `sparse_dense`; their exact independently authored construction is in `probe/src/main.rs`. Dense fixtures match #85's 1,146,880 bytes/327,680 tokens and 262,144 bytes/tokens.

For timing build **without** `--features counting` into a separate target directory. There is then no counting allocator, `ps` spawn or assertion per measured call. A single validation call precedes the timed loop. Use positive extra calls (zero is a memory control, not timing). Source construction/partition validation/output are outside the timed loop; token scanning, result construction, black-box consumption and immediate drop are inside it. This probe measures batch-average latency per fresh child, not per-operation p95.

## Correctness and residual limits

`cargo test --locked -p ktlsp-syntax`: 12 tests, including deterministic 17 allocations/zero reallocations for repeated 262,144-token input, sparse tails around growth boundaries, exact max token capacity, existing Unicode/CRLF/malformed strings/comments/templates and all resource errors. Existing generated crash/partition corpus remains intact. Separate correctness review compared historical/candidate exact `Result` outputs across 8,217 cases; separate adversarial review compared 20,184 cases including growth boundaries, malformed UTF-8 text, sparse maxima and all failure boundaries. Here “malformed” means valid Rust UTF-8 with malformed Kotlin, not invalid UTF-8 bytes accepted by `&str`.

Owner pinned `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, and `cargo test --locked --workspace` passed on base `09b77a3`: 128 active native tests, plus two ignored subprocess fixture helpers exercised by their parent tests. Standalone probe fmt/clippy also passed. Both independent reviewers approved code and memory evidence after correcting an inaccurate fourfold-growth rejection rationale; the architect approved the memory tradeoff conditionally on latency acceptance.

[timing-exploratory.json](timing-exploratory.json) preserves all 252 uninstrumented exploratory child averages (nine corpora, seven alternating rounds, two revisions, two allocator environments). Metadata records exact pass counts and timing binary hashes. These runs were **not coordinated quiet measurements and are not accepted**: some transition medians exceed the predefined limit, with large spreads. For example default-environment dense→sparse baseline median 26,942,683 ns (24,737,933–95,125,916), candidate 38,546,200 ns (26,305,525–58,605,016); the candidate exceeds 1.10× + 100 ns. This does not distinguish scheduling noise from a real regression. A quiet run must resolve it before this candidate can leave draft or merge; the gate is unchanged.

No official JVM oracle or Linux/Windows residency experiment ran. Low Darwin RSS does not imply lower Rust requested peak, zero residency after drop, a proven allocator leak fix, or full performance/parity completion. Hosted CI, quiet latency and integration delivery evidence remain pending.

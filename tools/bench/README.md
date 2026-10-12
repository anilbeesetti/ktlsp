# Existing native checkpoint baseline (T044 preparation, #64)

Standalone measurement tooling for product checkpoint `0d54973ce66f59362829360261fde6e46b4fc420`. Parent #44 stays open: parser/semantic indexing, incremental queries, cancellation, workspace scale, memory budgets and performance gates require later work. No official JVM comparison or faster-than-official claim. This workspace is outside the default native build/test graph and adds no third-party packages beyond those already locked in the product.

## Reproduce

Use Rust 1.99.0, the platform linker and a fresh **owned** directory. Never point at another chat's binary or reuse its target directory. From the repository checkout containing this harness (record its `git rev-parse HEAD`, and any uncommitted diff):

```sh
checkpoint=0d54973ce66f59362829360261fde6e46b4fc420
bench_root=$(mktemp -d /private/tmp/ktlsp-baseline.XXXXXX)
git archive "$checkpoint" | tar -x -C "$bench_root"
mkdir -p "$bench_root/tools"
git archive HEAD tools/bench | tar -x -C "$bench_root"
cargo build --locked --release --manifest-path "$bench_root/Cargo.toml" -p ktlsp-server --bin ktlsp
KTLSP_BENCH_CHECKPOINT="$checkpoint" cargo build --locked --release --manifest-path "$bench_root/tools/bench/Cargo.toml" --bin ktlsp-baseline
rustc -Vv
shasum -a 256 "$bench_root/target/release/ktlsp"
```

Before commit, copy `tools/bench/{Cargo.toml,Cargo.lock,src}` instead of the second archive command; do not copy an existing `target`. Both server and harness must build inside the fixed archive: its path dependencies resolve to the archived source. The compile-time checkpoint marker guards accidental unprepared invocations; it is an attestation from the preparation command, not a cryptographic source/binary verification. Preserve the commands, source revision and binary checksum with the result. Do not label an arbitrary binary with this checkpoint.

Create `metadata.json` with **actual** values (replace every placeholder):

```json
{
  "product_sha": "0d54973ce66f59362829360261fde6e46b4fc420",
  "harness_revision": "git commit containing tools/bench; note dirty diff if any",
  "server_sha256": "shasum output",
  "toolchain": "complete rustc -Vv output",
  "profile": "release",
  "os": "OS release, kernel and architecture",
  "cpu": "CPU model and logical CPU count",
  "competing_workload": "other builds/tests/agents or quiet-window observation",
  "timing_status": "provisional-contested or coordinated-quiet",
  "build_environment": "default Cargo release; record RUSTFLAGS/target/profile overrides if set"
}
```

```sh
"$bench_root/tools/bench/target/release/ktlsp-baseline" run \
  "$bench_root/target/release/ktlsp" metadata.json 101 > result.json
```

Exit 0 plus `status: passed` means all checks and measured subprocesses completed. Failures/timeouts produce `status: failed` and exit 1; never use partial timings as passing results. Authoritative runs require a coordinator-granted quiet window; record unavoidable OS activity and thermal/power conditions. Five repetitions are enough for a smoke run only. Default recommendation is 101; permitted range 5–1001. No significance claim follows from one run.

## Work and timing boundaries

All correctness gates run before timing: lossless spans/diagnostic boundaries, the checkpoint's ten malformed-EOF lexer fixtures, all four lexer resource errors, atomic Unicode edit rollback/stale versions, live initialization/JSON parse recovery/Unicode change/shutdown, and four fatal framing inputs. The live server has no document query; direct document-core assertions establish text contents, while live notification checks establish acceptance without stderr. No semantic correctness is implied.

- Lexer: small Unicode/CRLF source; roughly 1 MiB repeated declarations/comments; 262,144 bytes of many short tokens; the ten malformed EOF fixtures as one corpus pass. JSON records exact byte sizes and case counts. Ten warmup batches; batch size 32 for tiny inputs, 1 otherwise. Timer includes allocation/scanning/result destruction. Tails for batched workloads describe **batch-average** corpus-pass time. Throughput derives from byte count and median latency.
- Document core: eight-line and 32,768-line Unicode/CRLF documents. Increasing versions, fixed-byte Unicode replacements cycling beginning/middle/end, ten warmups. Only `Document::apply_changes` is timed; source creation, edit construction and exact result comparison are outside the timer. Includes the current atomic copy behavior. Samples mix the three positions; no editor/network/analysis latency claim.
- Startup: a fresh server each repetition, spawn through initialize validation and the `initialized` write. Includes parent pipe/thread setup, excludes shutdown; executable/filesystem caches are not flushed, so this is warm-cache process startup, not cold-boot latency.
- Steady stdio: one initialized process, ten warmups, one unknown request and matching `-32601` per sample. Includes JSON/framing, pipe I/O, parent thread/channel scheduling and server dispatch; no language provider work.

Raw nanosecond samples, nearest-rank median/p95/p99, warmup counts, repetitions and corpus byte counts are in JSON. Workload order is fixed; no randomized order or confidence interval. `memory.status: unavailable` is deliberate: std has no portable process peak RSS API. Nothing here estimates per-operation allocation, retained memory or official memory efficiency.

## Bounds and checks

Every measured worker/server has a 30-second **whole-process** deadline, including stdin writes and exit, rather than a timeout reset by progress. Output is capped at 1 MiB per stream; framing also retains the protocol library's 16 MiB maximum single-frame allocation. Input is capped at 2 MiB; queued events/unsolicited frames are bounded. Failure drops the input channel, kills the directly spawned child and polls to reap for up to two seconds; cleanup failure is explicitly reported. Reader/writer threads are not joined without a bound. These known Rust workers/server spawn no descendants; arbitrary wrapper/process-tree supervision is outside this harness contract. Use the server executable directly.

```sh
cargo fmt --manifest-path tools/bench/Cargo.toml -- --check
cargo clippy --locked --manifest-path tools/bench/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path tools/bench/Cargo.toml
```

Tests include a Rust-only hostile test peer: blocked stdin, silent/partial output, noisy stderr/framed stdout, premature exit and clean framing/reap. `bench-test-peer` is a test utility, not a server. Run the repository's pinned fmt/clippy/workspace tests separately; they intentionally do not include this standalone workspace.

Queued workloads for later children: bounded full-parser recovery/ambiguity, semantic indexing and invalidation, cancellation storms, multiple modules/dependency archives, measured process RSS and retained-memory instrumentation. They depend on real providers and cannot be manufactured from this checkpoint.

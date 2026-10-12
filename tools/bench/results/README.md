# Recorded native checkpoint baseline

Measured 2026-10-12 at 12:58:04–12:58:07 UTC in the coordinator-granted quiet window. Product `0d54973ce66f59362829360261fde6e46b4fc420`, harness source `0b43c7f15db440437e6f1cd08d8affa79d701358`, integration head `b4836508d686dcc7bd190a84621aacc29fde19ae`. Both release binaries were built in the owner's `/private/tmp/ktlsp-bench-64-checkpoint` archive; binary checksums are in [result JSON](macos-arm64-0d54973.json). Product source was held at the checkpoint throughout. Later main features are outside this measurement.

Hardware: Apple M4, 10 logical CPUs, 16 GiB RAM; macOS 27.0.1 build 26A434, Darwin 27.0.0 arm64. Rust 1.99.0 (`b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`, LLVM 23.1.1), default Cargo release, no recorded RUSTFLAGS/target/profile overrides. Caches were warm; power mode and thermal sensors were not recorded. The [harness README](../README.md) defines timing, warmup, batching, lifetime RSS and reproduction commands.

All correctness/controlled-failure checks passed before timing. Each of the eight workloads has 101 finite positive nanosecond latency samples. Median/p95/p99 use nearest rank. RSS is `wait4` whole-child lifetime peak residency in bytes on verified Darwin arm64, excluding the parent harness. Each worker and the steady server contributes **one** lifetime RSS observation; startup contributes **101** observations, one per fresh server. RSS includes setup, warmup, measured repetitions and exit/shutdown, so it is not memory attributable to an individual timed operation.

| Workload | Bytes/cases | Median µs | p95 µs | p99 µs | Maximum lifetime RSS MiB |
| --- | --- | ---: | ---: | ---: | ---: |
| Lexer small | 256 / 1 | 3.051 | 3.098 | 3.259 | 2.219 |
| Lexer large | 1,146,880 / 1 | 6,842.917 | 7,140.000 | 7,203.333 | 493.203 |
| Lexer short tokens | 262,144 / 1 | 11,738.167 | 12,237.750 | 13,815.667 | 396.359 |
| Lexer malformed corpus pass | 66 / 10 | 1.156 | 1.191 | 1.452 | 2.203 |
| Document core small | 192 / 1 | 0.125 | 0.167 | 0.167 | 2.078 |
| Document core large | 786,432 / 1 | 229.417 | 456.458 | 465.417 | 5.094 |
| Fresh server initialize | protocol lifecycle | 1,087.084 | 1,255.834 | 1,405.750 | 1.969 |
| Framed dispatch roundtrip | unknown request/response | 12.667 | 17.375 | 19.625 | 2.094 |

The small and malformed lexer timings are averages over batches of 32 corpus passes. Other latency samples time single operations/passes. Ten warmups precede steady samples; startup has none. Document edits cycle beginning/middle/end. The large lexer worker's 493 MiB and short-token worker's 396 MiB peaks cover 111 full corpus passes (warmup plus measurement) and allocator/OS residency; separate instrumentation is needed to attribute that memory. These observations establish no efficiency budget or regression threshold.

[Window observations](macos-arm64-0d54973.window.json) contain process names/counts only, sampled about every 250 ms before/through/after the 2.392-second run. No competing Cargo/rustc/known test/probe process was detected; only the measured harness name occurred; the shorter server lifetimes fell between samples. Ambient macOS `replayd` is recorded separately. Load averages before and after were approximately 2.02 / 2.55 / 2.90. The observer's `ps` sampling and ordinary OS/Codex activity remain interference; brief activity between samples can be missed. These are observations of a coordinated window, not an isolated-hardware guarantee.

The first preflight stopped before measurement because its broad name match included ambient `replayd`; the actual run used explicit known build/test prefixes. Earlier five-repetition runs were contested smoke checks and are not this baseline. Post-collection provenance fields were added to the result metadata; raw samples and computed summaries were unchanged.

Result SHA-256: `291a83aeeefd65b0d39b3a8d8fd520b0ea85e6ed6ec72683e67c3cfc50dba93e`.

No official comparison, retained/per-operation allocation measurement, other-platform RSS verification, semantic/indexing workload, statistical confidence interval or performance gate. Parent T044 remains open; this is preparation child #64 only.

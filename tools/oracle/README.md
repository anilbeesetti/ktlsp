# Committed-observation replay (T045a / #59)

This optional standalone Rust workspace replays decoded official JSON observations against a supplied stdio executable. It is outside the root Cargo workspace, native default tests, CI and shipped server. It reuses `ktlsp-protocol` and the repository's existing pinned serde versions; it downloads/runs no official distribution, Java, Gradle or Maven.

From repository root:

```sh
cargo build --locked -p ktlsp-server --bin ktlsp
cargo build --locked --manifest-path tools/oracle/Cargo.toml --bin ktlsp-oracle-replay
tools/oracle/target/debug/ktlsp-oracle-replay docs/research/oracle/lifecycle-folding "$(git rev-parse HEAD)" -- ./target/debug/ktlsp > /tmp/comparison.json
```

Exit 0 means exact comparison match; 1 means recorded differences; 2 means invalid evidence, process/I/O failure or unsupported interaction. The revision argument is caller-declared, not inferred from the executable. Preserve executable/input hashes alongside published reports (see comparisons README). Commands execute directly without a shell; use only the intended executable. The child inherits the caller's environment/current directory.

Fixture directory requires `transcript.json` directional `{direction,message}` records and `metadata.json` with release, archive SHA256 and recorded integer returncode. Original fixture files remain unchanged. Both committed observation directories are supported. Client requests must be sequentially answered in the evidence; interleaved client messages, duplicate/unknown IDs, server requests, malformed JSON-RPC and absent exit evidence are rejected. This runner cannot reconstruct omitted exchanges or interact with dynamic server requests.

No normalization is applied: IDs, original URIs, error codes/severities, ranges, edits, completion ranking and array order remain intact. JSON object key order is irrelevant. The original URIs work for native in-memory `didOpen`; this does not create the original Linux workspace on macOS. Responses include exact expected/actual JSON and JSON Pointer differences, distinguishing absent fields from null. Notifications compare the entire captured sequence separately; logs include machine-specific values and differences can be environmental. Recorded exit status is compared separately. Missing/additional notifications do not prove incompatibility outside the incomplete original capture; shutdown-tail completeness is explicitly unverified.

Defaults: 5 seconds per write/request/exit phase, 30 seconds per session; 8 MiB each input file and aggregate framed stdout, 1,024 fixture/stdout messages, 64 KiB stderr. Existing framing limits also apply. Concurrent I/O workers keep blocked stdin and partial stdout reads under the controller deadline. Every return path kills/reaps the direct child; no unbounded I/O thread join is attempted. `shortcut: cleanup covers the direct child only; add platform process-tree supervision before using launchers that spawn descendants.` Descendants can survive and retain pipe-worker threads until they close inherited handles; invoke the actual native server executable. This is a research tool, not a sandbox for arbitrary executables.

Optional tool checks (independent from default native checks):

```sh
cargo fmt --manifest-path tools/oracle/Cargo.toml -- --check
cargo clippy --locked --manifest-path tools/oracle/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path tools/oracle/Cargo.toml
```

`replay-test-peer` is a Rust-only subprocess fixture for these tool tests. No compiler/oracle is invoked. The first baseline publishes genuine initialize/folding/notification differences and matching shutdown/exit. T045 remains open for live isolated official capture, richer corpus, dynamic interactions, portable project materialization, semantic/performance parity and process-tree supervision.

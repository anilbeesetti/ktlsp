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

## Synthetic-only raw recorder (T045c / #91)

`ktlsp-oracle-record` adds one sequential scripted session with raw evidence. It is separate from replay; replay retains its documented direct-child cleanup limitation. No dependencies, lockfile, native workspace membership, shipped capability or default CI graph are changed. The optional `oracle-tools.yml` workflow actually tests this standalone workspace on Linux and macOS using Rust peers only.

```sh
cargo build --locked --manifest-path tools/oracle/Cargo.toml --bins
tools/oracle/target/debug/ktlsp-oracle-record --synthetic-only \
  tools/oracle/fixtures/synthetic-recorder/script.json /tmp/FRESH-capture-directory -- \
  "$PWD/tools/oracle/target/debug/record-test-peer" fragmented
```

The output directory must not exist (parent must exist); collisions fail without overwriting. Exit 0 means complete capture, including a nonzero child exit; 2 means incomplete capture or invalid input. A response need not match any expected value. Script input must be a regular file (symlinks, directories, devices and FIFOs are rejected; Unix opens are nonblocking and opened metadata is rechecked). Script is a bounded JSON array of client methods with unique integer/string request IDs and a final `exit` notification. Requests wait for their exact response before sending the next message. Notifications are retained; unexpected server requests, malformed JSON-RPC, unknown/duplicate response IDs, invalid frames and incomplete EOF cause controlled failure. There is no dynamic request handler or semantic normalization.

`client-intended.raw` is persisted before each write. Manifest `clientFrames` contains exact half-open persisted raw offsets, `persistedBytes`, original `intendedLength`, and `acceptedBytes` for the prefix acknowledged by the pipe. A partial evidence write maps its persisted prefix and sends none of that frame. This proves bytes accepted by the OS, not peer consumption. `stdout.raw` and `stderr.raw` are written before interpretation; decoded `stdoutFrames` refer to half-open byte offsets in stdout.raw. `undecodedStart..undecodedEnd` locates the retained malformed/partial tail; interpretation stops on the first failure and cleanup may retain additional raw tail. On storage failure, stream `bytes`/tail offsets describe the persisted prefix; `observedBytes`, `unpersistedBytes` and `storageFailed` explicitly record any read bytes that could not be saved. These raw files are authoritative; the manifest never substitutes lossy UTF-8 for bytes.

Defaults: 5-second phase, 30-second session, 1-second cleanup; at most 8 MiB intended stdin and stdout each, 64 KiB stderr, 1,024 script/decoded messages. Library limits can lower these (deadlines have validated hard maxima). Reads/writes have an 8 KiB quota per 2 ms tick, decoding at most 32 frames per tick. A capped stream preserves one extra observed byte and marks truncation, then stops reading; EOF may remain unconfirmed. Remaining peer bytes cannot be captured beyond the explicit cap. No unbounded thread/join/pipe wait occurs. Deadlines assume responsive local filesystem and process-spawn operations; they are not hard real-time filesystem guarantees. Fresh directories are private, environment is cleared, and HOME/config/cache/state/tmp plus cwd are fresh capture-local roots. This is isolation from accidental user state, not a security sandbox.

The initial incomplete manifest exists before launch. Final manifest records script completion, limits/failure, raw byte counts/truncation, direct-child exit code/signal, stdout/stderr EOF, pre-cleanup facts, process-group termination and any cleanup error separately. Spawn/setup/storage failures retain incomplete evidence; a storage failure cannot claim completeness. Interrupted captures can leave only the initial manifest, with acknowledged write counts unknown. Files are flushed to the OS, not fsynced against power loss.

Supervision is explicitly Linux/macOS-only (other platforms fail before creating output or launching). Each child starts its own process group. All return paths attempt SIGKILL on that group, including after direct-child exit; normal completion also kills any same-group descendant that closed its pipes. Cleanup drains within its own deadline and observes/reaps the direct child without blocking. Descendants that escape into another group/session are outside containment, and grandchildren are reaped by their OS parent/init. Tests independently observe a descendant heartbeat stopping and pipe EOF after group termination. No claim of a general process sandbox or Windows support is made.

**Official live execution remains blocked.** This child ran synthetic Rust peers only. A later separately admitted gate must authenticate publisher archive checksum/provenance, validate launcher-specific containment (including escaped groups), review isolation/arguments and admit actual official capture. An existing local official installation does not authorize this recorder to launch it. T045 remains open; no new official diagnostic or feature-parity evidence is produced here.

## PR174 input validation (source only)

`recorder::validate_pr174_inputs(identity_path, preflight_path, fixture_directory)`
checks bounded proposed-run and preflight data for the two unchanged
[PR174 fixtures](fixtures/official-pr174-symbols/README.md). Consistent inputs
return `Ok(())`; invalid inputs return an error. It creates no evidence and
spawns no process. This success is data validation, never launch authorization.
No official CLI or recording path is delivered.

Both JSON inputs are regular, nonsymlink files limited to 16 KiB; source/script
files must exactly match embedded bytes, including UTF-8/newlines. The proposed
identity schema pins the preparation contract and source baseline, repository,
workflow-dispatch attempt 1 and finite original-run identifiers. Preflight must
repeat that identity and fixed release/hash claims, command, limits and output
path. Test source provides deliberately fabricated schema examples, never run
evidence. Unknown or duplicate top-level, identity, pin and limit fields are refused.

This checks data consistency only. Matching supplied hashes do not authenticate
the publisher, actual executable bytes, run, permissions or authorization. The
trusted host-supervisor handoff, effective isolation, runtime attestation and
actual teardown remain unimplemented. A live recording entrypoint requires
separate source/execution admission and independent review. Existing `record()`
and `--synthetic-only` retain their scope, environment, limits and raw/error/
cleanup behavior. No dependency, workflow, product or native default-test graph
changes accompany this source preparation.

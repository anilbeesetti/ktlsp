# T005 server foundation interface clarification

Own `crates/ktlsp-server` (library and ktlsp binary); crate dependencies ktlsp-text, ktlsp-protocol, serde, serde_json. Root authorizes its Cargo.lock package/dependency update; no dependency addition overlaps lexer except preserving its existing locked addition after integration.

Expose testable `Session::new()`/Default; `Session::handle(serde_json::Value) -> Vec<serde_json::Value>` for completed requests/notifications; `Session::document(&str) -> Option<&ktlsp_text::Document>` read-only; `Session::exit_code() -> Option<i32>`. A `run` helper owns framing/JSON decode, writes responses and stderr controlled errors and returns exit status. Keep state machine explicit uninitialized -> initializing -> running -> shutdown -> exit. `initialized` notification activates running; initialize response advertises UTF-16 incremental sync/openClose/save false, serverInfo ktlsp 0.1.0, plus only implemented providers. The server always selects UTF-16: [LSP 3.17](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#clientCapabilities) makes UTF-16 mandatory even if omitted from `general.positionEncodings`. An absent or empty list, UTF-8/UTF-32-only preferences and unknown string alternatives therefore all retain the UTF-16 fallback; a non-array or any non-string entry returns deterministic invalid params without advancing initialization.

Accept string/integer IDs (including zero and extreme integers without coercion); reject floats/objects/null IDs and method nonstrings or missing jsonrpc 2.0 using Invalid Request id null. JSON-RPC responses from client ignored when valid (no server requests in first wave). Batches invalid in LSP. Extra JSON fields accepted; method-specific params validated. initialize params processId null/int optional; rootUri/workspaceFolders are stored configuration only, not successful project import. initialize twice invalid request; requests before init -32002, unknown method -32601, invalid params -32602. shutdown result null; after shutdown reject requests -32600, ignore changes. Exit succeeds only after shutdown, fails before shutdown. Clean EOF after shutdown succeeds; before shutdown fails. Bare JSON parse error -32700 id null is recoverable. Framing errors fatal exit1. Protocol stdout only.

Document store limits: 128 open documents, 64 MiB aggregate UTF-8. didOpen must have Kotlin document, URI string, version i32, text string. Accept syntactically valid absolute URI of any scheme (untitled editors must work); no local filesystem read on didOpen. Document URI identity is exact incoming URI; more normalization follows a dedicated URI policy. Duplicate didOpen rejected without mutation. didChange validates existing document, increasing version, complete TextChange schema and atomic application. didClose removes and is harmless if unknown. didSave accepts known doc and optional text but cannot change versioned document text without didChange. Invalid notifications produce a controlled stderr/diagnostic record, never JSON-RPC error responses; expose errors via session drain method for run to log. Unknown notifications ignored; cancelRequest acknowledged/ignored only because no asynchronous work exists, not advertised as full query cancellation. Do not claim project/file-watch/cancellation parity.

Test Rust subprocess framed exchanges with non-BMP changes and every lifecycle error, fragmented frames, malformed JSON recovery, hostile JSON shapes/generated frames, duplicate/stale open/change, aggregate limits, abrupt/truncated EOF and stdout cleanliness. External crate callers cannot mutate Document invariants.

## Concrete v1 validation policy

`initialize` requires an object `capabilities`; `initialized` requires object params. Shutdown/exit accept absent, null, or object params. Unknown fields in envelopes and parameter objects are accepted. Optional change `range`/`rangeLength` fields, if present, cannot be null. Valid client responses may use a null ID for protocol errors; null request IDs are rejected. No mutable session document accessor is exposed.

Absolute URI validation uses RFC 3986 generic ASCII syntax (including percent encoding, authority and IPv6/IPvFuture literals), accepts all schemes and preserves the exact spelling. Raw IRIs/non-ASCII paths must be percent encoded. It does not interpret scheme-specific filesystem semantics, normalize URI spelling, resolve filesystem paths, read files or import projects.

Additional bounds: 8 KiB per URI/workspace-folder name, 128 workspace folders, 128 retained notification-error records (then an explicit suppression record). Legacy protocol/lexical errors do not quote document content, URI strings or method strings. Explicit JSON import diagnostics can carry bounded original native/model reasons; wire serialization escapes them and initial notification records JSON-quote untrusted terminal text. Framing limits apply before JSON decode. The finite #421 authority below does not establish full progress/project/file-watch providers, semantics, persistence or official differential verification.

Run the native binary as `cargo run --locked -p ktlsp-server --bin ktlsp`; with no arguments or explicit `--stdio` it uses framed LSP on stdio. `--help` and `--version` print CLI output. Other arguments, including combinations of launch flags, fail with exit status 2 and stderr only. Core operation does not execute or require Java or project build tools. Optional tooling policy is defined in the current repository parity specification.

## Lexical pull diagnostics

Clients advertising an object `textDocument.diagnostic` negotiate `diagnosticProvider` with `interFileDependencies:false` and `workspaceDiagnostics:false`. Open/change/close then emit no push diagnostics. Clients without that capability retain push diagnostics and receive method-not-found for `textDocument/diagnostic`.

Pull requests read the current open document through the existing bounded lexical/structural cache and return `{kind:"full",items:[...]}`. No result IDs or unchanged reports are produced. Invalid request shapes and missing/closed documents return invalid-params without mutation; analysis-limit diagnostics remain visible. UTF-16 ranges, atomic edits and close/reopen invalidation are unchanged. Compiler/inspection diagnostics, related documents, workspace diagnostics, refresh/progress and asynchronous pull-diagnostic cancellation are not provided.

## Deferred parser consumers and native input

`Session::handle(Value) -> Vec<Value>` remains the sole protocol validator and lifecycle/document owner. Accepted parser requests and explicit native imports defer responses; service input and `poll()` until completion. `pending_requests()` counts parser/import jobs, bounded initial-symbol waiters and cancelled generations awaiting actual completion. Terminal `close() -> (Vec<Value>, bool)` returns remaining cancellation responses and requires joined owned threads plus original request-specific Parser closure proof. Drop cancels without reporting success.

At most eight retained parser/import/waiting requests per Session and eight owned parser/import thread handles process-wide are admitted; at most one retained native import exists per Session. Initial unopened-symbol waiters share, not extend, capacity. The bounded global registry retains unfinished handles after drop and joins only finished handles before releasing slots. Parser jobs still enter the existing validated-CST provider; native import jobs perform only bounded read-only filesystem/model discovery. Exhaustion is `-32803`, not an empty outline. No second parser pool or supervision/deadline relaxation is added; cleanup grace excludes OS calls.

The provider uses `Parser::parse_snapshot_with_cleanup -> (Result<SnapshotResult, ParserError>, bool)` through the same existing parse/supervise/admission/reaper pipeline. True proves no child ownership/spawn attempt, or same-child reaping before return and non-Unix scoped-I/O exit. False is unproved closure, not an assertion that a child remains alive. A false report or thread panic permanently latches the Session's terminal cleanup failure; later/global state cannot turn it into proof, and original Cancelled/Timeout errors remain unchanged. Limits, deadlines and the Unix 20 ms observation grace are unchanged. This conservative status-1 limit is intentional; successful symbol-thread joins alone do not prove graceful cleanup.

Symbols, finite definition/hover, same-body references/highlights, eager local completion, manual source-owned signature help and guarded local-val prepare/rename use that one pipeline. Parser `Cancelled` remains RequestCancelled (`-32800`); other parser variants remain RequestFailed (`-32803`) with a distinct static category for input/nesting/output/allocation limits, coverage/tree validation, deadline, worker failure, I/O-or-cleanup and snapshot mismatch. This preserves selected-result, cancellation/freshness and cleanup precedence. Category text does not diagnose an OS cause or repair the original failures in #377; no error code, deadline or resource ceiling is relaxed. Negotiated lexical semantic-token transport is synchronous and parser-free; it does not establish identifier semantics.

An active request's exact string/integer ID receives exactly one normal/error response. Valid `$/cancelRequest` signals the same AtomicBool used by Parser and completes that ID with RequestCancelled (`-32800`); unknown IDs are ignored, invalid notifications only record bounded errors. A completed/cancelled ID can be reused without confusing old work with its new generation. Duplicate active IDs return InvalidRequest with null ID and do not cancel the original request. Accepted didChange/didClose invalidate matching requests with ContentModified (`-32801`). Publication independently checks the open-document incarnation, version and exact content; identical close/reopen cannot publish old ranges. Rejected notifications do not invalidate committed state.

The existing `run(&mut BufRead, &mut Write, &mut Write) -> i32` now drives the shared resumable `FrameReader` and deferred Session completions. A supplied library reader must yield WouldBlock periodically while idle; an arbitrary blocking reader delays completions. The CLI uses an owned duplicated stdin handle with Unix poll or Windows redirected-pipe PeekNamedPipe, with 10 ms idle readiness ticks, no reader thread and no frame queue. The globally buffered std::io::Stdin is not read after readiness polling. Partial/back-to-back framing uses the same canonical grammar as blocking `read_frame`; fatal framing remains fatal.

Shutdown/exit/EOF/fatal input/output errors cancel owned work. Clean shutdown/exit or shutdown/EOF returns success only after confirmed job cleanup. Broken output is never retried, and undeliverable responses are not claimed delivered. Incomplete cleanup is controlled failure; no unfinished thread is joined. Output writes/flushes, OS scheduling, handle/file operations, and OS spawn/cleanup calls can block and are not hard real-time guarantees. See the server README for actual ownership and resource limits.


## Explicit JSON source authority (#421)

Capture-bearing composition passed original full Linux/macOS/Windows CI and
independent actual-value/edit gates. Capture-free source then passed its OWN
reviews/full three-host CI, guarded PR422 integration and both original
exact-main gates. [Exact evidence and limits](../status/development.md#verified422-capture-free-integration-and-original-exact-main-acceptance)
remain narrower than official project equivalence. Explicit
`initialize.initializationOptions.projects` entries `{type:"json",path:fileURI}`
load actual `workspace.json` through unchanged immutable v2 import and a separate
native source-inventory layer. An immediate unopened `documentSymbol` can await
that same initial attempt without readiness probes. `intellij/reloadWorkspace`
requires `initializationOptions` and returns `null` only after complete current
commit; empty projects clear authority, failure leaves prior data historical.

Known membership, original model/source byte/native-identity checks before and
after successful parsing, whole-model generation, overlay epoch and exact
open-document incarnation/version/content guard publication. Native URI aliases
map one member/overlay for symbol queries; synchronization keeps its existing
exact wire identity. Unknown file overlays block closed publication without
outside-path probes. Accepted synchronization/reload/folder changes stale
affected work; rejected edits and whole folder batches preserve prior authority.
At most two whole retained generations include candidate/historical captures.
Cancelled work stays owned until join and receives no late success.

Generic URI validation, T004 and original JSON/JPS/Maven data producers remain
data-only. The explicit I/O layer has stricter trusted-quiescent-local filesystem,
physical ownership and aggregate bounds documented in the import/server READMEs.
No watcher, status/FINISHED signal, effective classpath, build-tool orchestration,
workspace-symbol or official project/URI/semantic equivalence is claimed.


## Manual signature-help boundary (#424)

Only the trusted parser-provider branch advertises `signatureHelpProvider:{}`.
The manual completed named-call envelope reuses the original419 applicability
proof and current OPEN document, not imported421 aliases/waiters. Signature and
sole-formal labels are original source strings, active indices zero; no trigger
or optional label-offset negotiation/state is added. Context shape is validated
but prior client help is not source authority. Strict UTF16 and existing global8
owned work/cancellation/join/incarnation/version/content publication remain.
One semantic allowance continues through escaped1MiB response/ID/framing
preflight and source copies; separately bounded lowering/region stages and
ordinary fixed-count JSON/final serializer allocations do not establish a
whole-pipeline/RSS/prompt semantic-cancellation guarantee. [Exact provider and
current acceptance limits](../../crates/ktlsp-server/README.md#manual-source-owned-signature-help-424)
do not establish incomplete typing, general argument/overload/inference/imported
semantics or official signature-help equivalence. Static component clears never
substitute for exact ROOT union/compiler/original hosted actual-value/edit gates.


## Coherent eager native workspace-symbol source contract (#426)

The complete composed source now adds an independent `workspace/symbol` path;
the narrower421 operation above did not itself grant this capability.
Capture-bearing4b3429c passed full original three-host CI and independent
BOTH owner/ROOT complete actual-value/genuine-edit acceptance before retirement.
The cleaned union passed its OWN source/compiler/original three-host CI, guarded
PR429 integration at main21aab03 and ALL THREE feature OWNERS+ROOT original
exact-main acceptance before the exclusive original-base lease cleared.
Only the trusted sibling-parser provider advertises `workspaceSymbolProvider:true`;
the client's `workspace.symbol` kind profile is independent of document symbols.
Results are eager full declaration locations, without resolve/data/pagination/
progress. Empty query selects all; otherwise native matching is case-sensitive
LITERAL substring on existing outline spelling, not official fuzzy matching.

The operation captures every unique admitted.kt member under one immutable
workspace Arc/epoch and exact mapped-overlay incarnation/version/content. Both
verification phases visit every original model and every captured closed source,
including zero/all-overlay models, empty/negative and nonmatching files. Manual
reload admits additions; changed/deleted records refuse. Whole-success publication
requires the same authority after joined work; no valid prefix escapes a late
failure. The original two-generation/trusted-quiescent-FS limitation remains.

One existing globally owned8-slot job parses sequentially and retains cancelled
ownership until join. No second pool, production probe/barrier/delay or raised
parser bound/deadline is added. One256 Mi-unit logical allowance includes
verification, capture, delegated parser precharge, projection and output;
conservative precharge permits AT MOST42 tiny-source delegations before other
work, not1,024 successful parses merely because membership admits1,024.
Actual escaped IDs reserve512 failure bytes before parameter errors. IDs fitting
only the compact refusal keep their echoed ID; an ID that cannot fit EVEN that
response is rejected by provider policy as InvalidRequest (`-32600`, `id:null`)
before params/source work. The connection remains available; no false full-ID
echo or oversized transport write is attempted. Logical limits do not prove
CPU/RSS/OS latency or prompt inner cancellation.
[Full source/error/resource and evidence-layer contract](../../crates/ktlsp-server/README.md#coherent-eager-native-workspace-symbols-426).

Packaged raw protocol values, separate Session ownership, actual lossless byte
runs for giant IDs and genuinely applied returned edits received independent
BOTH owner/ROOT complete original hosted readbacks BEFORE separate observation
retirement admission. Complete old captures remain outside the repository;
source-only aggregate/hidden internal states have no invented producer telemetry.
The permanent real requests, producer calls, applied edits, assertions and owned
cleanup survived retirement and passed the cleaned union's OWN new gates.
No local Kotlin/parser/server/Session/worker runtime is admitted under96.
No semantic index, effective library/classpath, official matcher or full-parity
claim is added.

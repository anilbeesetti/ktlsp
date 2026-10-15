# T007 / #62 parser adoption safety gate

This library invokes the separately built `ktlsp-parser-worker` through an explicit
trusted absolute path. Build both with `cargo build --locked -p ktlsp-parser`.
`Parser::parse` accepts source, source/script kind, and a cancellation flag; it
returns a checked flat CST with original byte spans and syntax diagnostics, or a
named rejection. Source is never replaced with parser-generated text. This crate
has no server caller and advertises no provider. It is not full grammar, recovery,
HIR, formatter or official Kotlin LSP parity.

Each call starts one Rust worker. On Unix, the parent transfers input and drains
stdout through one nonblocking `UnixStream` endpoint; the child's duplicated
stdin/stdout endpoint stays blocking. Input uses byte offsets and explicit write
half-close for EOF, including best-effort half-close after a failed write. Each
iteration attempts at most one 8 KiB read and one 8 KiB write, one pending
half-close and one child-status poll. Cancellation precedes the original 500 ms
deadline. Productive iterations continue; idle iterations sleep 2 ms. No Unix
I/O threads or scoped joins remain. Non-Unix retains the pipe/scoped-thread path.

The Unix pump samples reader errors before child status; allocation exit86 and
other failed exits precede a remembered writer error. After successful child
exit, writer errors precede later reader errors, with cancellation/deadline
checks before accepting either completion. This preserves these priorities,
not asynchronous thread-arrival timing. Post-success deadline selection sends
no grammar observation, as before. Observer validation still follows cleanup.

Every ordinary post-spawn return checks synchronous cleanup of the same owned
child. Confirmed cleanup preserves the selected result; unconfirmed cleanup
returns ambiguous `Io`, overriding the original error. Drop retains best-effort
kill/wait for unconfirmed state and unwinding; abort can skip it. Blocking OS
spawn/kill/wait and scheduling have no hard wall-time bound. Conditional Unix
progress assumes those operations return and requested termination/reaping
completes; failed OS cleanup containment remains a gap. This removes Unix I/O
joins without establishing a hard caller-return bound. Non-Unix joins also
remain unbounded. The existing three-second fault-test elapsed assertion runs
after return and cannot interrupt a hung call.

Worker stderr is discarded except for explicit bounded observations; upstream
panics may contain source, and no document is logged. The trusted shipped worker
never spawns descendants. Direct-child reaping does not establish descendant
termination. Callers must limit concurrent requests and install the matching
worker. This synchronous API has no detached task, reaper or global pool.
Editor consumers use `SourceSnapshot` and `Parser::parse_snapshot`; accepted
`SnapshotResult::tree_for` rejects a changed document/revision/source/file kind.
See [the exact declaration corpus and unresolved recovery gates](CORPUS.md).
Linux native checks do not establish Darwin/Windows reliability, resolve #96 or
close B3/B4; historical MissingACK observations stay inconclusive. No comparative
speed/RAM claim follows from the transport change. Collected-byte limits do not
bound parent allocation capacity, stack or RSS.

Limits are enforced independently in parent and worker where applicable:

| Resource | Limit |
| --- | --- |
| Original and normalized input | 64 KiB UTF-8 |
| Worker exchange deadline | 500 ms, including spawn/input/output |
| Raw conservative delimiter preflight | 128 open `([{<` after saturating closes |
| CST depth / flat elements | 128 / 16,384 |
| Syntax diagnostics / each message | 256 / 4 KiB |
| Wire response | 4 MiB (collection reads at most limit + 1 byte) |
| Worker allocation requests | 64 MiB cumulative, including zeroed/realloc |

Deadline means termination is requested when the polling supervisor next runs;
OS scheduling, process spawn and termination/reaping are not real-time guarantees.
No code can promise a fixed cleanup duration when the OS stalls. The allocator
conservatively charges all successful and failed requests without refund, including
new realloc size; exhausting it aborts only the worker. This caps live requested
Rust heap and allocation churn, **not OS RSS, allocator overhead or stack**.
Recursion/stack overflow and unsafe upstream crashes are isolated in the worker.
There is no security sandbox against an actively malicious worker executable or
exploited upstream unsafe code. The worker path must be trusted.

Preflight counts delimiters even inside strings/comments and treats `<` as an
opener. It can reject valid Kotlin and does not model parser recursion; process
supervision remains mandatory. Limits are intentionally conservative for this
adoption gate, smaller than the existing lexer/document limits. A resource or
coverage rejection yields no partial tree. The caller retains its source on any
error. Error recovery is ktrs behavior, not a compiler diagnostic claim.

CRLF and lone CR normalize to LF only inside parsing. A boundary map converts all
returned spans to original source; a normalized LF covers the entire original
CRLF. `Parsed::position` delegates UTF-16/invalid-boundary handling to ktlsp-text.
Worker checks original normalized tree spelling and missed-token reports; parent
checks rooted preorder/containment/depth, UTF-8 spans, exact token partition,
diagnostic/count limits, and bounded JSON. The token-loss regression is rejected
explicitly, never presented as a lossless tree.

## Adoption audit

The direct parser dependency is the audited local fork of ktrs-parser `=0.5.1`
under `vendor/ktrs-parser-0.5.1`; ktrs-syntax `=0.5.1` remains a registry dependency.
Locked transitive packages remain ktrs-lexer 0.5.1 and text-size 1.1.1. `cargo tree --locked
-p ktlsp-parser -e normal,build` contains these plus existing Rust serde/text
packages; all four new crate manifests have no build script or normal/build
native/JVM dependencies. Upstream dev dependencies (mimalloc/expect-test), facade
(stacker), CLI and Java integration are excluded. No upstream tests are imported
as runnable dependencies. The root workspace excludes the standalone vendor workspace; production parser
regressions run in this crate. No upstream dev dependencies enter native tests.
The parser's original archive checksum and all 54 source hashes remain in vendor
provenance; recovery.patch followed by formatting.patch reproduces final source
bytes, recorded in formatted-sha256.json. Formatting has no opt-out. Other registry SHA-256 values
match Cargo.lock:

| Archive | SHA-256 |
| --- | --- |
| ktrs-parser 0.5.1 | ec8d43d9549490689477385c7678bbd22ecbc445973ab364e8ecb69dd4882bcc |
| ktrs-lexer 0.5.1 | e085425e4b01ec2d56f4159051bf9152f149b5501ce13f7d05eb3a895c721f58 |
| ktrs-syntax 0.5.1 | 4b29166c66ba5e9edb079497726eae48b28a5dc405b5e189d54ff96ddbd38e4f |
| text-size 1.1.1 | f18aa187839b2bdb1ad2fa35ead8c4c2976b64e4363c386d45ac0f7ee85c9233 |

Upstream ktrs-syntax TreeBuilder uses unsafe vector writes after reserving capacity.
Reviewed builder reserve/push paths; this is not a proof against every unsafe bug.
The production call stays in the child. Our unsafe code only forwards allocator
calls unchanged to Rust System after allocation-free atomic checked accounting.
Allocator denial and arithmetic overflow have tests; independent adversarial
review is required.

Published ktrs archives omit notices. `notices/` retains the full NOTICE and both
licenses from inspected upstream revision
`a703f88d6d4ac9d983a17df66ddf6ac403546c00`, plus text-size licenses. This deliberately
retains the complete upstream NOTICE even though formatter/plugin code is not
included. Preserve these files in later packaging. Regression input intent comes
from the upstream backtracking_memory/missed_tokens tests documented in completed
research; the small harness and additional fixtures here are independently written.

## Remaining acceptance work

T007b/#83 remains pending. Delivered child #86 adds immutable snapshot/result validation and
an exact named declaration corpus. Delivered child #87 supplies a source-local parameter-list
boundary repair and production snapshot recovery controls; generic/default/type/
annotation recovery gaps remain explicit. It does not unlock T008a or establish the full #83
recovery/resource contract. See CORPUS.md for each tested outcome and gap.
Delivered PR102/#100 adds bounded classifier-generic sibling recovery. C17/#104
records three complete known-defect inputs/outcomes for missing defaults, malformed
parenthesized types and annotation arguments in `tests/recovery_residuals.rs`.
See CORPUS.md for the finite ledger and reuse of existing excluded headers/bounds.
Passing a defect assertion is not required recovery. C12/#103 is delivered in
[PR106](https://github.com/anilbeesetti/ktlsp/pull/106), merge
`0a1a863744a2bc520dac46792a5ce1eebe5b70dc`: exact accepted-control declarations/names/
parents/ranges/diagnostics, original snapshot property and member recovery, nested
anonymous annotation ownership and repeated-recovery healthy name.
[Exact merged-main Linux CI](https://github.com/anilbeesetti/ktlsp/actions/runs/38332673276/job/115085727770)
passed all three pinned commands (226 active tests, two exercised helpers and
19 separate protocol fault cases). Generic threshold/leading-trivia/aggregate/
scratch controls remain resource-only. C12 adds assertions, not grammar repairs. The final refreshed
[#83 audit](https://github.com/anilbeesetti/ktlsp/issues/83#issuecomment-6130232640)
keeps #83/#7 open and T008/T008a blocked on remaining recovery/assertion/resource gaps.

T008a declaration HIR child follows the accepted result/recovery gate: lower named
declarations/package/imports to stable revisioned HIR; preserve parents, names,
original spans and incomplete-declaration diagnostics, with explicit unsupported
grammar. Byte offsets alone are not symbol identity. No resolution/provider claim
without separately accepted semantics.

## Delivered #100 classifier-generic recovery

PR102 recovers simple named sibling function headers after incomplete
class/interface type-parameter lists at parameter entries or after completed
parameters/bounds. Optional complete annotations/modifiers stay with the function.
Newlines and keywords alone do not establish a recovery boundary. Generic and
extension function headers, other sibling kinds, objects, and incomplete nested
bounds/annotations remain outside this positive contract; ordinary parsing stays
in effect for them. #83/#7 and T008a require the remaining acceptance audit.

Scratch prefix parsing preserves live state and respects existing truncation;
256 copied raw tokens/4096 source bytes per attempt and 4096 copied raw tokens per class list
bound scratch parsing. Leading trivia inspection is outside the copied-token
charge and remains bounded by the input limit and advancing parser cursor.
Exceeding the scratch limits declines recovery without changing global budgets,
Coverage rejection, snapshot identity, strict result validation or the 500 ms
worker deadline. No server provider or semantic API changes. Exact independent
regressions are in declarations.rs/recovery_limits.rs. PR102/#100 was merged as
`ab00d55557cfea9bcee87aa41fe2922f82770027`, reviewed head
`d7da79ddeea95901327d774382a37740445a0ef7`; their tree equals final CI checkout
`78c55c4bfb5629704bfdb11203f869af2e24781c`.
[Exact merged-main Linux CI](https://github.com/anilbeesetti/ktlsp/actions/runs/38289885940/job/114947148613)
passed all three pinned commands (192 active tests, two exercised helper fixtures,
19 separate protocol fault cases), after renewed correctness/adversarial/architect
approvals and the coordinator's guarded merge grant. CORPUS.md retains identities
and evidence limits. This supersedes stale candidate/pending delivery prose in the
historical review/provenance files; it does not reinterpret those old runs.
C17 starts from main `4d27673f703fd3c7b0ee57fc7a3cf35c6c21f05f`, compatible with the
frozen parser source above. Its one final integration base is
`0a1a863744a2bc520dac46792a5ce1eebe5b70dc`; inherited C12 tests and all main product,
dependency, workflow and optional-tool files are preserved. C17 itself changes no
parser source/API/worker/dependency files.
Issue #96 remains OPEN/UNKNOWN; Linux passes do not establish Darwin correction.

The coordinator explicitly amended the accounting contract after a focused audit:
leading trivia and cheap kind/modifier/EOF prefilter misses may remain outside
the copied-token allowance. Every eligible scratch window is charged before
grammar evaluation, including failure; a zero-copy eligible rejection debits one.
The separate allocation-free leading scan is bounded by the 64 KiB input and
local comma-consuming loop progress. This is not a bound on every examination,
whole-parser complexity, RSS or total work. The production leading-trivia regression passed
final Linux CI. Unconditional window-predicate checks compile
against the actual byte/EOF guard. These do not establish an end-to-end active-EOF
recovery case; that gap remains for #83. Attributable production deadline/allocation/serialization-boundary evidence remains
incomplete under #83. C17 adds no resource-boundary claim or grammar repair.

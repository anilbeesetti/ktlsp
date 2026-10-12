# T007 / #62 parser adoption safety gate

This library invokes the separately built `ktlsp-parser-worker` through an explicit
trusted absolute path. Build both with `cargo build --locked -p ktlsp-parser`.
`Parser::parse` accepts source, source/script kind, and a cancellation flag; it
returns a checked flat CST with original byte spans and syntax diagnostics, or a
named rejection. Source is never replaced with parser-generated text. This crate
has no server caller and advertises no provider. It is not full grammar, recovery,
HIR, formatter or official Kotlin LSP parity.

Each call starts one Rust worker, concurrently transfers input and drains stdout,
checks cancellation/deadline every 2 ms, and terminates/reaps before scoped I/O
threads join on every exit path. Worker stderr is discarded (may contain source
on upstream panics); no document is logged. The shipped trusted worker never
spawns descendants. Callers must limit concurrent parse requests and install the matching worker.
Editor consumers use `SourceSnapshot` and `Parser::parse_snapshot`; accepted
`SnapshotResult::tree_for` rejects a changed document/revision/source/file kind.
See [the exact declaration corpus and unresolved recovery gates](CORPUS.md). This synchronous
API has no detached task or future whose drop could leave parsing running.

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
-p ktlsp-parser -e normal,build` must contain these plus existing Rust serde/text
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

## Remaining accepted child proposals

T007b/#83 remains pending. Child #86 adds immutable snapshot/result validation and
an exact named declaration corpus. Child #87 supplies a source-local parameter-list
boundary repair and production snapshot recovery controls; generic/default/type/
annotation recovery gaps remain explicit. It does not unlock T008a or establish the full #83
recovery/resource contract. See CORPUS.md for each tested outcome and gap.
The parent gate needs joint assessment of #86/#87 and remaining recovery/resource
gaps before T008a can proceed.

T008a declaration HIR child follows the accepted result/recovery gate: lower named
declarations/package/imports to stable revisioned HIR; preserve parents, names,
original spans and incomplete-declaration diagnostics, with explicit unsupported
grammar. Byte offsets alone are not symbol identity. No resolution/provider claim
without separately accepted semantics.

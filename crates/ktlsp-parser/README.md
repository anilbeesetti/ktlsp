# T007 / #62 parser adoption safety gate

This library invokes the separately built `ktlsp-parser-worker` through an explicit
trusted absolute path. Build both with `cargo build --locked -p ktlsp-parser`.
`Parser::parse` accepts source, source/script kind, and a cancellation flag; it
returns a checked flat CST with original byte spans and syntax diagnostics, or a
named rejection. Source is never replaced with parser-generated text. The server
consumes the validated CST for its bounded syntactic document-symbol provider
(see [server limits](../ktlsp-server/README.md)); the finite declaration consumers
below are library-only. This is not full grammar, recovery, HIR, formatter or
official Kotlin LSP parity.

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

On Unix, all Parser instances share eight reservations covering active children,
pending cleanup and permanent failure pins. Admission happens before normalization,
Command/observer allocation and terminal Once initialization. Once may block before
the original exchange clock; cancellation is rechecked after initialization.
A single retained cleanup reaper requests a 256 KiB stack and borrows each reserved
Child in static storage. The caller closes its transport before publishing cleanup
and observes it for 20 ms. The reaper requests termination and waits for that same
child; no caller-side kill/wait/join remains on Unix.

Confirmed same-child reaping preserves the selected result, even if the reaper
has died elsewhere. With valid pending cleanup at observation expiry, an already
selected error is preserved, but success becomes Io. Observed cleanup failure,
invalid ownership or reaper death before confirmation overrides the result with Io.
Optional deadline/copy receipts are read only after confirmed cleanup; otherwise
sockets are dropped without a receive and receipts are Missing (deadline selection
and send scalars remain recorded). A returned Timeout/Cancelled is not cleanup
proof; late cleanup never revises a returned result.

`Parser::parse_snapshot_with_cleanup(&SourceSnapshot, &AtomicBool)` returns the
unchanged normal parse result together with one request-specific boolean. True
means this invocation made no child ownership/spawn attempt, or its same child
was actually confirmed reaped before return (and non-Unix scoped I/O exited).
False means closure is unproved, not that a child is necessarily still alive.
The flag becomes false before the actual spawn attempt and only existing
same-child cleanup confirmation can set it true. It is not inferred from thread
completion, global counts or a later reaper state; no original result is revised.
The result-only `parse`/`parse_snapshot` APIs retain their existing semantics via
the same parse/supervise pipeline. No limits, deadlines, selected errors or Unix
20 ms observation grace are relaxed. The server consumes the reporting API and
permanently fails terminal cleanup status if any owned request reports false or
its symbol thread panics, even if reaping might complete later.

Caller buffers and observer endpoints remain inside the reservation until disposal.
One canonical output Vec reserves before append; exposed capacity above 8,388,610
bytes is dropped and rejected. Normalized input/map capacities are checked against
65,536 bytes/65,537 words. These are requested-storage bounds, not a whole-process
heap, allocator, stack or RSS bound. One blocked kill/wait stalls the sole reaper;
eight sequential unconfirmed or failed reservations can exhaust admission despite
functional fixture serialization. There is no replacement thread, queue, retry or
production reset. Once, OS spawn/kill/wait and scheduling have no hard wall-time
bound. The existing three-second fault-test assertion runs after return and cannot
interrupt a hung call. Non-Unix retains synchronous cleanup and unbounded
pipe/scoped-thread joins; an unjoined thread panic can prevent ordinary return.

Worker stderr is discarded except for explicit bounded observations; upstream
panics may contain source, and no document is logged. The trusted shipped worker
never spawns descendants. Direct-child reaping does not establish descendant
termination. Callers must install the matching trusted worker; this is not a sandbox.
Editor consumers use `SourceSnapshot` and `Parser::parse_snapshot`; accepted
`SnapshotResult::tree_for` rejects a changed document/revision/source/file kind.
See [the exact declaration corpus and unresolved recovery gates](CORPUS.md).
Linux native checks do not establish Darwin/Windows reliability, resolve #96 or
close B3/B4; historical MissingACK observations stay inconclusive. No comparative
speed/RAM claim follows from the transport change. The parent capacity checks do not
bound whole-process allocation, stack or RSS.

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

## Bounded semantic prerequisites (#320)

This branch composes PR307 → PR304 → PR299 → PR302 → PR305 and the two adopted
#308 lowering/regression files, including the reviewed function-EQ refusal.
It incorporates verified incoming main `2e408cfd8d65c68a9eede863e3f983a2db720d95`
without changing its product providers, native dependencies or workflow isolation.
This is finite syntax/structure admission, not full scopes, binding, navigation,
types, stable cross-revision symbols or a new server capability.

- `lowering::lower_declarations` borrows declarations and existing syntax errors
  from a validated source snapshot, preserving original byte/name spans, CST
  parents, result-local owners and incompleteness. Unsupported syntax rejects
  the whole operation, including malformed/error-bearing package/import headers.
  Root recovery errors between a nonempty preamble and the first declaration also
  refuse: the producer can recover malformed header tails outside the header node,
  so publishing only its valid prefix would silently lose malformed syntax.
  #346 adds borrowed `package()` / ordered `imports()` projections from the pinned
  producer's validated preamble shapes. Qualified names retain original CST index,
  raw spelling/range and ordered `segments()`; segment dots inside backticks stay
  inside that component. Supported semantic spellings reuse the declaration helper
  unchanged, without Unicode normalization; unsupported spellings reject atomically.
  Imports distinguish named vs `on_demand()` and expose a separate optional alias.
  Duplicate import records remain in source order; this is not compiler duplicate-
  import checking. Headers are neither declarations nor resolved symbols, and do
  not change declaration ordinals, structural regions or local-reference binding.
  Empty producer package/import nodes remain supported. File annotations, modifiers
  and other unsupported header shapes remain refused; NativeSourceFile admission
  is unchanged. Projection uses the existing shared 2,097,152-work-unit and
  131,072-byte requested/retained-capacity budgets, with at most 1,024 total nonempty
  package/import directives and 1,024 total name/alias components, checked before
  reservation/emission. Qualified components borrow CST/source with no component
  allocation or recursive traversal. Production-parser fixture tests cover LF/CRLF,
  UTF-16 positions, backtick dots, named/star/alias/duplicate imports and the actual
  authenticated f/x/y binding consumer. Hosted execution is required under #96;
  this source change alone is not hosted proof or package/import resolution.
- Reference-bearing functions require an ordinary top-level zero-parameter
  function with an ordered brace body of plain immutable untyped local `val`s,
  whose initializers are integer constants or whole bare-identifier reference
  subtrees. Parameters, receivers, nested/reference-bearing member contexts,
  calls, qualified accesses, assignments and relevant error-bearing contexts
  are not admitted. Accepting a reference initializer does not resolve its name,
  enforce declaration-order visibility or diagnose self/forward references.
- `Declaration::semantic_spelling` separately supports ASCII bare names and
  nonempty backtick bodies without backticks, CR/LF or NUL, borrowing unchanged
  bytes without normalization; unsupported names return `None`, not an identity.
- `structural_regions::build_regions` authenticates the exact original lowerer
  and snapshot result, then supplies file/class-body/function-body regions and
  ordered declaration memberships. Equal bytes from an independent parse do
  not establish origin. Regions and declaration owners are not Kotlin scopes.
- `declaration_name_index` groups exact **raw** spellings within an identical
  borrowed region, retaining duplicates and result-local declaration ordinals.
  `ancestor_candidates` returns all such groups along structural ancestors,
  nearest first, without picking a semantic winner or applying visibility,
  shadowing, declaration-order or backtick-equivalence rules.
- `position_regions::containing_regions` accepts original UTF-8 boundaries and
  returns every half-open containing region in directory order; brace bytes
  count, EOF matches nothing. It neither authenticates a reference occurrence
  nor converts UTF-16 positions; callers convert against the identical source.
- `lowering::reference_site` authenticates an original whole-initializer
  `REFERENCE_EXPRESSION` and its sole identifier token against the exact lowerer,
  parsed tree, result and current snapshot. Private fields retain borrowed origin,
  original range/spelling, containing property, body CST index and function.
  `ReferenceSite::semantic_spelling` reuses the declaration spelling helper.
- `local_value_binding::bind_local_value` validates the complete ordinary top-level
  zero-parameter function body once, then returns the original borrowed site,
  identical body region and unique preceding complete direct local `val`.
  Only plain immutable untyped vals with integer or whole-identifier initializers
  are admitted. Semantic-equivalent duplicates of the queried name anywhere in
  that body, including later duplicates, refuse the query; unsupported candidate
  spellings never disappear from the search. Containing and later declarations
  never bind the occurrence. Initializers are not chased transitively.
  Origin, stale, unsupported context/spelling, ambiguity, no eligible local and
  resource errors expose no target; no eligible local is not global unresolvedness.
  One checked 2,097,152-unit work budget charges snapshot comparisons, traversal,
  validation and compared bytes. The query allocates nothing and is nonrecursive.
  This is library-only and is not complete references, nested/outer lookup,
  parameter/receiver/import/overload resolution, types, stable cross-revision
  identity, official navigation evidence or a new server capability.

These consumers retain immutable producer association and require a synchronized
current-document recheck before publication. Document/revision/kind/source
changes invalidate results; reopening must use a new document identity or
revision. Their checked work/capacity limits and allocation refusals are
incremental bounds, not whole-process allocation, RSS, stack or time guarantees.
The raw-name index may scan quadratically within its charged work limit;
ancestor queries use fixed storage and position queries allocate nothing.

### Bounded positive integer initializer types (#330 / T012 child)

`integer_initializer_types::infer_integer_initializer` accepts only a private-field
`LocalValueBinding` and current snapshot. It authenticates that binding's original
tree using `ReferenceSite::tree`, then inspects only its already-validated direct
target initializer, without validating the complete body again. Success borrows
the original binding, integer token and original source spelling/span, with an
exact nonnegative `u64` value and default `Int`, `Long`, `UInt` or `ULong`.
An unsuffixed signed value defaults to `Int` through 2,147,483,647 and `Long`
through 9,223,372,036,854,775,807. `u`/`U` defaults to `UInt` through
4,294,967,295, otherwise `ULong` through 18,446,744,073,709,551,615.
`L` forces `Long`; `uL`/`UL` forces `ULong`. Lowercase `l` is refused.
Decimal, `0x`/`0X` hexadecimal and `0b`/`0B` binary spellings permit repeated
underscores only between digits; decimal leading zeroes are invalid.
Invalid spelling and overflow return errors, never a fallback type.

Rules were read before implementation from the pinned
[Kotlin specification, integer literals](https://github.com/Kotlin/kotlin-spec/blob/7fccf8d6ee8210612ee8d55fa2a24fbb7c3ceea9/docs/src/md/kotlin.core/expressions.md#integer-literals)
and Kotlin v2.2.20 source commit `693c44ee79f62895a9b92bdd60fdd7a9bc29a975`:
[numeric suffix/range decoding](https://github.com/JetBrains/kotlin/blob/693c44ee79f62895a9b92bdd60fdd7a9bc29a975/compiler/psi/psi-api/src/org/jetbrains/kotlin/psi/utils/ConstantExpressionUtils.kt),
[literal lexer](https://github.com/JetBrains/kotlin/blob/693c44ee79f62895a9b92bdd60fdd7a9bc29a975/compiler/psi/parser/src/org/jetbrains/kotlin/lexer/Kotlin.flex),
[lowercase-long rejection](https://github.com/JetBrains/kotlin/blob/693c44ee79f62895a9b92bdd60fdd7a9bc29a975/compiler/frontend/src/org/jetbrains/kotlin/resolve/constants/CompileTimeConstantChecker.java)
and [default signed/unsigned selection](https://github.com/JetBrains/kotlin/blob/693c44ee79f62895a9b92bdd60fdd7a9bc29a975/compiler/frontend/src/org/jetbrains/kotlin/resolve/constants/evaluate/ConstantExpressionEvaluator.kt).
The Rust implementation is independent; these source rules are not an execution
of the official LSP, a language-version configuration or full T012 acceptance.

Reference initializers refuse without chasing. Expected types, integer-literal
constraint types, unary minus (including minimum signed values), folding,
smart casts, overloads, generics and external lookup are excluded.
Origin/stale/resource errors remain distinct. One checked 2,097,152-unit query
budget charges snapshot/source comparisons, target visits/kind comparisons and
raw literal bytes; the query allocates nothing and is nonrecursive. This is a
library-only subset with no server capability, stable revision identity or
whole-process memory/time bound. Native consumer tests require actual parser
production of signed and unsigned cases and admission of the frozen invalid/
overflow spellings all the way to the integer query: an upstream refusal fails
the test rather than substituting decoder-only evidence. Hosted execution
evidence must be recorded separately from source.

ROOT composed #346/PR348 source `e9f8aeec5396c6f76a4c76ab8b5f74ea78dfd337`
and #347/PR349 source `749ed9854f7b16ab8b2975234d1c58bff378743b` on verified
incoming main `bb6a75f61fd467c9b090210a369c7e1d225713d5`.
[Original package/import CI38866350684/job116859972463](https://github.com/anilbeesetti/ktlsp/actions/runs/38866350684/job/116859972463)
passed 896 workspace tests; its actual LF/CRLF production consumer retained
authenticated reference/binding origin. The earlier malformed-header assertion
failure was fixed in the shared lowerer, not relaxed: recovered error-bearing
header tails now refuse the whole projection.
[Original integer CI38865103037/job116855978067](https://github.com/anilbeesetti/ktlsp/actions/runs/38865103037/job/116855978067)
passed 895 workspace tests. ROOT authenticated thirteen direct production
observations across Int/Long/UInt/ULong boundaries and spelling variants,
retaining original binding/literal/source associations; strict negative cases
also reached the actual producer. Only the temporary stderr proof writers were
removed after those observations; permanent behavioral assertions remain.
Those are source-component receipts, not final aggregate CI or merge evidence.
No local parser runtime ran; #96 and full semantic parity remain open.

At #333 source head `a22b54619f6996e03cceee56362453a9b846d574`,
[original attempt-1 Linux CI38857165917/job116831130221](https://github.com/anilbeesetti/ktlsp/actions/runs/38857165917/job/116831130221)
passed pinned format, locked workspace Clippy and all 890 workspace tests.
Its synthetic checkout `78dc92de87802132f3451d51c44c7e4b4a58ec47` has
ordered parents `[a52f9518ff1532c25aff8fc1bdb1c566a05526b4,a22b54619f6996e03cceee56362453a9b846d574]`
and tree `6b32ab3c8b3a04052cdf8e7ba01ee03a3b8a7fd3`.
ROOT independently authenticated the original API/log and observed occurrence
`29..30` binding to declaration `10..19`, name `14..15`, with original target,
body and site pointer associations all true. The temporary direct-`Write` marker
was then removed; its production assertions remain. This is bounded library
consumer proof, not official navigation or Mac/Windows reliability acceptance.
Final reviewed integration-head CI and guarded merge are separate gates.

At source head `b94e6ded912d72c700be9f3e8d09ab923c4fb5a3`,
[original attempt-1 Linux CI38850851404/job116811504205](https://github.com/anilbeesetti/ktlsp/actions/runs/38850851404/job/116811504205)
checked synthetic merge `fad71a4c94b9e154ce90503f24cae5d39d27b7db`,
ordered parents `[2e408cfd8d65c68a9eede863e3f983a2db720d95,b94e6ded912d72c700be9f3e8d09ab923c4fb5a3]`,
tree `1436973f46d32175ec16398be571dc44c4e16a2d`. Unchanged pinned
format, locked all-target workspace Clippy and locked full workspace tests passed.
The actual supervised parser → lowerer → structural-directory observation
returned 37 CST rows/27 tokens, zero diagnostics/errors, complete `f`/`x`/`y`
declarations, two regions, memberships `[0, 1, 1]` and all three original-producer
association checks true. The temporary direct-`Write` marker was removed after
inspection; the substantive production and synthetic regressions remain.
Final aggregate reviewed-head CI and guarded integration are separate gates.
No local parser runtime/tests ran under #96.
T008/T008a, full T009 and broader recovery/binding acceptance remain pending;
#96 remains OPEN/UNKNOWN. No official oracle or platform-parity proof is claimed.

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

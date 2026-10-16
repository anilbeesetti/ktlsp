# T007 / #62 parser adoption safety gate

This library invokes the separately built `ktlsp-parser-worker` through an explicit
trusted absolute path. Build both with `cargo build --locked -p ktlsp-parser`.
`Parser::parse` accepts source, source/script kind, and a cancellation flag; it
returns a checked flat CST with original byte spans and syntax diagnostics, or a
named rejection. Source is never replaced with parser-generated text. The server
consumes the validated CST for its bounded syntactic document-symbol provider
(see [server limits](../ktlsp-server/README.md)); declaration-name documentation
also has the finite source-owned operation below. This is not full grammar, recovery, HIR, formatter or
official Kotlin LSP parity.

Each call starts one Rust worker. On Unix, the parent transfers input and drains
stdout through one nonblocking `UnixStream` endpoint; the child's duplicated
stdin/stdout endpoint stays blocking. Input uses byte offsets and explicit write
half-close for EOF, including best-effort half-close after a failed write. Each
iteration attempts at most one 8 KiB read and one 8 KiB write, one pending
half-close and one child-status poll. Cancellation precedes the original 500 ms
deadline. Productive iterations continue; idle iterations wait for the unfinished
read/write directions with one `poll` on the existing socket, requesting at most
2 ms before another cancellation/deadline/child-status check. Completed writers
are not watched for writability, and a fully finished exchange omits the socket
while waiting for child status, avoiding writable/EOF busy loops. No Unix I/O
threads or scoped joins remain. Non-Unix retains the pipe/scoped-thread path.

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
Child in static storage. Each reservation retains the originating thread handle
under the same metadata mutex as its Child. The caller closes its transport,
publishes cleanup, drops that mutex, and then unparks the retained reaper. The
reaper requests termination and waits for that same child; no caller-side
kill/wait/join remains on Unix.

Cleanup still has its original strict 20 ms observation grace, with clock samples
both before and after the ownership/death observation. Reaper and owner use
`park_timeout` with the existing 2 ms cap instead of timer-only sleeps. Before
publishing confirmation or failure, the reaper captures the original owner and
releases the metadata mutex; only then does it publish and unpark that owner.
Thus immediate confirmation consumption cannot mistake the reaper's own lock for
invalid ownership. Stored unpark tokens cover notification before parking;
spurious or stale tokens only trigger another unchanged state/deadline check.
Slot reuse clears the retained owner, and an already captured notification still
targets the old thread, never a later reservation. Startup/reaper failure also
wakes registered waiters. There are still eight slots and one retained reaper;
no per-request reader thread, queue, or new cleanup deadline is introduced.

The original changed-source macOS probe run
[38889917253, job 116936629275](https://github.com/anilbeesetti/ktlsp/actions/runs/38889917253/job/116936629275)
on source `51f011a9b34b8b53ef519fdb939fe7cb2181cb96` observed same-child
`CONFIRMED` only after the 20 ms grace (including a 21.5645 ms CLI sample and
a 30.648 ms controlled Session sample). It did not timestamp publication, prove
confirmation occurred within grace, or establish OS timer coalescing as the cause.
The notification repair requires its own changed-source hosted evidence. Waking
a thread does not guarantee the scheduler runs it within grace: late or otherwise
unproved cleanup still fails closed, with no later promotion of the original
request's boolean.

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

`Parsed::position` lazily retains an immutable original-byte UTF-16 table, built
in one forward source sweep for arbitrary-order constant-time lookups. Trees
whose positions are never queried allocate no table. At the unchanged 64 KiB
source ceiling, at most 65,537 two-u32 entries occupy 524,296 payload bytes per
queried tree. Fallible reservation checks actual Vec capacity with checked byte
arithmetic against that bound; excess capacity is dropped, not retained. Vec
storage is retained without a boxed-slice shrink/reallocation. Allocation refusal
is remembered once per tree and preserves the exact scalar conversion instead
of adding a valid-offset error or repeatedly trying allocation. This trades
parent-side retained storage for eliminating repeated source rescans; it changes
neither worker/query allowances nor source limits and is not an allocator/RSS
guarantee. Larger future source ceilings would require reconsidering this table.
Strict UTF-8/CRLF interiors remain invalid; source, snapshot and old-tree identity
are unchanged. No measured speedup is claimed before changed-source hosted CI.

For #404, ROOT's admitted original full Native debug workspace/platform run must
retain all frozen position totals (231,567 / 262,756 / 206,112 / 142,131). Compare
the unchanged 65,536-byte cap-case completion intervals, in longer-simple /
qualified / nullable-two / two-argument order, against 63.4100483 / 61.7083814 /
63.0678637 / 55.1976965 seconds from original Native job 117212394570. These
intervals include case assertions and healthy followups, not isolated CPU time;
release/platform timings are separate compatibility evidence, not debug parity.

Linux native checks do not establish Darwin/Windows reliability, resolve #96 or
close B3/B4; historical MissingACK observations stay inconclusive. No comparative
speed/RAM claim follows from the transport change. The parent capacity checks do not
bound whole-process allocation, stack or RSS.

### macOS fault evidence (#332)

The original macOS release run
[38854469458/job116822734134](https://github.com/anilbeesetti/ktlsp/actions/runs/38854469458/job/116822734134)
failed the 4096-byte deadline-receipt datagram send with `Message too long`, and
selected `Timeout` rather than the required `OutputLimit` for the oversize fixture.
The receipt regression now sends an 18-byte valid-prefix datagram, which still
exercises truncation by the consumer's 17-byte receive buffer. A separate
4096-byte valid-prefix byte-slice regression preserves exact acknowledgement
rejection without assuming the OS can transmit that datagram.

The original logs did not establish the oversize cause. The later original
changed-source release run
[38868405795/job116866483405](https://github.com/anilbeesetti/ktlsp/actions/runs/38868405795/job/116866483405),
at head `10c62efdbb109a3e8a097abd7133248f7b75526b`, recorded only 458,752 output
bytes before selecting `Timeout` at 505.513 ms, with all eight input bytes sent
and half-closed, no output EOF and no successful child exit observed. Repeated
8 KiB progress/`WouldBlock` pairs were followed by roughly 18 ms idle gaps.
The unconditional 2 ms sleep request was timer-throttling output draining on
that observed host; the original strict `OutputLimit` assertion still failed.

This repair replaces that sleep with readiness-driven `poll`, borrowing only
the existing socket and retaining the per-tick I/O budgets and original clock.
The positive wait is capped at 2 ms and rounds sub-millisecond remainders up to
poll's integer-millisecond granularity; no deadline value is extended. Readiness,
timeout, `EINTR` and hangup return to the ordinary control-first pump, without
retrying the wait. Unexpected poll failure also checks cancellation/deadline
before selecting `Io`. The direct Unix `libc = 0.2.190` OS-binding edge reuses
the already audited locked package; it adds no C build or semantic engine.
Changed-source hosted macOS release proof is still required; this source repair
alone is not a macOS reliability claim.
For ROOT-admitted **changed-source hosted** tests, set
`KTLSP_NATIVE_IO_OBSERVE=1` and run the release workspace tests with `--nocapture`.
Only the unit-test fault-worker command is observed; no product instrumentation
or parser/source logging is compiled into ordinary builds.

`KTLSP_NATIVE_IO_V1` records spawn completion, first read/write, input half-close,
output EOF, observed successful exit, idle/total tick counts, the final 32 scalar
pump states, and selected result/cleanup. At most eight child-stage receipts are
retained: 1 = fixture entered, 2 = first input byte, 3 = input EOF,
4 = oversize output buffer ready immediately before the stdout write call,
5 = that write returns. Stage 4 does not prove the write has executed.
Reads/counts include subprocess test-harness stdout, not only fixture bytes.
Receipt timestamps are parent receive times, not child execution timestamps;
absent stages remain inconclusive. Each collection tick/final drain receives at
most eight tiny datagrams. Setup/send/logging failures never substitute a product
result. Logging does not retry or flush, but a blocking stderr sink can delay
return. Observation has unmeasured
timing effects and leaves cancellation-before-deadline, the original deadline,
exact input/output bounds, and failure-visible `OutputLimit` assertions unchanged.
Local parser runtime remains prohibited under #96. Hosted macOS release full
workspace and supervision proof is still required; #96 remains OPEN/UNKNOWN.

The first changed-source diagnostic workflow,
[38859356634](https://github.com/anilbeesetti/ktlsp/actions/runs/38859356634),
refused both platforms before checkout: the event payload's optional
`merge_commit_sha` was empty, although the immutable source/workflow SHA and
merge ref agreed. Original identity artifacts were retained and independently
rehash-verified. The next original run,
[38864989435](https://github.com/anilbeesetti/ktlsp/actions/runs/38864989435),
also refused before checkout: the now-present payload merge SHA still named the
older test merge while the original event head/base and actual source/workflow
SHA had advanced. That payload field is retained as an observation, not admission
authority. Admission requires the immutable context source/workflow SHA and ref,
then actual ordered Git parents matching the original event base/head before
native execution. Neither refusal diagnosed the Windows source guard or observed
the macOS parser.

The later original changed-source release run
[38875113607/job116888168806](https://github.com/anilbeesetti/ktlsp/actions/runs/38875113607/job/116888168806),
at `0e8027d4905cf4ff7a66c3d860683112d98a8ef3`, passed source guards, format,
strict Clippy and the parser library: 94 passed, zero failed, one ignored,
including the previously failing strict oversize supervision case. It then failed two worker
binary fixtures independently: the bounded queue-fill fixture received Darwin
`ENOBUFS` (55) instead of `WouldBlock`, while the sampled-checkpoint fixture's
post-disarm receive returned `ConnectionReset` after its sole worker endpoint
was dropped. These use thread-local observer state; this was not mutex poisoning.
The release workspace run did not pass.

The worker-fixture repair classifies `ENOBUFS` as queue saturation only on Apple
targets and only in the finite nonblocking fill fixture; all other errors remain
fatal. The 4096-send bound is unchanged. A worker-endpoint alias now stays alive
through empty-queue assertions, matching production's retained original stderr
fd after the observer clone is dropped. This tests absence of an extra ACK
without requiring any particular peer-close errno. The explicit peer-loss case
still drops the parent endpoint and verifies disarm without receiving from it.
The backpressure case drains exactly its queued noise after the first sampled
attempt, then verifies no late ACK when capacity returns. Production checkpoint
sampling, exact challenge validation, one ACK attempt and no-retry semantics are
unchanged. This fixture source repair still requires an original changed-source
hosted run; it establishes no full-workspace pass or #96 resolution.

The next original changed-source run,
[38879411025/job116902229288](https://github.com/anilbeesetti/ktlsp/actions/runs/38879411025/job/116902229288),
at `74135c0a7dcdbb16f75b457a625569c11518488c`, passed the parser library
(101 passed, one ignored) and worker fixtures (eight passed, one ignored).
The frozen real generic-copy observation then returned `Invalid` instead of
its unchanged exact `Complete(GenericCopyRecord)` requirement. An earlier
unselected deadline observation also reported `Invalid`, without claiming a
deadline witness. The logs did not expose which generic-copy receive failed.

Both parent receipt validators run only after confirmed same-child cleanup.
Previously, every worker-side datagram alias had closed by then, so a Darwin
peer-close error could not satisfy the strict `WouldBlock` empty-queue sentinel.
Deadline, generic-copy and native-I/O observation now share a request-owned
datagram channel retaining an inert worker-side alias until validation finishes.
That alias performs no I/O and cannot fabricate a child receipt. Validation
still requires the exact packet, all scalar checks and a clean `WouldBlock`
sentinel; `ConnectionReset`, malformed, truncated and extra packets remain
nonaccepting. Unconfirmed cleanup still drops channels without receipt reads.
Error/unwind/cancellation paths own and drop the same channel inside their
existing reservation. Ordinary unobserved parsing creates no observation channel.

Descriptor accounting is explicit, not a hard process-FD ceiling: the observed
path has at most six known caller-owned FDs before Command disposal (three
stream endpoints, channel receiver/keeper, and Command's child-stderr alias),
up from five without the keeper. The unchanged eight-reservation admission cap
therefore bounds those known observed-path FDs at 48, excluding ambient FDs,
child stdio and Rust/OS spawn internals. The keeper uses the safe standard-library
close-on-exec socket/clone operations; only the intended child stderr mapping
survives exec. After Command disposal, only the parent stream plus two channel
FDs remain; the stream closes before cleanup, and both channel FDs drop after
receipt validation or earlier error/unwind disposal. Public observation APIs
select one channel per request; ordinary parsing retains its three known stream
FDs and allocates no channel. No kernel limits, reservation cap or resource-error
handling are raised or bypassed; failed setup remains failure-visible.

The native-I/O trace already required the subprocess fault-test name, excluding
the real parser command in the failing run. It now also explicitly excludes
both private observation-mode flags, so diagnostic setup cannot replace those
stderr channels even if a future fixture combines arguments. Regressions close
external worker/Command aliases before strict receipt validation, check channel
descriptor `FD_CLOEXEC` flags, preserve malformed/duplicate rejection, and ensure
child closure does not increment invalid native-stage counts. No local runtime
has exercised this repair; full changed-source hosted proof remains required.

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
CRLF. `Parsed::position` preserves ktlsp-text's strict UTF-16/invalid-boundary
behavior through the bounded lazy original-source table described above, with
scalar ktlsp-text conversion when table storage is unavailable.
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

### Source-owned declaration documentation (#397)

`documentation::candidate` only checks an original direct declaration-name token
for a producer-attached leading `DOC_COMMENT`; it is not a documentation result.
No candidate keeps the existing docless lowerer/rendering path unchanged.
For a positive candidate, `documentation::lower` and `select` use one opaque
`Budget` through the original lowerer structure/count/emit passes, authentication,
exact name selection, prefix/comment validation, decoding and caller rendering.
The returned `Attached` borrows the caller-owned original `LoweredDeclarations`
and `Declaration`; equal source from another result/lowering is not authority.
Original declaration identity, owners, names, ranges and incompleteness remain
unchanged. Presentation uses `doc.end..declaration.end`, without repeating KDoc.
Contiguous prose borrows original source; only decoration-discontiguous prose
receives fallibly allocated decoded storage.

Admission is documentation-operation-only: one direct leading KDoc before the
core header, optionally followed by one direct plain modifier list with at most
one visibility (`public/private/protected/internal`) and one finality (`final/open`)
modifier. A modifier list without that declaration's own KDoc still refuses.
Earlier unowned container KDocs are explicitly validated but never promoted;
body, annotation, modifier and ancestor documentation cannot supply prose.
Blank lines and intervening ordinary comments do not override producer ownership.
Existing public `lower_declarations`, value BodyCheck, type and callee proofs
remain strict; documented use-site hover and annotations remain unsupported.

The finite prose form is one default `KDOC_SECTION`: `KDOC_TEXT`, `WHITE_SPACE`
and literal `KDOC_LPAR/KDOC_RPAR`, with producer `KDOC_START`, `KDOC_END` and
`KDOC_LEADING_ASTERISK` decoration removed. All other content bytes, indentation,
paragraphs, Unicode and original LF/CRLF/lone-CR endings are retained, including
delimiter-adjacent spaces. Empty/whitespace-only prose, tags, lexical links,
bracketed external/reference/image syntax, backticks, tilde fences, code leaves,
nested comment delimiters, errors and unsupported/ambiguous shapes refuse the
whole operation, never raw/empty/enclosing fallback. This is not Markdown or KDoc
semantic interpretation, inheritance, tag-derived parameter docs or official parity.

One existing 2,097,152-unit work counter and 131,072-byte requested/actual retained
capacity counters cover lowerer output plus decoded and rendered source-owned
strings, monotonically, with no per-phase reset/refund. Original 64 KiB source,
element and depth limits remain. Caller `reserve_string` only accepts a fresh
zero-capacity string and reserves once fallibly, charging actual capacity and
dropping an over-capacity allocation on refusal; implicit later growth is forbidden.
`work`, `storage` and `output` extend the same counters through escaping, fences,
UTF-16 and the complete escaped success response, at most 1 MiB including request
identity/envelope. This is an upper bound, not a guarantee that every 1 MiB response
fits the tighter managed storage/work ceilings. The borrowed cancellation observer
uses the caller's existing token and creates no new deadline. Existing fixed-shape
JSON/map/envelope allocation, allocator overhead, borrowed inputs, whole-process
heap/RSS and transport OOM remain inherited exclusions, not fallible-storage claims.

`tests/documentation.rs` retains original-worker source/CST writers before
assertions for ROOT-hosted ownership, token partition, ranges, line endings,
prefix/refusal and identity evidence; unit tests cover exact/late budget,
preexisting and actual capacity, overflow and cancellation boundaries.
These new checks have not been executed locally; actual hosted producer/wire
evidence and integration verification remain ROOT-gated under #96.

### Existing finite declaration consumers

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
- Reference-, parameter- or leading-type-parameter-bearing outer functions require
  an ordinary named top-level function with an ordered brace body of plain
  immutable local `val`s, direct calls and ordinary direct local functions.
  Local initializers remain integer constants or whole-identifier references;
  optional annotations retain one simple bare/backtick type name. Ordinary parameters
  require exactly a name, colon and the same simple type-name shape, separated
  by commas with one optional trailing comma after a real parameter. Positive
  complete shape is mandatory even when the parser reports no diagnostics or
  ordinary value references. One nonempty leading `<T, U,>` list is admitted:
  unmodified supported bare/backtick identifiers, supported header/list trivia
  and one optional real trailing comma. Bounds, variance, reified, annotations,
  where clauses, empty/malformed/post-name/second lists remain refused, as do
  missing/reordered/repeated syntax, defaults, vararg, richer types,
  receivers/context/explicit returns, expression bodies,
  members/constructors/lambdas/deeper nesting/destructuring and relevant recovery.
  This is syntax admission, not generic inference or semantic non-nullability:
  unbounded `T` has implicit bound `Any?`, despite having no nullable syntax.
- `Declaration::semantic_spelling` separately supports ASCII bare names and
  nonempty backtick bodies without backticks, CR/LF or NUL, borrowing unchanged
  bytes without normalization; unsupported names return `None`, not an identity.
- `structural_regions::build_regions` authenticates the exact original lowerer
  and snapshot result, then supplies file/class-body/function-body regions and
  ordered declaration memberships. Equal bytes from an independent parse do
  not establish origin. Regions and declaration owners are not Kotlin scopes.
  Original `VALUE_PARAMETER` and `TYPE_PARAMETER` declarations retain their
  original parameter-list parent and enclosing function owner ordinal. Top-level
  header parameters remain file structural members; direct local-function header
  parameters remain members of the enclosing outer block, not of the local
  function's own body. Neither membership establishes visibility. No property,
  `val` text or body membership is fabricated, and raw name indices are not visibility rules.
- `declaration_name_index` groups exact **raw** spellings within an identical
  borrowed region, retaining duplicates and result-local declaration ordinals.
  `ancestor_candidates` returns all such groups along structural ancestors,
  nearest first, without picking a semantic winner or applying visibility,
  shadowing, declaration-order or backtick-equivalence rules.
- `position_regions::containing_regions` accepts original UTF-8 boundaries and
  returns every half-open containing region in directory order; brace bytes
  count, EOF matches nothing. It neither authenticates a reference occurrence
  nor converts UTF-16 positions; callers convert against the identical source.
- `call_syntax::call_site(lowered, result, current, call_index)` selects an
  **original** standalone direct `CALL_EXPRESSION`. It retains the original call,
  callee reference/name, argument list/parentheses, enclosing function/body and
  source-ordered argument records. Each record borrows its original wrapper,
  integer/simple-reference expression/token, optional named-label wrapper/reference/
  identifier/equals and following comma (including a trailing comma). Empty lists,
  unknown callees, repeated labels and mismatched names/arity/type spellings are
  syntax views, not successful invocations. Qualified/safe/chained/expression
  callees, type arguments, spread, trailing lambdas, nested/richer values,
  incomplete/error-bearing lists and unsupported ancestry refuse atomically.
- `call_syntax::callable_parameters(lowered, result, current, original_function)`
  takes an explicitly selected original local `FUN` **Declaration**, never a
  callee or name. Only direct ordinary non-generic local functions with empty
  bodies and required simple-typed parameters qualify. Returned parameter records
  preserve original borrowed Declaration identity, parameter-list ordinal,
  annotation syntax and following comma; declaration name/full ranges remain on
  the original Declaration. Duplicate names stay distinct. Defaults, modifiers,
  varargs, receivers and richer types refuse; raw `Int`/`T` never resolves a type.
  Parameter declaration order is independent of call argument order.
  Both kernels retain the existing monotonic ReferenceBudget across authentication,
  source/name comparison, role/delimiter inspection, count/reserve/emission and
  final construction. Fallible exact reservation is bounded cumulatively to
  131,072 requested/actual retained bytes and 1,024 records per vector; work stays
  at 2,097,152 units. Composition uses the private shared-budget kernels, not
  chained public-query resets. These finite borrowed syntax views add no target
  selection, argument-to-parameter mapping, applicability, inferred type, active
  parameter/signature, provider or capability. `ReferenceSite` remains initializer-
  only; complete semantic BodyCheck and non_value_reference are unchanged, so
  newly admitted rich calls/parameterized local functions still atomically refuse
  all existing rich-body semantic consumers.
  `tests/call_syntax.rs` retains original supervised-worker identity, role,
  ownership, actual edit/reparse/rebinding and foreign/stale/refusal assertions.
  Synthetic accounting tests cover exact/one-over/overflow and call-then-parameter
  shared work/storage. PR406's first original Linux/macOS/Windows full workflows
  passed; independent OWNER and ROOT completely read all ten original actual
  source/CST/declaration/projection packets and the four-edit record per platform
  artifact, independently applied those actual edits and matched the full
  reparsed source. Only then were temporary JSON writers/bookkeeping removed;
  permanent pointer assertions, actual edits and post-binding queries remain.
  [Exact first-source receipts and limits](../../docs/status/development.md#first-original406-full-ci-and-actual-source-readback)
  are not capture-free final-source or exact-main acceptance. Those require their
  own original full CI and guarded integration; no filtered rerun substitutes.
  Existing paired-CLI mixed value/type/callee regressions remain required.
  No signature help or official Kotlin parity is established; #96 is unchanged.
- `lowering::reference_site` authenticates an original whole-initializer
  `REFERENCE_EXPRESSION` and its sole identifier token against the exact lowerer,
  parsed tree, result and current snapshot. Private fields retain borrowed origin,
  original range/spelling, containing property, body CST index and function.
  `ReferenceSite::semantic_spelling` reuses the declaration spelling helper.
  Annotation `TYPE_REFERENCE(USER_TYPE(REFERENCE_EXPRESSION(IDENTIFIER)))`
  subtrees are authenticated retained syntax, not local value occurrences or
  successful reference sites. Only the separate annotation-only lexical query
  below resolves a same-function type parameter; builtin/class/import names
  remain unresolved. Qualified, nullable, generic, annotated,
  parenthesized, function, intersection and dynamic types remain refused, as do
  malformed/reordered annotations. Binder consumers reuse the lowerer's shape,
  not a second type grammar. Explicitly typed integer-initializer query targets
  return `UnsupportedContext` rather than pretending their declaration type is
  the literal's default `Int`; an annotated containing val does not invalidate
  inference for a different untyped target. Original declarations and hover text
  retain annotations unchanged. This finite extension is not Kotlin type resolution.
  At component source `9363430fb9543569bbe31603b0b3690f8aee2cae`, original
  [attempt-1 Linux CI38884530092/job116918948331](https://github.com/anilbeesetti/ktlsp/actions/runs/38884530092/job/116918948331)
  passed pinned format, strict locked workspace Clippy and full workspace tests:
  962 passes, zero failures, two ignored, plus 22 separate protocol fault cases.
  Actual checkout `c0554127ded7a16f3c1f08e11da4893f9d0f69a0` had tree
  `3e807212e5b0f32320b7caa6ad51b532b5e01357` and ordered parents
  `[abe98694226c86fced3aed31691b8f0945a565e0,9363430fb9543569bbe31603b0b3690f8aee2cae]`.
  Owner and Root independently read two actual producer trees, six production
  binding/occurrence/integer-query payloads and two real-worker Session JSON
  definition/hover payloads. Annotated targets refused integer-default inference;
  an untyped target retained `Int` despite its annotated containing val. Original
  target association, LF/CRLF/nonBMP ranges and exact declaration hover were
  preserved, excluding type-name references. This is Session-provider evidence,
  not typed packaged CLI/stdio or general type resolution. Temporary observation
  writers are removed; permanent behavioral assertions remain. This receipt
  covers the original component source, not cleanup-head runtime validation,
  composed integration or Darwin reliability; no local parser/runtime tests ran
  and #96 remains OPEN/UNKNOWN. See [#363 / PR365 receipt](https://github.com/anilbeesetti/ktlsp/pull/365#issuecomment-6213206923).
- `local_value_binding::bind_local_value` validates the complete admitted top-level
  function body, then returns the original borrowed site, identical body region
  and target. For the queried semantic spelling, duplicates are counted separately
  in parameter and local scopes, including later duplicates. A unique eligible
  preceding complete direct immutable local `val` wins; otherwise the unique
  parameter of that identical function. A local/parameter collision is legitimate
  shadowing: before-shadow and `val p=p` initializer reads bind the parameter;
  later reads bind the local. Unrelated duplicate-name behavior is unchanged.
  Containing/later locals never bind the occurrence, and initializers are not
  chased. Unsupported candidate spellings never disappear from the search.
  Origin, stale, unsupported context/spelling, ambiguity, no eligible target and
  resource errors expose no target; refusal is not global unresolvedness.
  One checked 2,097,152-unit work budget charges snapshot comparisons, traversal,
  validation and compared bytes. Binding allocates nothing and is nonrecursive.
  This is not complete nested/outer/member/receiver/import/overload resolution,
  types, stable cross-revision identity or official navigation equivalence.
- `local_value_binding::bind_local_function_callee(directory, result, callee_index,
  current)` adds definition-only lookup for a direct standalone plain empty call
  to an original preceding same-block local `FUN`. Local targets have ordinary
  non-generic zero-parameter headers and empty bodies. The complete ordinary
  top-level outer function (including retained #386 generic headers/annotations)
  is validated; **every** call must prove this same local-function identity.
  Callees are a distinct authenticated role, never initializer `ReferenceSite`s.
  All original value binding, integer-default, visibility, occurrences and
  guarded rename remain available in admitted mixed bodies. Local function
  declarations, type parameters and callees never enter value completions or occurrences.
  Lookup inspects every relevant semantic-equivalent local declaration and
  parameter before eligibility; unsupported competition, overloads (including
  different arities), values/classifiers, unsupported spellings and richer call
  shapes refuse atomically. Later locals are not forward candidates; conflicts
  with future declarations are conservative refusals, not visibility claims.
  A proved local positive needs no guessed package/import universe. Rename
  conflicts with any relevant local-function name refuse, preserving proved
  callees after fresh value edits. One checked 2,097,152-unit reference budget
  covers origin, complete body, every callee and all competitor/rename work.
  Private lookup never recursively invokes public body validation, allocates no
  names/candidates, and selected-call resolution is not repeated. Permanent real
  producer and paired-CLI tests apply actual edits and rebind updated source.
  ROOT and the source owner read all three complete original CST/target packets
  and real mixed-body/returned-edit/rebind wire on both original platform hosts;
  corrected-source38941146673 passed all full workspace checks. Writers were
  removed only after readback; meaningful identity/edit/refusal assertions remain.
  This is hosted native behavior, not local runtime or official equivalence.
  General overload/invoke, nonempty/nested local bodies, package/global fallback,
  callee references/hover and function rename remain unsupported;
  #96 quarantine and the full-parity gap are unchanged.
- `local_value_binding::bind_lexical_type_parameter(directory, result,
  annotation_reference_index, current)` authenticates original snapshot/tree
  association, exact reference/token/USER_TYPE/TYPE_REFERENCE/parameter-or-local
  ancestry, enclosing function and complete admitted body. It scans the entire
  same-function type-parameter list under ONE existing 2,097,152-unit reference
  budget, including spelling/comparison work after the first match. A unique
  semantic spelling returns original borrowed annotated declaration, function,
  body, reference/name token and `TYPE_PARAMETER` target. Matching duplicates
  (including later `T`/backtick-T), no match, foreign/stale snapshots, unsupported
  context/spelling and exhaustion refuse atomically. No values, other functions,
  outer classes, package names or imports are fallback candidates. This query
  allocates nothing; value completion/occurrences/rename remain value-only.
  Original platform38930044300 at source1f118a4 directly retained four complete
  producer packets per host; ROOT and the owner independently authenticated
  and read original CST/name/owner associations and paired-CLI edit rebinding.
  Temporary observation writers/bookkeeping were removed afterward; all
  original semantic assertions remain. Cleanup-source CI/main are separate
  gates. That historical evidence does not establish official equivalence,
  typeDefinition, type-parameter hover or generic inference; #96/full parity stay open.
- `local_value_binding::type_parameter_occurrences` and
  `guarded_type_parameter_rename` accept positive `TypeParameterAnchor` roles:
  an original function `TYPE_PARAMETER` declaration (even unused), or an
  authenticated simple parameter/direct-local annotation reference. Returned
  `TypeParameterAnnotation` records retain original annotated declaration,
  reference/name-token identities and source order; no value site is fabricated.
  The existing definition query shares private ancestry/function/list helpers.
  Each query authenticates its original tree and completes body/callee validation
  once under one 2,097,152-unit budget, including the complete type list, **all**
  same-function annotations, borrowed-record reservation/capacity and emission.
  Matching semantic duplicates, stale/foreign origins and exhaustion fail atomically.
  Rename checks both edited and unedited annotation identities: another type
  parameter or any unrelated simple annotation with the new spelling refuses
  capture, including header parameters and unused targets. Value names and proved
  local function/callee names are separate namespaces, not automatic collisions.
  Output remains bare ASCII with existing spelling/boundary limits; native
  contextual outputs `where`, `dynamic` and `suspend` refuse before same-name quote
  removal. `out`/`reified` edited-producer/rebinding regressions are included, not
  blanket-classified as invalid modifiers. Storage remains at most 1,024 records
  and 131,072 requested/retained bytes, measured using the actual annotation width.
  This is finite same-function lexical refactoring, not full type resolution,
  type hover/typeDefinition, workspace rename or official equivalence. New hosted
  producer/wire observations and checks remain ROOT-gated; no local runtime ran.
- `local_value_binding::visible_local_values(directory, result, initializer_index,
  current_snapshot)` authenticates an existing whole-identifier initializer
  reference without requiring its spelling to bind. The owned result retains
  the original site, borrowed directory/body and original declaration pointers
  in declaration preorder. Same-function ordinary parameters and eligible
  preceding direct immutable locals participate; containing/future locals do
  not, and a preceding local hides its parameter. Every visible candidate name
  must be unique separately in parameter/local scopes, including later
  duplicates of that name. Unrelated future-only duplicates are not visible.
  Ambiguity or unsupported spelling/context refuses the entire result, never
  a prefix. Authentication, snapshot comparisons, complete body validation,
  candidate scans, counting, exact fallible reservation and emission share one
  2,097,152-work budget, starting before index selection. Output is capped at
  1,024 borrowed declarations and 131,072 requested/retained capacity bytes.
  Quadratic borrowed scans deliberately trade larger-body admission for no
  additional index. `tests/visible_local_values.rs` retains real-producer
  original-pointer/order/shadow/snapshot/refusal assertions; private tests cover
  exact shared work and storage boundaries. At component source
  `cbd4f5f01885f24283abb30b9381d1eac9dc3a20`, original attempt-1
  [Linux Native38905509897/job116987948413](https://github.com/anilbeesetti/ktlsp/actions/runs/38905509897/job/116987948413)
  passed 991 standard/doctests plus 22 separate worker fault cases. Original
  [platform38905510309](https://github.com/anilbeesetti/ktlsp/actions/runs/38905510309)
  passed macOS job116987949539 (989+22) and Windows job116987949723 (936+22).
  Both platform jobs authenticated actual PR merge
  `e656c43ce980b5c8af760eb6745cc28c811ecf5f`, ordered parents verified main
  `594cc51cb4f1994b5e4e896e3a3c171b0ff3bcf4` and that component source,
  and identical source tree `9dfcb9ad8a16a0a8fa177c1223068df335e96b26`.
  OWNER and ROOT directly read all 26 actual producer/public-consumer records
  per platform, preserving original parameter-list/function versus local/block
  membership, before/self/after-shadow winners, unknown-query empty visibility,
  sibling isolation and CRLF/nonBMP/backtick identity. Only the two temporary
  capture writers were removed after ROOT's readback; all behavioral assertions
  remain. This is original component PR proof, not final-source, union or
  exact-main acceptance; #96 remains OPEN/UNKNOWN. No caret/hole, completion
  capability, initializer chasing, global scope/type resolution or parity is
  claimed.
- `local_value_binding::local_value_occurrences` accepts precisely an original
  `LocalValueAnchor::Declaration` or a `ReferenceExpression` CST-index anchor.
  The identical directory/lowerer/result authenticates selection, including
  unused local/parameter targets. Reference-anchor authentication and binding
  consume the SAME 2,097,152-unit budget as collection, not a fresh preliminary
  query. Parameter ownership identifies the unique introduced function body;
  header type references never become value occurrences. Original borrowed
  target/body and whole-initializer sites are returned in source order.
  Same-body sites bound to a different target are excluded, preserving separate
  parameter/local shadow sets; different-body/origin/stale failures remain.
  Query-name duplicates refuse even unused targets; unrelated duplicates do not
  make this target ambiguous. The whole function/candidate spelling must be
  admitted. Snapshot/name comparisons, scanning, binding and emission share that
  one budget, without per-candidate resets.
  Before allocation, a count pass bounds storage to 1,024 sites / 131,072 bytes;
  reservation failures or excessive returned capacity refuse the whole query.
  Borrowed source/producer objects are not copied. Repeated binding and
  authentication scans are quadratic in admitted input, including snapshot bytes;
  large otherwise valid bodies can therefore return `Resource`, never a partial
  publishable result. This kernel supports finite native providers, not complete
  Kotlin resolution, workspace references or official navigation equivalence.
  Permanent `tests/local_value_occurrences.rs` assertions use the production
  parser. At component source `12298d8a2b8ff835593f283f07370b1480a92155`,
  [original attempt-1 Linux CI38876375538/job116892312858](https://github.com/anilbeesetti/ktlsp/actions/runs/38876375538/job/116892312858)
  passed pinned format, locked all-target Clippy and full workspace tests.
  Actual checkout `20ca840932deb5948fc17991021d4cb2033f6a77` had tree
  `43c20d6922879daf8d623db273e6fa33aa84c6fd` and ordered parents
  `[bb6a75f61fd467c9b090210a369c7e1d225713d5,12298d8a2b8ff835593f283f07370b1480a92155]`.
  Owner and Root independently read all eight actual production JSON payloads:
  original `x` name14..15 had exactly sites29..30/51..52 amid unrelated repeated
  names; unused targets stayed empty; the separate function had only111..112;
  quoted `x` sites were31..32/42..45; quoted crab site34..40 was UTF-16
  (0,32)..(0,36); mixed forward/self/after had only49..50, with self-only and
  forward-only empty. All three production occurrence tests and the exact-budget/
  storage assertions passed. The temporary direct observation writer is removed;
  permanent behavioral assertions remain. This receipt covers that component
  source only, not the cleanup head, a composed integration, aggregate test counts,
  official equivalence or Darwin reliability. No local parser/runtime/tests ran;
  #96 remains OPEN/UNKNOWN. See child #357 under #330 (T027/#27) and the
  [component receipt](https://github.com/anilbeesetti/ktlsp/pull/361#issuecomment-6211943713).

### Ordinary parameter source observation (#369 / PR370)

At source `9e96b16bcee0e10cf9228a7e467860c52576e9ec`, tree
`351c9b2a81ecb1e127190e237e7cefc5294152c8`, original attempt-1
[Native38898391304/job116964308157](https://github.com/anilbeesetti/ktlsp/actions/runs/38898391304/job/116964308157)
passed 983 standard/doctest cases plus 22 separate worker fault cases.
[Platform38898391737](https://github.com/anilbeesetti/ktlsp/actions/runs/38898391737)
passed macOS981 and Windows928 standard cases plus22 separate cases each.
All checked synthetic `16182690c9275ad3801852e7f774a4d4f7f81d78`,
ordered parents `[3047b69d5f8bf57286eb78338b23fd1305694974,9e96b16bcee0e10cf9228a7e467860c52576e9ec]`,
with the exact source tree, pinned format/strict locked all-target Clippy/full
workspace tests. Platform release tests additionally used `--no-fail-fast -- --nocapture`.

ROOT directly read identical actual macOS/Windows producer and paired-CLI captures.
The CRLF/nonBMP/backtick producer retained parameter CST11/name15..22/full15..30,
original list CST9 and enclosing FUN CST3/declaration ordinal0. Both header
parameters stayed FILE membership0; local `y` stayed BLOCK CST32/membership1.
Value occurrence67..74 associated with the parameter; annotation57..64 did not
become a value site. Parameter targets explicitly refuse integer-default inference
before literal work; typed/untyped-local rules remain unchanged.

Only the three temporary stdout capture writers were removed after direct readback;
all permanent behavior/origin/freshness/shared-budget/atomic-refusal assertions
remain. This receipt covers that first source, not an unexecuted cleanup/final head
or official equivalence. No local parser/server/peer runtime ran; #96 and full
scope/type/completion/official parity remain open.


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

## Guarded local immutable-val rename (#379)

`guarded_local_value_rename` authenticates an original declaration/use anchor
through the private occurrence collector and retains its original local-val
declaration and every bound whole-initializer reference. `None` prepares that
local target; parameters are explicitly refused because external named-argument
callers are outside same-body analysis. The ordinary top-level function and its
complete admitted body must pass the existing shape checks.

A proposed name uses deliberately bare ASCII Kotlin spelling (letter/underscore,
then letters/digits/underscores, excluding all-underscore names and hard keywords).
There is no automatic quoting, Unicode normalization or full Kotlin name-validator
claim. Existing supported quoted/Unicode targets may become supported ASCII.
New-name validation, original collection, parameter/local conflicts (including
future declarations) and potential capture checks consume the SAME 2,097,152-unit
semantic budget, with the existing 1,024-site/131,072-byte storage bounds. An
otherwise external/unresolved reference that would become captured is refused,
never assumed globally safe. Same-semantic-name quote removal retains existing
parameter/local shadow partitions. No partial edit set is returned on refusal.
Original replacement boundaries must remain separate identifier tokens, even
when preparing or removing semantically redundant quotes: `val` immediately
followed by a quoted name is refused rather than emitting `valfresh`.

The server adds versioned publication, growth and escaped-output guards; this is
not parameter/member/workspace/full Kotlin rename or official parity. Permanent
real-worker tests apply edits in memory and reparse/rebind, but execution remains
hosted-only under #96; source changes do not imply new runtime acceptance.

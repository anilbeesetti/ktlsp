# #83 B3 finite original-deadline grammar observation

The outer supervisor Timeout alone does not identify a parser checkpoint. This
change adds an opt-in Unix observation on the same production worker/request:
the original main-loop deadline branch sends one nonblocking decision token,
then immediately takes its existing cleanup. If actual vendor Parser::eof next
receives that token before termination, it may emit one ACK. Missing/invalid ACK
is inconclusive. No wait, yield, sleep, retry, grace, replacement supervisor,
new relative deadline or guaranteed witness is introduced.

The coordinator approved frozen contract v3 SHA256
`b656c26049a71d3d6cf13ee706ebfa9c6854a66876bc8af0674a446ff3356b62`
and its finite acceptance interpretation before source work. Exact assigned source
base is released PR195 main `3baf05494c142416c8cd4641707e1c7dbed0a539`, tree
`a2ba0441577cc8ca8e7acab201c83789c7e1bd0a`, sole parent
`e5bd9e988db9f831982413e04e4b32f9e284b394`. The owned branch fast-forwarded to
that exact base before edits. Historical v1/v2/feasibility HOLD stays preserved;
no past Timeout is reclassified as a positive witness.

## Causal fact and API

Doc-hidden Unix `Parser::parse_snapshot_observing_deadline` returns the ordinary
snapshot Result plus separate DeadlineEvidence: main-loop deadline_selected,
single decision_send outcome and Missing/Invalid/CheckpointAfterDecision receipt.
It reuses ordinary input/cancel/preflight/normalization/supervision/decode/validate
and immutable snapshot reconstruction. Existing Parser/Reply/Snapshot/error APIs
remain compatible; there is no server caller or new dependency.

An accepting finite conjunction requires same-child/request provenance, original
deadline branch, successful16-byte decision send, exact16-byte ACK plus clean
WouldBlock sentinel, ordinary final Timeout, strict normal healthy followup and
actual reviewed source/original CI evidence. Root accepts that conjunction for
the finite production-worker deadline-boundary component of canonical #83 B3.
No positive runtime witness is claimed in this source-stage document. Actual
reviewed evidence is required before status promotion. Cleanup Io, final non-
Timeout, absent/invalid receipt or wrong identity remain nonaccepting.

This proves only grammar checkpoint execution after the original deadline
decision before termination. It does not prove continuous occupancy, CPU time,
scheduling cause, predicate EOF truth, main-source versus scratch/lazy context,
B11 active-EOF, hard total work/return/cleanup time, RSS or descendants. Setup,
TLS, polling/allocation and the new default EOF TLS presence check can perturb
outcomes; effects are UNMEASURED. Observer failures are not DIRECTLY mapped into
parser errors, but that is not an identical-outcome guarantee.

## Transport and preserved boundaries

One unnamed connected UnixDatagram pair belongs to the invocation. Both endpoints
are nonblocking before spawn; only worker endpoint W becomes child fd2, and the
Command is dropped after spawn so no W alias remains in the parent. Parent P plus
same owned Child and immutable snapshot bind identity. Trusted parent pair creation
establishes datagram type; this is not malicious-executable authentication.

Only the explicit private `--observe-deadline-eof-v1` worker argument enables fd2
setup. Safe stderr AsFd duplication and OwnedFd conversion do not validate type.
Read-only local_addr/unnamed then peer_addr/unnamed checks MUST both pass before
set_nonblocking on the shared open file description. FILE/PIPE/named/unconnected
fallbacks precede mutation. Address-valid unnamed streams can pass; no SO_TYPE
proof or arbitrary socket-type validation is claimed. Setup/TLS failures fall back
to ordinary parsing without direct error remapping. Address-valid socket flags
have no rollback promise if later setup fails.

The vendor doc-hidden parse_file_observing_eof wrapper restores a thread-local
fn() callback on normal/nested/unwind paths. Parser::eof dispatches immediately
before its unchanged builder predicate. Worker callback state is request-local,
fallibly accessed, sampled every1024 EOF calls with wrapping u16 counter. Each
sample makes at most one nonblocking recv into17 bytes. Only exact decision
`KTLSP-DUE-v1!!!!` accepts; WouldBlock keeps it armed, every other error/length/
packet disarms. Disarm precedes the one attempted ACK `KTLSP-EOF-v1!!!!`; sends
never retry. Protocol emission is at most32 bytes/two packets per request.

After existing cleanup/scoped joins ordinarily return, parent receives at most
twice into17 bytes; exact ACK plus WouldBlock sentinel is required, and malformed/
extra/oversized/truncated datagrams invalidate evidence. Diagnostics on fd2 never
enter stdout Reply decoding. Default calls create no sockets and retain null
stderr. No unbounded observation reader, new join or per-event allocation exists.
Kill-before-ACK races remain intentionally inconclusive.

PR195 worker_allocator.rs, allocation latch/accounting, main exit helper, normal
reply serialization and allocation-result priority remain unchanged. Main-loop
cancellation -> original deadline -> output -> status order and join_io/finish_io
deadline policy remain; a post-success-I/O Timeout never sends a decision token.
ChildGuard::finish before scoped joins retains ambiguous Io override and existing
unbounded OS wait/join residual. No allocator, existing protocol/corpus tests,
Cargo/lock/workflow or status files change.

## Finite controls and verification limits

Exactly one observed production source is frozen ASCII44 bytes, no newline:
`val x = (a<(a<(a<(a<(a<(a<(a<(a<(a<(a<(a<(a<`, SHA256
`d69976699f5938c3e7bac8239c06f4a67c157066cb81eebebc6662dca0e6acb2`.
It records the first outcome/evidence directly to stderr so successful CI preserves
it. Ok(validated snapshot), WorkerFailed, Timeout and AllocationLimit are allowed
containment outcomes; all other errors fail. Missing/nonaccepting evidence prints
INCONCLUSIVE/B3-HOLD. No input/depth hunt, repeated observation, synthetic work,
shorter deadline, assertion relaxation or rerun-to-witness is permitted.

The next request is NORMAL snapshot `val healthy=1\n` (14bytes), with exact11
elements/6tokens/0diagnostics, literal kinds/spans/parents/subtree ends, original
token partition, immutable identity/source and stale-revision rejection. Failure
stops admission. Healthy controls prove new-request health, not old-child causation.

Machinery tests separately cover exact/short/oversized/unknown/empty/extra records,
failed/partial decision send classification, worker WouldBlock/cadence/one-shot,
finite synthetic ACK backpressure/peer loss, reentrant-state disabling, observed
pre-cancellation without child/token, and vendor active-thread isolation/nested
normal restoration/unwind restoration. These do not substitute for the production
witness. Existing cancellation-before-deadline and pending-I/O precedence tests
remain. Unavailable TLS teardown, arbitrary socket-error injection and rare OS
cleanup-wait Io remain STATIC: unchanged guard.finish()? precedes the selected
result, and post-cleanup observation does not remap it. No broad fake cleanup/
transport framework or dynamic OS-failure coverage is claimed.

Linux-only FILE and PIPE fallback controls keep the same retained parent initially
blocking descriptor across child completion and compare complete /proc fdinfo
flags before/after. Both require exact healthy replies. Test child polling uses
the existing500ms/2ms policy with kill/wait Drop cleanup and post-exit stdout cap;
this is finite test containment, not an OS hard-reap guarantee. Non-Linux never
executes /proc flag checks; local Darwin Clippy cannot typecheck that gated body.

Unchanged limits: source65,536; raw nesting128; original pre-spawn500ms/2ms poll;
cumulative allocation67,108,864/no refund/full new realloc charge; stdout4,194,304
plus parent sentinel; elements16,384; diagnostics256/messages4096. Caller-managed
admission, trusted direct child and no server consumer remain.

Owner fresh pristine archive -> recovery.patch -> formatting.patch replay matches
all54 final Rust hashes, zero fuzz/offsets:16 behavior/50 formatting files/four
pristine survivors. Independent adversarial replay is separately required; notices,
upstream inventory/archive, manifests and locks are preserved. Only vendor lib.rs
and abstract_kotlin_parsing.rs final bytes change from the assigned base.

Local formatting and compile-only locked all-target workspace Clippy are admitted;
no local parser/worker/server/test/bench/oracle/native runtime runs under #96.
SAME architect/correctness/adversarial review actual source/fixes and completed
original unchanged Linux all-three CI before coordinator expected-head/base merge
lease. Compile success is not runtime acceptance. Merge remains HOLD.

T008/T008a HOLD; broader C13/B3/B4 GAP; B11 UNVERIFIED; #7/#83/#96 OPEN;
Darwin UNKNOWN. No semantic/performance/full-parity or historical tally change.

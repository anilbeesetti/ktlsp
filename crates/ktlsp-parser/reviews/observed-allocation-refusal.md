# Observed cumulative allocation-cap refusal

The worker latches an actual rejected cumulative-cap charge in a private,
never-reset AtomicBool and returns null as before. Rejected alloc/alloc_zeroed/
realloc branches add only that static store; no exit, log, format, lock, heap
access or panic. Successful System forwarding, deallocation, checked cumulative
charging, no refunds and full new realloc-size charging are unchanged.

After run() returns, main uses the shared private worker_exit_code(bool) with
the allocation-free is_ok discriminant query; the helper reads the latch before
selecting ordinary0/1. Latch true selects reserved86 over Ok, Err and an already
written reply. Result is not formatted, boxed or dropped before that read; the
existing std exit remains outside allocator callbacks. The parent maps only
actually observed trusted-worker code86 to public ParserError::AllocationLimit.
Root explicitly admitted the public exhaustive-enum compatibility change and
doc-hidden shared constant; no server consumer is added.

Code86 reports at least one cumulative-cap refusal before the outer latch read,
not parser phase or immediate failure cause. System null does not set the latch;
an earlier independent refusal may still select86 after a later unrelated error.
Infallible allocation can abort before run returns. Refusal during exit cleanup
is after the read, and unsuccessful/unfinished exit can remain ambiguous.

On admitted base b026508e126e64572fd3a4b646fee95304a5768d (PR182), observed86 is
checked only after try_wait(Some(status)) on the owned child, before other failed
status handling. Prior selected cancellation/deadline/received output/Io win;
unread output is not retroactively inspected. PR182 finish_io/join_io ordering
is untouched. Checked finish Io still overrides the saved result. Same-child
successful reaping remains necessary for a normal non-Io post-spawn return;
blocking cleanup/scoped joins still have no hard return-time bound.

The existing harness=false fault executable includes the exact private allocator
implementation but keeps its parent/ordinary allocation on System. Three real
fallible direct callbacks assert null, helper0/1 before refusal, helper86 for
both outcomes after, sticky flag after later successful allocation, and realloc
original initialized byte/ownership preservation. An independently valid full
reply is written before the alloc child's outer86; zeroed selects the error
outcome. All three run through actual Parser::parse_snapshot, retain rejected
source, and require exact healthy production snapshot/source/11-element tree,
names/ranges/parents/tokens/zero diagnostics. Separate bounded raw invocations
assert literal86 and positive wait/try_wait through the same owned Child; they
are not represented as the Parser invocation's raw receipt. Existing infallible
abort helper stays distinct. Synthetic86/87 mapping controls retain earlier
fault/cancellation/output tests. Rare checked-cleanup wait-error precedence is
source-audited, not fabricated OS failure execution.

The unchanged finite production depth8/10/12/32 cohort emits one direct Write
observation per request under original unchanged cargo test. Existing generic
failure allowlists add only AllocationLimit as the prospectively admitted
attributed refusal category; exact known-defect results and all input/cohort
limits remain. Neither injected faults nor typed errors close production B4.
If normal requests do not naturally observe86, that absence remains B4 GAP.

Contract v3 SHA2563c88dd7852a1c07034dc8f810449209c2c335f4b23f94f2bcc68ddcc2929d354
was independently approved by the retained architect/functionality/adversarial
reviewers before explicit root source admission. Historical v2 proposed exit
inside GlobalAlloc; own reviewers initially accepted its owner-cleanup reasoning,
but root's architect disagreed and root rejected the design. That disagreement
and all v2 approvals remain preserved in auxiliary history; this source removes
the disputed path rather than treating it as consensus. Pinned Rust1.99 source
context is commit b940084d7eb6a299eb4bfeb8e34901bc051e7ac4.

Local formatting and compile-only pinned locked workspace Clippy are admitted;
no local product/parser/worker/test/server/bench/oracle/native runtime runs under
unresolved #96. Execution belongs to original unchanged Linux Native Rust CI;
actual check/review identities are reported in the PR, not predicted here.

Source65536, allocation67108864 cumulative requested bytes, nesting128,
output4194304, elements16384, diagnostics256/message4096, deadline500ms and
poll2ms remain. B3/B4 GAP, B11 UNVERIFIED, T008/T008a HOLD, broader C13 GAP,
#7/#83/#96 OPEN and Darwin UNKNOWN. No RSS/stack/phase/CPU/global-concurrency,
hard end-to-end deadline, descendants, semantic provider or full-parity claim.

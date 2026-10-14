# T007 successful-I/O deadline result selection

This change continues checking the original supervisor deadline and cancellation
after successful same-child exit while required I/O thread completion is pending.
The pre-success output/status ordering is unchanged. A shared private `join_io`
checks cancellation, then deadline, then readiness on every iteration; `finish_io`
uses it for writer then reader, preserving writer error, reader panic and remaining
output precedence. The final receiver operation is nonblocking.

The source retains ChildGuard::finish before implicit scoped joins. Successful
same-child reaping is still required before normal post-spawn return. Slow cleanup
does not invent Io; an actual wait error retains the existing ambiguous Io override.
is_finished can precede complete thread termination, and cleanup/joins still have
no hard return-time bound. An unjoined scoped-thread panic can still prevent a
selected error from becoming an ordinary return.

The independently frozen contract is auxiliary SHA256
`b5e9d9310856eda0e17feb0a629528bf54d959b1ed06c3072c28785a33825ec3`,
based on source941c72efd9896b0847ab342ec0dd22c10a000f3b. Root explicitly accepted
shared production readiness-loop tests plus static successful-reap and sequential
stage wiring as the evidence boundary. Gated scoped-thread tests cover pending
writer/reader result types, initial cancellation/expiry, future-deadline expiry,
output receipt before reader completion, errors/panics/missing output, buffered
and fresh success, and cancellation-before-expiry-before-ready precedence.
Finite controller escape releases all gates even if a faulty join blocks selection.
No test claims dynamic post-writer reader-stage expiry, delayed real-pipe completion
inside supervise or an exact poll count. Existing actual-child fault/protocol and
normal snapshot controls remain separate integration evidence.

At source preparation, no local runtime was executed under unresolved #96.
Formatting and compile-only Clippy are locally admitted; runtime validation belongs
to the original unchanged Linux Native Rust CI all-three commands. CI results and
exact reviewed identities are recorded in the PR rather than predicted here.
Historical parser README/RESOURCE_EVIDENCE/status cutoff claims are not rewritten
by this two-path child; they remain historical evidence, with this review note
describing the new finite correction.

This is deadline result selection only. B3 attribution/B4 remain GAP, B11
UNVERIFIED, T008/T008a HOLD, #7/#83/#96 OPEN and Darwin cause UNKNOWN. No deferred
cleanup, descendant lifecycle, semantic provider, memory/RSS, hard return-time or
full-parity claim follows. Worker/vendor/public API/protocol/dependencies and
all numeric resource limits are unchanged.

# #62 review evidence (2026-10-12)

Scope: parser adoption safety gate only; parent T007/#7 remains open. No server
caller/capability or full grammar/recovery/official parity claim. Read README for
64 KiB/500 ms/conservative delimiter/cumulative-allocation limits and trusted
worker/no-descendant assumptions. Original source remains authoritative.

Coordinator allocated two independent slots and approved exact ktrs-parser and
ktrs-syntax 0.5.1 with ktrs-lexer 0.5.1/text-size 1.1.1. Archive checksums match the
locked graph; upstream dev/facade/native/JVM dependencies are excluded. Full
upstream notices/licenses retained. Initial architect review required precise
heap-vs-RSS wording, supervision of input/output plus cleanup, tree coverage and
normalization checks; implementation addresses these without an RSS claim.

## Independent functionality review

No actionable findings on production implementation dc2131c6e1928c733697db7fab1fde6b6e03ad14.
Reviewer ran parser tests and an independently compiled Rust consumer with nine
multiline/declaration/malformed/Unicode fixtures in Source and Script modes (18
parses). Verified original source/token partition, UTF-8 and UTF-16-valid element
and diagnostic boundaries, parent containment/preorder, diagnostic message
pairing with ERROR_ELEMENT, and healthy parsing after malformed cases. Inspected
normal/build graph and additive lock changes. Issue network access was unavailable
to reviewer; crate README and assigned contract were reviewed instead.

## Independent adversarial review

No blocking runtime findings on the same production implementation. Independent
Rust probes covered root overflow, token coverage mismatch, 4 MiB+1 output, six
actual timeout cycles, 20 midflight cancellations, and 14 real-worker cases
including empty/CR/CRLF/NUL/astral/malformed template/64 KiB repetition inputs.
Separate decoder probes covered exact MAX/MAX+1 element/diagnostic counts,
truncated JSON, unknown variants, missing fields and trailing junk. Reviewed
allocator checked cumulative accounting and upstream TreeBuilder reserve/unsafe
push paths; audited approved normal/build graph.

Recommended permanent tests for truncated/unknown JSON, decoder count limits,
immediate-parent span/subtree containment. Added these plus a real-worker
output-count rejection and healthy follow-up. Reviewer rechecked those additions.
No production logic changed after initial independent approval. OS-wide process
census was sandbox-denied (`ps`); no claim of independent process-list verification.
Scoped ownership/kill+wait and repeated timeout/cancel probes provide cleanup
evidence under the documented no-descendant worker assumption.

## Finished architect audit

No blocking findings; independently reran the 13-test parser suite. Confirmed
child guard drops before scoped joins on error and trusted no-descendant worker
makes post-exit joins finite absent OS stalls. Source mapping, structural checks
and cumulative allocation accounting match limited documented guarantees.
Removed an overbroad comment implying run() never drops third-party parse state;
locals are dropped before run() returns, and process isolation contains failures.
No production logic changed.

## Native checks and limits

Pinned fmt, locked workspace clippy with warnings denied, and locked workspace
tests pass. Final workspace: 89 active tests; two ignored helper fixtures are
intentionally launched by active subprocess tests. Parser: 13 active tests.
Release parser suite also passes. Generated parser corpus has 64 deterministic
UTF-8 fixtures, separately from upstream-regression depth 8/10/12/32 cases.

No JVM/JDK/Gradle/Maven/official oracle executed; no full Kotlin grammar corpus,
semantic/declaration HIR, revision identity, provider, hard RSS, general malicious
executable sandbox, aggregate concurrent-worker admission or packaging validation.
Hosted CI and exact final head are recorded on the PR; local tests are not hosted
CI. This gate alone does not unlock T008. Next: T007b accepted declaration corpus
and validated parser-result/recovery contract, then accepted T008a HIR identity.

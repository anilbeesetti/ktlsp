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
tests pass. Initial reviewed workspace: 89 active tests; later integration counts
are recorded on the PR. Two ignored helper fixtures are
intentionally launched by active subprocess tests. Parser: 13 active tests.
Release parser suite also passes. Generated parser corpus has 64 deterministic
UTF-8 fixtures, separately from upstream-regression depth 8/10/12/32 cases.

No JVM/JDK/Gradle/Maven/official oracle executed; no full Kotlin grammar corpus,
semantic/declaration HIR, revision identity, provider, hard RSS, general malicious
executable sandbox, aggregate concurrent-worker admission or packaging validation.
Hosted CI and exact final head are recorded on the PR; local tests are not hosted
CI. This gate alone does not unlock T008. Next: T007b accepted declaration corpus
and validated parser-result/recovery contract, then accepted T008a HIR identity.

# #86 revisioned result and declaration corpus review (2026-10-12)

Partial child of #83/#7. Architect reproduced missing-parameter-list recovery
failures and recommended splitting result/corpus validation from coordinated
upstream recovery repair. CORPUS.md records all four known recovery counterexamples
and individual unaccepted grammar areas. #83/T008a remain gated.

Two independent slots were allocated by coordinator. Functionality reviewer
passed the parser suite and independently exercised 22 Kotlin source cases plus
script through public parse_snapshot: KDoc/file annotations, constructors, enum,
companion/anonymous object, delegation/destructuring, string templates/raw strings,
generic constraints, default lambda, typealias, Unicode CRLF, and incomplete
declarations. These probes verify structural acceptance only; they do not expand
the named accepted declaration corpus or establish semantic/version parity.

Adversarial reviewer found P2: parent depth validation missed tokens/childless
nodes at depth129. Fixed the shared validator to check every element's depth.
Both reviewers independently rechecked: separate public-API probes for token and
empty-node chains at 2/3/127/128/129/130 accepted through128 and rejected129/130.
Permanent custom protocol cases assert both exact boundaries. Thirteen additional
independent malformed replies (cycles/overflow/subtree/range/token gaps/overlaps/
diagnostics/UTF-8/trailing JSON), each followed by healthy parsing, also passed.
Original LF/loneCR/CRLF malformed Unicode EOF diagnostic mapping was rechecked.

Inherited abort fixture could yield Timeout instead of WorkerFailed because the
supervisor checks deadline before exit status. Architect approved accepting only
these two failures for abort. Added normal process::exit(1) fixtures that strictly
require WorkerFailed; all other expected errors/healthy results remain strict.
Both independent reviewers rechecked this test-only correction using stable
already-built executables. Production deadline/exit ordering did not change.

One healthy real-worker followup returned unexpected Io during overlapping Cargo
and reviewer jobs. Io still fails the test. Three sequential stable-artifact
custom suite runs and independent reviewer runs did not reproduce it. Concurrent
relinking is a possible cause, not proven; no retry-on-Io production behavior or
relaxed healthy-success assertion was added.

Pinned native checks pass on the initial base09b77a3: fmt, locked workspace clippy
with warnings denied, full workspace tests (136 active +2 exercised helper
fixtures), plus assert-based custom protocol suite (19 fault cases, exact depth
boundaries, cancellation and real-worker healthy followups). Final head, hosted CI
and integration evidence are recorded on the PR. No dependency/lock/limit change,
JVM/Gradle/Maven/official oracle, full grammar, semantic HIR/provider, hard RSS or
packaging claim. Allocation abort remains generic WorkerFailed, possibly Timeout;
source snapshots require caller-managed document incarnation and publication.

Finished architect audit of production90c142c found no blocking findings for #86.
Reviewed complete diff and corpus/protocol expectations; direct already-built
executables passed8 declaration tests and19 protocol faults with boundary/cancel/
healthy-followup checks. Verified explicit name offsets, original-source snapshot
association, all-element depth and child/diagnostic validation. Parent recovery
and resource gates remain open; local pinned/hosted/integration checks are owner
evidence, not independently certified by this audit.

# #87 parameter-boundary recovery integration (development)

Coordinator handed ownership over after #86/PR92 merged. Product now selects the
minimal source-local parser fork; lock delta only removes its registry source and
checksum, while explicitly approved workspace exclusion keeps upstream dev/test
packages out of default native tests. Original archive checksum, all54 Rust source
hashes, seven-file recovery.patch and complete notices are retained in vendor.
No supervisor, common API, source mapping, kind validator or resource limit changed.

Source-local implementation and independent reviews are recorded in vendor/REVIEW.
Both reviewers found/rechecked a fixed exponential speculative annotation-probe
regression; a scoped flag suppresses only recursive speculative probes. Committed
nested parsing remains recovery-enabled. Remaining superlinear work is bounded
by unchanged worker resource gates, not claimed as equivalent upstream performance.

Production tests use the immutable snapshot API and real supervised worker. Exact
checks replace three parameter-defect assertions; generic defect and Coverage
remain. Read-only functionality review caught two new fixture errors before
execution: repeated unmatched parentheses above128 would trigger preflight instead
of diagnostics, and object declarations are wrapped by OBJECT_LITERAL. Corrected
repeated recovery to100 and used balanced raw delimiters for diagnostic overflow;
corrected immediate object parent. No production limits/expectations relaxed.

Preparation integrated exact main583a0adc32c79c1a68257855c4ee7432f0088796
once at merge8d833c12. Both independent source-only integration reviews approve
the preserved corpus, strict assertions, authorized graph and provenance.

Targeted production checks and both behavioral integration rechecks await the
coordinator's local-check slot. Full-workspace checks, stable integration, hosted
CI and architect finished review remain pending. No JVM/oracle/Gradle/Maven ran.

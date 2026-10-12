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
A separate pinned-rustfmt formatting.patch transforms that behavioral source into
the final 54-file formatted-sha256 inventory; 49 files have formatting changes
(including the seven behavioral files), with no formatting opt-out.
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

First actual production-graph checks on cc4920d passed: locked offline metadata
(nine workspace members, vendor excluded) and normal/build/all-feature parser
inventory; pinned workspace clippy; parser tests (28 active + two invoked helper
fixtures and 19 protocol faults). Functionality reviewer independently passed
21 integration tests +19 protocol faults and 219 fixtures/438 worker parses with
no unexplained lifecycle failure. These are observations on the behavioral source,
not approval of the later formatting phase. Initial root fmt check found upstream
style differences because --all traverses excluded local path dependencies.
A temporary untracked formatting-disable experiment was removed; the architect
approved a separate formatting-only phase instead. Both final behavioral reviews,
full-workspace checks, final formatting and exact-head hosted CI remain pending.

Both independent formatting semantic-preservation reviews approve the separate
phase. They compared all 54 files to cc4920d: 49 changed and five pristine, with
identifiers, literals and comments preserved. They inspected 147 added trailing
commas and 17 other token edit hunks: one Layer re-export ordering change,
diverging-return match-arm/let-else semicolons and block/closure reflow. No changed
control/evaluation/drop behavior, macro-token grammar or location/stringification
hazard was found. Functionality review independently applied both ordered patches
and matched all 54 final hashes. These reviews ran no Cargo/build/tests/probes.
Final behavior must still be rechecked after coordinator-approved #89 integration.

## Controlled final production validation

Exact main758e53b17566e4fde908705fc090d516ba4b0065 (#89) integrated once at
b3bdc97cd21c181582a31fdf541f3f3ca7c880d9. Both source-only integration reviews
confirm the fork, inventories, strict corpus and approved three-file graph delta
are unchanged from the reviewed formatting phase. Local pinned fmt and locked
workspace Clippy pass on the integrated implementation; compiler output explicitly
checks vendor/ktrs-parser-0.5.1. Locked metadata confirms nine workspace members,
no vendor enrollment and the direct parser path. Normal/build/all-feature trees
contain the existing pure Rust closure only.

The final file audit verifies archive SHA256, all 54 pristine source hashes,
unchanged licenses/notices, the seven-file behavioral patch, the 49-file separate
formatting patch and all 54 final hashes. Applying both ordered patches exactly
reproduces the final source set and bytes. No formatting opt-out is present.

The coordinator authorized existing Linux Native Rust CI as the controlled
executor of all three unchanged pinned commands on the exact published head.
Final CI and both behavioral reviews/finished architect evidence are recorded
on PR90; earlier source-local/pre-format/old-head passes are historical only.
No new Darwin parser execution was used for a passing result. Reported Darwin
supervisor timeout reliability remains issue #96 OPEN/UNKNOWN, with its cause
unresolved. The 500 ms deadline and all failure assertions remain unchanged.
No JVM/oracle ran. #7/#83/T008 acceptance remains unfinished, with generic,
missing-default/deep-type/annotation recovery and Coverage gaps retained.

## #100 controlled classifier-generic recovery review

The candidate integrates prerequisite main73c03df once. Local pinned format and
locked workspace Clippy pass; standalone vendor test sources compile without
execution. The unchanged pure Rust graph and ordered archive/54-original-hash ->
nine-file behavioral patch -> 49-file formatting patch -> 54-final-hash replay
were verified. The adversarial reviewer independently replayed both artifacts
in memory and confirmed exact bytes; notices/original inventory are unchanged.

First controlled Linux head86c0b97, run38285596221/job114933082651: format and
Clippy passed, 15/16 declarations tests passed, but an incorrect negative assertion
for an incomplete annotation rejected a legitimate existing nested FUN. Both
reviewers independently traced annotation/value-argument parsing. Corrected
assertions require VALUE_ARGUMENT parent, exact nested name/function and damaged
class/list ranges, and forbid a KT_FILE sibling. No production fix was indicated,
no unchanged-head retry was used, and later recovery-limit tests were not reached.
Corrected candidate054e352e1903250f44b5e3a418a7b1719f004844 passed Linux
run38285975233/job114934324677. Actions tested synthetic merge
86bbd04fe50de7e28d68fb34dd6ce3d3cec73eec, tree
ad10ce11498b068785296334144365325649dae4, with parents
b7ba65b849c2e1be681c88bd7bba7dc66a14f78d and candidate054e352.
This is actual combined-tree evidence, not a claim that CI checked the bare head.
All three unchanged pinned commands passed: 169 active tests, zero failures,
two ignored helper fixtures exercised by parent tests and 19 worker fault cases.
All 16 declarations and five recovery-limit tests passed. Both independent
reviewers inspected these logs and approve bounded functionality/resource behavior;
production source matches the independently replayed86c0b97 source. Leading trivia
scans are input-bounded and monotonic; the aggregate budget covers copied tokens.

Stable integration, final-head behavioral rechecks/CI and coordinator expected-head
merge grant remain required. No Darwin runtime or JVM/oracle execution occurred;
issue #96 remains unresolved.

## Focused accounting review and explicit contract amendment

Both independent reviewers found that leading-trivia scans and cheap
kind/modifier/EOF misses are uncharged, despite the original broader accounting
reading. Their earlier candidate behavior approvals did not establish that
broader contract. The coordinator explicitly amended it after independent
architect review: at most 4096 copied scratch tokens per classifier list, counting
internal/trailing copied trivia and charging every eligible window before grammar
success/failure; a zero-copy eligible rejection debits one. Leading scans and
cheap prefilter misses may remain outside the allowance. The 256 copied-token/
4096-source-byte attempt caps, minimum active EOF and recursion guard are retained.

The leading scan increments a raw index until nontrivia or lexeme end. Every list
backedge requires and consumes a comma; ordinary token queries normalize skipped
trivia before continuing. Scratch recursion returns before this scan. Together
with the unchanged 64 KiB input limit, this is a local finite-progress argument,
not proof of global linear parsing, total work or RSS. Existing internal-trivia
threshold and failed-eligible aggregate exhaustion/reset fixtures remain relevant;
they do not establish a precise aggregate acceptance threshold.

The focused production-worker fixture places 4401 leading trivia tokens
(13204 bytes) directly after '<', then pins exact damaged class/list and sibling
name/function/parent ranges, entry diagnostic, token partition, snapshot identity
and healthy followup. Format and workspace test-source compilation pass; no
runtime execution has occurred for this addition. A public active-truncation
control alone would not prove negative clipping. Root explicitly authorized a
private const predicate extraction in the owned type-parameter module and accepts
that narrow guard evidence. The actual window loop uses it unchanged; unconditional
asserts cover EOF before/at/inside a token, exact endpoint, absent EOF and independent
4096/4097-byte bounds. The minimum active EOF reaches this real guard; zero copied
tokens return false before scratch construction, and a prefix cut before its
required FUN/name/LPAR cannot pass. This is window-predicate evidence, not annotation
parsing, active-layer integration or end-to-end truncated recovery. That limitation
remains for #83; it does not admit T008 or broader acceptance.

Ordered archive -> nine-file behavior -> 49-file formatting -> all54 final source
hashes replay passed after extraction. Pinned format, locked workspace test-source
compilation and Clippy pass without local runtime. The new worker regression still
awaits final integrated Linux execution; separate resource/provenance and finished
architect reviews remain required.

Source-only correctness review caught an ordinary comment-binding fixture issue
before execution: same-line block comments would bind to the damaged class. The
fixture now brackets the comment/space run with blank lines, independently keeping
class/list and bare sibling boundaries fixed. The leading scan still starts
immediately after '<' and crosses the whole run before candidate copying.

Both independent reviewers approve source preparation after rechecking the
corrected blank-line fixture: 4401 raw trivia tokens/13204 bytes, FUN13214/name
13218..13225, class0..10/list9..10 and entry diagnostic10. Adversarial review
independently replayed the updated archive/ordered patches to all54 final source
bytes; correctness checked the final hashes and actual guard call path. They
withhold final behavior/resource approval until integrated Linux execution.

Owner architect approves this focused preparation: actual-predicate preservation,
large-leading-trivia fixture reach and honest accounting/EOF limitations match
the root-amended contract. This is conditional preparation approval, not delivery
or renewed runtime approval. Final integrated Linux checks and both reviews remain.

## Authorized final integration

Exact main ef120beebb45f96a60bf61866fe15dd228b674bc, tree
f657f81e16237c1ba0903169ab94fd3e2747eafe, was integrated once into focused
preparation f7d56979539cdda7c4216733fd6647bf75ef782f at merge
78e9df525145df31a8dd6ef2b56d31dac53ce7af. The integration changed no owned
parser/vendor source or test bytes and preserves current-main binary exports,
shared docs, manifests and lock exactly. Ordered archive/patch/final-hash replay
passed again. The authoritative combined-head Linux workflow, new leading-trivia
runtime outcome and renewed separate reviews/finished architect remain pending.
Root expected-head merge authorization and exact merged-main checks are still
required. No local runtime or JVM/oracle ran; #7/#83/#96 remain open.

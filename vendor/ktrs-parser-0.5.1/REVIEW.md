# Historical source-local review evidence — #87

This records the source-local phase before #86 delivery and coordinator handoff.
The handoff is now complete; production checks/reviews/CI remain required. No JVM/oracle/Gradle/Maven or forbidden native dependency ran. Exact
source patch is recovery.patch; upstream-sha256.json fingerprints pristine source.

Architect plan review identified shared-loop contexts, terminal recovery consuming
siblings, annotation/modifier prefixes, token remaps and nested grammar. Explicit
function-list context, parameter-boundary checks and non-consuming diagnostics
address the original three counterexamples. Constructor/initializer boundaries
use guarded lookahead, preserving soft-keyword parameter names.

Independent functionality review ran 210 valid structural comparisons against
registry upstream, including 200 soft-keyword/annotation combinations. Independent
adversarial review ran 60 valid structural comparisons, 210 malformed cases and
1,000 repeated recovery points with original ranges/nearest parents/token partition.
Both found a P1 exponential modifier-probe replay on nested annotation anonymous
functions. Fixed with scoped probe suppression; both rechecked the exact cases,
valid tree equality and nested malformed recovery. No remaining source-local
blocker reported; both require production integration recheck.

Six source-local tests pass (cargo test --offline --locked --manifest-path
vendor/ktrs-parser-0.5.1/Cargo.toml --test recovery), checking original malformed
function boundaries, later names/kinds/nearest parents, diagnostics, annotation
prefixes, repeated recovery, constructor/initializer recovery and valid defaults.
EOF retains exact upstream diagnostics. The depth-20 annotation regression finishes without recursive branching; production
acceptance is constrained by unchanged 128 tree-depth limit.

Read-only production-source experiment: standalone /private/tmp/ktlsp-t007b2-worker
manifest (now preserved portably as worker-probe/Cargo.toml) uses ktlsp-parser path dependency, the unchanged production worker source
as its binary path, and a local crates.io parser patch. No production manifest or
lock changes. Two worker unit checks plus six existing parser integration checks
and two original test probes pass. Worker allocation-denial child was invoked by
its parent test; existing generated/crash/backtracking/coverage/limits/healthy-
followup gates passed. Original source recovery passed nine top/member/local x
LF/CRLF/lone-CR combinations with Unicode/backticks. Nested annotation depths
8/12/14/16 accepted at ~10/16/19/26ms on this busy macOS debug host; depths
20/30/40/50 explicitly returned NestingLimit at ~30/84/178/332ms, each followed by
a healthy parse. These are observed local checks, not a performance guarantee,
final integrated #86 API evidence, hosted CI, or product-graph delivery.

Residual recovery examples and version/full-grammar limits are in PROVENANCE.md.
Architect finished review approves the source-local phase/draft PR. Root fmt and
clippy pass on the unchanged product graph; full-workspace tests initially hit
sandbox-denied mkfifo in artifact tests, then passed with actual escalation:
137 active tests plus two invoked helper subprocess fixtures. Hosted Native Rust
CI passed source-local draft head 2534c83e783d8b4e4b2ea2b688a751c5e51c05cc:
https://github.com/anilbeesetti/ktlsp/actions/runs/38257790041/job/114840244988
The hosted root checks do not execute either standalone vendor test workspace.
Production pinned full-workspace checks, hosted CI and both integration
rechecks and expected-head coordinator merge grant remain pending.

## #100 review continuation

The new classifier recovery uses isolated bounded scratch parsing and inspects
specific completed type productions, preserving ordinary non-class behavior.
Both independent reviewers supplied production regressions for annotation/operator
state preservation, incomplete bounds, excluded headers, entry deduplication,
exact raw-token/byte thresholds, repeated/aggregate exhaustion and recursion
suppression. Adversarial review independently reproduced all54 final source bytes
from the archive and both ordered patches, with the unchanged Rust dependency
graph. Production exact-head Linux execution is the behavioral authority; the
standalone recovery tests compile locally but are not run by root workspace CI.
The first Linux failure was a new test's blanket nested-FUN prohibition; corrected
parent/range assertions passed on candidate054e352 in Linux
run38285975233/job114934324677, actual synthetic checkout86bbd04/treead10ce11
(full identities in the production parser REVIEW.md). Both independent reviewers
approve after inspecting all169 active passes/zero failures, two exercised helper
fixtures and19 worker fault cases. Final integrated-head rechecks remain required.

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

## C13b / #136 finite source preparation (not delivered behavior)

The source-authored contract was approved by the SAME correctness/functionality
and adversarial reviewers and owner architect at frozen37bfe388/tree8ba503ba.
Root then admitted finite source and exact issue publication only. The private
optional-DOT expression preserves old no-dot short circuit, existing caller gates,
completed-bound check, copied-window accounting and scratch suppression/state
isolation. The broader recovery exclusions remain. The single String-extension
negative member is deliberately superseded by exact positive assertions.

The new focused test includes a literal selected-field copy of the reviewed final
70-seed/38-projection contract rather than recalculating expectations from parser
output or reusing the obsolete intermediate generator. Complete selected sequences
include expression-body this references, exact declaration names/nearest owners,
receiver/list/parameter hierarchy and ordered diagnostics. Compatibility-only cases
remain explicitly limited. A corrected source trace proves that single-token
suspend reaches USER_TYPE because DOT-truncated lookahead prevents modifier
consumption; no missing-type or semantic validity inference remains.

Owner data-only replay passed from the original checksummed archive through the
ordered recovery/formatting patches to all54 final source hashes. Counts are14
behavior/50 formatting/four pristine survivors; only type_parameters.rs final Rust
bytes differ from the source base. Details, exact fixture/artifact hashes and
compile-only/source-review receipts are in
../../crates/ktlsp-parser/reviews/c13b-generic-extension-recovery.md.
No local runtime test occurred. Source approval is preparation only; future
coordinator integration, exact Linux all-three/completed CI review, guarded merge
and owner/root main verification remain required. No current delivery/parity or
Darwin96 correction is asserted; #83/#7 stay open and T008/T008a remain held.

# C13 single nullable extension receiver — held source

This is finite source preparation against6365dcdf58ecd58b0a4715eba10d60307edddb15,
tree6fc6ddcb50587d2ac76fc2e3382ef6cd02786edb,
soleparent058b002010bf49585e09c2dbc9e9d559a076f9c9. Coordinator admitted the actual
all-three-reviewed contractv3 manifest
`f3c27baf01e3f6dac69b0dfb4dd38f535f5d74ccd98cd6cf7efb842b47b99066`.
Source/CI/merge gates are distinct; local runtime remains forbidden under96.

## Source and compatibility

Only vendor `src/parsing/declarations/type_parameters.rs` changes final Rust
bytes. Scratch default joining exposes adjacent rawQUEST+DOT as SAFE_ACCESS;
separate SAFE_ACCESS and QUEST(requiredDOT&&IDENTIFIER) arms occur only after the
first identifier. The existing plain/qualifiedDOT arm, finalLPAR, completed-bound/
error-free eligibility, classifier !object callsite, recursion suppression and
isolated builder/drop remain. A consumed QUEST cannot fall through when DOT is
missing. No live cursor/remap/event/diagnostic/newline/join/truncation mutation,
type/function callback, keyword policy or resource change is introduced.
Normal function parsing already disables joining and builds
TYPE_REFERENCE→NULLABLE_TYPE→USER_TYPE→REFERENCE_EXPRESSION; QUEST is under
NULLABLE_TYPE, separatorDOT underFUN. Whitespace/comments betweenQUEST/DOT are
handled as separate tokens; Elvis and qualified-nullable remain excluded.

The new `recovery_nullable_extension_functions.rs` embeds selected-field literal
contractv3 data SHA256
`73b12ce096beac2b65ff08f4053d95fbd031e5fc4f66932352ba039a88059e58`:
32 exact selected positive projections includingNULLABLE_TYPE,53 finite controls,
491 original UTF16 expectations. Complete named zero-parameter positives cover
class/interface LF/CRLF/loneCR and KT_FILE/CLASS_BODY/lazyBLOCK, malformed lists,
annotated function prefix, intact headers, Unicode backtick function name,
spaced/commentQUEST/DOT, lazy recovered body, duplicates and window edges.
Negative controls cover safe-call expressions/Elvis/question/partial/anonymous/
Unicode/incomplete-bound/excluded families. Partial header reachingLPAR asserts
header-only recovery with downstream errors, not a complete-function recognizer.

Only legacy decline-header-10 code dispatch changes: exact class[0,11),FUN[12,37),
namehealthy[24,31),TYPE_REFERENCE/NULLABLE_TYPE[16,23),USER_TYPE[16,22),QUEST[22,23),
DOT[23,24),parameters[31,33),sole diagnostic[11,11)Missing'>'. The old projection
helper still omitsNULLABLE_TYPE, so that node and ancestry/tokens are separately
asserted. Historical70 literal JSON bytes and every other assertion remain;
prior decline-header-8 is preserved. Qualified59 file is byte-identical. No other
parser test family, API/worker/server/Cargo/dependency/lock/rootstatus/workflow path
is changed. New fixtures check exact token partition/reconstruction, complete
selected names/diagnostics/parents/owners, positions/invalid boundaries, immutable
snapshot identity, stale revision and mismatched id/source/kind, exact same-Parser
healthy followup, and unchanged complete original elements/diagnostics/source.

## Bounds and provenance

Scratch256 raw-token/4096-byte windows and shared4096 copied-token accounting,
minimum activeEOF, failed eligible/internal-trailing-trivia debit and zero-copy debit1 remain; leading
trivia/cheap misses are uncharged as before. LPAR256/257 and byteend4096/4097,
QUEST256 with excludedDOT257, QUESTend4096 with excludedDOTend4097, first16x256
aggregate exhaustion/fresh reset and4401 leading-trivia controls are frozen.
These are not total-work/linearity/RSS/stack or active-EOF end-to-end proofs.
Global64KiB/500ms/64MiB requested allocation/4MiB output/16384elements/256diagnostics/
depth128 remain; requested allocation is not RSS/stack, deadline is scheduling
dependent. B11 remains UNVERIFIED.

Pristine archive SHA256
`ec8d43d9549490689477385c7678bbd22ecbc445973ab364e8ecb69dd4882bcc`.
Owner independently extracted archive, verified54 upstream hashes, rebuilt both
ordered phases, then independently extracted archive AGAIN and replayed both
patches. All54 final hashes match with zero offsets/fuzz; rederived14 behavioral,
50 formatting,four pristine survivors. Original archive/upstream inventory,
minimal vendor manifests/locks and audited notices stay byte-identical.

- recovery.patch `b34f04722153a2e8373b0c52b579089b9bf904f3d332d430faf5876f10b80d46`
- formatting.patch `658abef3409def450c57fc6c46800654df52ec543b839f089a70f54baa20246b`
- formatted-sha256.json `024d3883aefa6e4b80963413d35bc92020f0fdb34867d3b6ec4dd028ad4f8fd1`

Adversarial reviewer must independently extract the original archive and replay
both phases, verify54hashes/zero fuzz/offsets/notices and derive counts. No borrowed
receipt. Actual held-source SAME2+architect approvals and independent receipts
are stored externally against immutable head/tree to avoid self-referential hashes.
Contractv1/v2 history retains the corrected auxiliary shared lazy-owner defect
and exact proposed patch hunk correction; product source did not cause them.

## Verification and gates

Pinned Rust1.99.0 `cargo fmt --all -- --check` and compile-only
`cargo clippy --offline --locked --workspace --all-targets -- -D warnings` passed.
No local native/parser/worker/server/tests/bench/JVM/oracle/Gradle/Maven runtime ran;
compile success is not behavior evidence. The ordinary CI commands/workflow are
unchanged. Issue is unpublished at this held-source checkpoint; publication of
one reviewed finite child requires actual3source approvals. Parent7/83 stayOPEN.
Final integration/push/PR/CI/merge are HOLD until one root base after ZIP158 actual
squash; merge also waits ZIP158 owner+root exact-main clear. Subsequent combined
source reviews, original unchanged Linux all3, completed-CI reviews, expected-head
merge and independent owner/root exact-main verification remain required. No
new executor/retry-to-green/deadline relaxation or protection bypass.

BroaderC13/B3/B4 GAP, B11 UNVERIFIED, T008/T008a HOLD, #7/#83/#96 OPEN, Darwin96
UNKNOWN and historical45/32/10/3 unchanged. No HIR/semantic/provider/full official
parity/RSS/official performance or Darwin correction claim.

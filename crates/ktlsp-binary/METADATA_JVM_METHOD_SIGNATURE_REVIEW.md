# Selected JVM method-signature contract review — READY

Stage CONTRACT/TEMPDATA only. No product/source/export/test implementation,
compiler/native/parser/worker/server/bench/JVM/oracle, issue publication, push/PR/CI.
Exact frozen base0a0af012d7a5a258357b5a53407a31b2359bc09f,
tree20cb2cf3f9f1a5daace6c082dc050278dcd3a15f,
soleparenta13ab6ba4100f7c26377a47ab5af0858b6491a48.
Managed worktree /Users/anil/.codex/worktrees/t018-jvm-method-signature/ktlsp,
branch codex/t018-jvm-method-signature-contract. Initial git status clean; base
unchanged throughout. Review slots explicitly granted by coordinator.

Retained SAME3:
- /root/architect (architect role, plan/repeated-source-location-error/finished work)
- /root/correctness (correctness/functionality)
- /root/adversarial (edge cases/crashes)

Architect plan approved with presence/merge/budget/span/full-wire/fixture cautions.
Actual correctness/adversarial/architect draft reviews found nonexistent
classfile::decode in intended composition; corrected to frozen parse_class,
ClassFile::utf16 and explicit wire::decode(&decoded_d1,..). Correctness requested
literal P1 onebelow offsets; fixed. Architect requested hardcap probes and later
wrong-wire scalar-preserves-earlier case; added bytesMAX+1/65536empty-message
record probes and V19. Added inherited nonminimal/low32/tenbyte cases V20..V22.
Reviewers verified merge/defaults/arithmetic/helpers/source hashes/genuine Pair
provenance; no source/runtime exercised. Actual revision2 rechecks: all SAME3 PASS / READY, no remaining actionable
findings. Contract-only approval; no source/runtime authorization.

Data checks actually run: fixture-specific Python CP/annotation/method byte
transcription with assertions (not product decoder); unchanged Pair hash, mv/k,
d1/Constructor/signature slices, d2[7/8], unique encoded MemberCP8/9, exact string
spans. Source SHA256 checks; git cleanbase/status and read-only ticket searches.
Actual delivered Rust resolver/composition remains FUTURE hosted-CI obligation.
No test success, source readiness, performance, initialization or parity claim.

Preserved lookup failures: web raw cache miss for unsupported guessed Kotlin
GeneratedMessageLite path; sandbox curl DNS failure; permitted exact guessed path
returned404. Architect identified existing protobuf pin/path; one corrected
immutable fetch succeeded with SHA25614c63c0c074008f24dcacf101ee09b19691bfaacbfc7422b7ddf9c2aa9bc964f.
Sandbox gh query connection failure, permitted read-only query succeeded; broad
JVM method signature returned parent/unrelated infrastructure only, exact
JvmMethodSignature returned[]. No issue posted. Workflow guessci.yml missing;
actual native.yml read unchanged. These are data lookup failures, not product/
CI/runtime failures, and were not hidden or rerun to claim product success.

No source grant received. Source/publication/push/CI/integration held. SAME3 must
review later source+actual completed original Linux CI; exclusive root lease and
separate read-only fresh guard precede ordinary expected-head merge. #18/#68OPEN;
T008/T008aHOLD; broaderC13/B3/B4GAP;fullB11UNVERIFIED;#7/#83/#96OPEN,DarwinUNKNOWN.

## Actual retained final verdicts (verbatim summaries)

Architect /root/architect:
“Revision 2 passes architect review for contract freeze. All three findings are
corrected, and added scalar, prefix, budget, and hard-cap expectations are
consistent with the frozen helpers. No further actionable findings. Final READY
still requires the other retained reviews and recorded manifest. Source and
runtime validation remain unperformed; this verdict does not authorize
implementation or execution.”

Correctness /root/correctness:
“Correctness contract review: READY, revision 2. Both findings are fixed; merge
semantics, budget arithmetic, literal errors, and Pair fixture linkage are
consistent with the frozen APIs. No source changes or runtime/tests executed.
Rust composition and CI remain unverified; reviewer retained for later rechecks.”

Adversarial /root/adversarial:
“Adversarial contract review: PASS / READY. The API naming finding is fixed. Merge
behavior, signed indices, malformed/prefix offsets, aggregate budgets, hard-cap
arithmetic, and genuine fixture evidence check out. No source, tests, compiler,
JVM, oracle, or extraction script executed. Runtime composition and dynamic
work/owned hard ceilings remain unverified; retained for later review.”

Source edits and runtime remain held. Package MANIFEST and BASE_TRACKED_FILES
record SHA256 of all contract/evidence inputs and every frozen-base tracked file;
clean-base Git tree/soleparent recorded above. The worktree contains only owned
untracked contract/data files; tracked diff and cached diff are empty.

## Admitted source stage

Root explicitly admitted source/API/compile-only/publication/original Linux CI for
reviewed revision2. Exact assigned initial source basefa26a992a54f5a7c81dc34fe1fc51ba3253e8e54
(tree3e597c898389aead6688c79157296de770d24dcc,soleparent0a0af) integrated ONCE;
incoming Type.Argument and all other exports preserved. Exact13newfiles plus
additive metadata/mod.rs export only. No shared helpers/Cargo/lock/workflow/vendor/
parser/server/public-status changes. Reader/test source now exists; preparation
statements above are historical. All frozen fixture EXPECTATIONS bytes unchanged.

Actual local cargo fmt --all then cargo fmt --all -- --check passed.
Actual cargo clippy --locked --workspace --all-targets -- -D warnings passed
compile-only (Finished dev in5.06s). No native tests/parser/worker/server/bench/
JVM/compiler/oracle ran locally. SAME3 actual source review dispatched; original
Linux CI execution and SAME3 completed-CI review pending. No merge lease.

Root preserved its tooling-only archived210 dependency lookup failure then
corrected to active204; source/blob equivalence0a0af→fa26 confirmed by root.
Owner status send to coordinator was specifically auto-review rejected: low-risk
status message lacked recognized trusted human authorization to message that
thread. No retry/proxy/workaround attempted; coordinator can pull owner chat.

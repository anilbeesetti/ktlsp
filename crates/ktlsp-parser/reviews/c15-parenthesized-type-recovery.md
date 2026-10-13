# C15 / #123: ordinary malformed parenthesized parameter type

Frozen delivered base18785f9efab15ec1cf94b4d78064a3c20de4b9e4,
tree4d8b0f94c1b2e1d47ed16993e87145c88013aa13. Source edits are confined to
parameters.rs and types.rs. No parsing/mod.rs, user_types.rs, dependency, worker,
API, workflow, shared status or existing C14/C12/resource fixture edit.
Parents #83/#7 remain open; T008 stays held; #96 remains OPEN/UNKNOWN.

## Pre-runtime source-derived expectations

Both independent correctness and adversarial reviewers derived the complete
selected-node projections in tests/recovery_parenthesized_types.rs directly from
the delivered grammar/builder source before any runtime execution. The architect
independently agreed. Existing C12 controls freeze declarations and diagnostics;
the additional nested type/annotation expectations here are newly source-derived,
not inherited runtime evidence. All selected declaration, parameter, type,
annotation, modifier, callee, function-type receiver and constructor nodes assert
numeric original spans, immediate parent, nearest declaration owner and all direct
IDENTIFIER children; unexpected selected nodes/direct names are rejected.

LF seed `fun broken(x: (@A\nfun healthy() = 2` (EOF after2) retains broken
FUN0..17, namebroken4..10, VALUE_PARAMETER_LIST10..17, VALUE_PARAMETER11..17
(namex11..12), TYPE_REFERENCE14..17, MODIFIER_LIST/ANNOTATION_ENTRY15..17,
CONSTRUCTOR_CALLEE/annotation TYPE_REFERENCE/USER_TYPE16..17. All damaged nodes
have nearest owner(0,17), and the top-level FUN has no declaration owner. Healthy
is FUN18..35 with namehealthy22..29, VALUE_PARAMETER_LIST29..31, immediate KT_FILE
and no declaration owner. CRLF shifts only healthy by1; lone CR matches LF.
Production token spans must partition the exact originals, UTF-16 positions retain
line0/column17 and healthy line1/columns0,4,11,17; CRLF interior is rejected.
Snapshot identity/stale tree rejection and a healthy revised followup are checked.

The exact diagnostic list for all three originals is
`[(17,17,"Type expected")]`. The initial recursive type parse encounters FUN,
which is in its non-consuming recovery set. Keeping the parenthesized parse
retains that deepest error item. Attempted missing inner/outer ')' items occupy
the same original lexeme; the existing builder duplicate_error_items policy
retains the first after whitespace balancing. No error-marker/global equal-offset
policy changes. No FUNCTION_TYPE node is fabricated for the damaged seed.

Four LF-only valid controls assert empty diagnostics and complete nested owners:
annotated function type (37 bytes), nested nullable function type (31 bytes),
receiver function type (33 bytes), and constructor annotated function type
(47 bytes). Their exact numeric preorder projections and direct names were
independently derived by both reviewers and are frozen in the test. The existing
C14 repaired seeds/controls, C12 constructor/function-type contracts, classifier
recovery, C16 known defect and five incomplete-bound contracts are read-only.
Partially asserted old incomplete bounds do not establish exact diagnostic lists.

## Caller, speculation and resource review

Ordinary function lists pass recovery=true into parse_value_parameter_2 and
parse_function_parameter_rest; all three parameter-rest type entries pass that
call-local argument into parse_parameter_type_ref. parse_type_ref_2/contents pass
it through parenthesis and intersection recursion. Constructors, function-type
try/fallback parameters, direct catch/context/lambda parameters pass false;
general type refs, type arguments, annotation callees/arguments, function-type
returns and truncated receiver parsing keep false entry APIs. No ambient context
can leak through nested calls or rollback. Valid parenthesized speculation and
function-type fallback use the existing grammar; only the damaged ordinary type
can stop before rollback would consume its complete named sibling.

C14 at_missing_default_sibling is renamed/exposed internally as
at_simple_function_header; its body is unchanged. The live builder is read-only;
a scratch builder copies at most256 complete raw tokens including internal trivia,
4096 bytes within effective truncated EOF and stops after four nontrivia tokens.
Four grammar expect calls recognize FUN/IDENTIFIER/LPAR/RPAR without recursive
parsing. Suppression remains separate from the ordinary context. All scratch
cursor/productions/remaps/diagnostics/flags are discarded, with no live state or
newline/join/layer stack mutation. Leading trivia inspection is bounded by input,
outside copied-window accounting. Each active malformed parenthesis can charge one
bounded probe while unwinding; input64KiB/nesting128 and existing allocation/time
ceilings remain, without a new performance guarantee or resource instrumentation.
Broader headers, constructor/function-type recovery and C16 are outside this child.

Cargo/lock/default Rust graph,64KiB input,500ms,64MiB requested allocation,4MiB
wire, element/diagnostic/depth ceilings and strict Coverage rejection are unchanged.
No JVM/JDK/compiler/C/C++ parser or oracle dependency/execution is introduced.
The frozen base does not yet contain resource_boundaries.rs; the assigned final
integration must inspect any newly delivered resource fixtures read-only.

## Provenance and review state

Original archive checksum and all54 upstream hashes/notices are unchanged.
Ordered pristine archive -> recovery.patch -> formatting.patch independently
reproduces all54 final source bytes and formatted hashes:10 behavior files,
49 formatting files,54 final inventory entries,5 pristine survivors. types.rs is
the added behavioral file. The active PROVENANCE phase count/list is corrected;
historical C14 evidence stays intact. No assumption from old9/49/54 counts.

Pre-runtime correctness/adversarial source reviews found no behavior blocker.
Correctness review requested all selected direct names be checked, not only
parameter/declaration names; that assertion was tightened without changing frozen
expected arrays. Adversarial review requested removal of redundant parentheses;
the same cleanup resolves it. Both reviewers rechecked the complete changed tests,
source and provenance and approved. Each independently extracted the original
archive and replayed both ordered patches to all54 final bytes/hashes; correctness
also verified zero-fuzz replay and exact rustfmt reproduction. Finished-source
architect review approved commit-and-hold, confirmed context/intersection behavior,
unchanged probe body and all54 final hashes, and requested the clarified wording
"no new probe implementation" (C15 adds invocations). That wording is corrected.
These are source-only reviews; candidate-specific final integration and completed
CI evidence still require the same reviewers and architect.

No local native/parser/test/benchmark/runtime execution has occurred. Local
rustfmt and static original-archive replay are authoring checks only. Source is
held for the coordinator's one exact stable integration assignment; preliminary
CI is not used. Authorized Linux CI must execute unchanged cargo fmt --all --
--check, cargo clippy --locked --workspace --all-targets -- -D warnings and cargo
test --locked --workspace on the final reviewed combined head and exact merged
main. Same two final reviewers and architect must read actual checkout/parent/tree
and completed logs/counts. Passing delivery, expected-SHA merge grant and explicit
vendor/provenance ownership release are still pending.

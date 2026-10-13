# C16: ordinary parameter annotation argument recovery

Frozen delivered base `aa9e6587bb28c368e2e770fc8fb297c59f83854b`, tree
`67f6ad4bed3048e7a86b9207784334e24c8ed6db`, sole parent
`6b76477f532f8140a51af1f4492bf46d39864301`. This finite child of #83/#7
does not establish full grammar, semantic or LSP parity. #83/#7/#96 remain open;
#96 is unresolved, T008/T008a stay held, B3/B4 remain gaps and B11 unverified.

## Expectations frozen before implementation and runtime

The same separate correctness and adversarial reviewers independently derived
every selected declaration, parameter/list, type, annotation, callee, argument,
block and call projection from the delivered source. Both and the architect read
the complete staged arrays before source edits. The original seed/three-control
freeze SHA256 is
`04c3c2e7a618b1572a5d1b91d7149feebaeb03e3b74de80d938eb4e32050d5e7`;
the separately appended lazy-body negative freeze SHA256 is
`81609833525a2337794974e7d9893fa38492a0f1598471c28548013b1fb55b71`.
These identify the pre-edit staged fixture texts, before combined-file rustfmt.
`tests/recovery_annotation_arguments.rs` preserves their complete numeric arrays,
immediate parents, nearest declaration spans and every selected direct identifier.
Unexpected selected nodes/names are rejected; anonymous functions have no names.

LF `fun broken(@A(\nfun healthy() = 2` ends immediately after `2`.
Damaged FUN0..14/name4..10, parameter list10..14, parameter/modifier/annotation
11..14, callee/annotation type/user type12..13 and argument list13..14 remain
within broken. No VALUE_ARGUMENT or parameter type is fabricated. Healthy is
FUN15..32/name19..26, with parameter list26..28. Both functions are direct
KT_FILE children with no declaration owner. CRLF shifts only healthy by one;
lone CR has the LF byte extents. Original tokens partition the entire input;
UTF-16 coordinates retain the original newline mapping and reject CRLF interior.
Production parse_snapshot/tree_for identity, stale rejection and healthy revision
followup remain strict.

Exact seed diagnostics for all three newline spellings are
`[(14,14,"Expecting ')'"),(14,14,"Parameter name expected")]`.
The annotation argument-list closure emits an error item. The missing parameter
name is an empty left-bound error marker; whitespace balancing places it at14.
The outer parameter loop emits its existing missing-')' item, which the existing
builder drops at the same balanced lexeme in favor of the first/deepest item.
The distinct error marker survives. No builder policy or blanket offset dedup
changes, and no empirical expectation tuning, are involved.

The three exact LF controls retain complete anonymous annotation/call argument
ownership, parameter/type ranges and BLOCK-owned local property/function names,
with no diagnostics. Their sources are unchanged from the admitted proposal.
The additional exact LF negative is
`fun f() = fun() { fun local(@A(\nfun healthy() = 2 }` (51 bytes).
Outer FUN0..51 owns anonymous FUN10..51; its BLOCK16..51 owns local FUN18..49.
Healthy FUN32..49 stays a VALUE_ARGUMENT child owned by local, without sibling
promotion. Its exact diagnostic is `[(49,49,"Expecting ')'")]`: the closing
brace makes annotation/parameter/outer-loop recovery non-consuming error items,
and existing item dedup retains the first. This is distinct from the seed marker.
The negative exposes fresh-parser lazy BLOCK rearming. It is not an exhaustive
negative corpus for every excluded parameter/type/expression context.

## Scoped recovery and bounds

Exactly six source paths change: declarations/{annotations,parameters,mod}.rs,
expressions/{primary,mod}.rs and parsing/mod.rs. The coordinator separately
admitted the context/expression paths and the two lazy-entry wrappers. Every
constructor/function-type/context/direct/probe/nested parameter entry explicitly
uses Nested while parsing modifiers; declining eligibility alone is insufficient.
The entire parameter body, including rest and speculative rollback, runs Nested.
Only eligible ordinary modifiers enter Modifiers. Their callback consumes an
actual Recovered result before restoration; only that parameter emits the empty
missing-name marker and closes without consuming the healthy header.

Annotation eligibility is captured only from Modifiers in Default mode outside
scratch suppression. Callee/type arguments/argument expressions run Nested; the
direct argument list receives eligibility as a local boolean. Its actual bounded
header stop returns the recovery result. Ordinary argument-list callers pass
false. Expression entry runs Nested, and block/lambda reparse entry does likewise
because lazy expansion creates a fresh Parser. This conservatively excludes new
C16 recovery for all local functions in reparsed blocks/lambdas. Existing C14/C15
recovery does not consult this field. Scoped callbacks restore state on normal
returns, including early returns and returns after speculative rollback; no
panic-safe guarantee is made.
Builder rollback does not restore Parser fields.

The C14 four-terminal scratch probe body is unchanged; only internal visibility
widens so the sibling expression module can call it. Its per-call limits remain
256 complete raw tokens,4096 bytes,effective truncated EOF and at most four
expect calls, without recursive grammar or live state mutation. Each new eligible
invocation is individually bounded; no whole-parser linearity claim follows.
Leading trivia inspection stays outside copied-window accounting. Probe suppression
and parsing context remain separate. Existing worker/resource/Coverage contracts,
native dependency graph, Cargo/lock, API and protocol are unchanged.

## Validation and publication state

Existing C14/C15 positives, five incomplete-bound partial assertions, classifier
nested-FUN assertion and resource fixtures are read-only. Partial old assertions
do not establish complete diagnostic preservation. No JVM/JDK/compiler, oracle,
Gradle/Maven, local parser/server/test/benchmark runtime has executed for C16.
Local rustfmt and compile-only Clippy are authoring checks; Linux behavior remains
pending the coordinator's one assigned integration/CI turn and unchanged all3.
Same reviewers and architect must review actual source/provenance and then actual
completed Linux checkout parents/tree/logs/counts before delivery can be claimed.

The local format check and locked offline workspace/all-target Clippy passed.
The initial compile-only check caught sibling-module visibility for the existing
header helper; only visibility changed, without changing its body or fixtures.
Initial provenance authoring attempts retained a strict mismatch because raw
upstream constructors omit trailing commas. Architect review approved a script
match with an exact two-substitution assertion; no source expectation changed.
Fresh original-archive -> recovery.patch -> formatting.patch replay then reproduced
all54 final source bytes/hashes with zero fuzz/offsets:14 behavior files,50 formatting
files and4 pristine survivors. Both separate reviewers independently extracted
the original checksummed archive and replayed the ordered patches into fresh
directories, reproducing all54 paths/bytes/hashes with zero fuzz/offsets and the
same counts. Correctness review also independently reproduced exact rustfmt bytes.
Both approve the local source/fixture/provenance package; architect review approves
commit-and-hold with runtime, integration and publication gates still pending.

GitHub issue publication was rejected twice by automatic approval review. The
second objection requires direct authorization in this child chat; external issue,
push/PR/CI publication remains blocked. Local source preparation does not satisfy
that gate. No passing merge or ownership release is claimed by this note.

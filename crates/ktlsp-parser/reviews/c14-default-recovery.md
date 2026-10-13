# C14 / #121: missing ordinary-parameter default

Frozen base: `0512d6dbbf9d1687e2977693cb814076d6d5e54e` (C12/C17 delivered).
Source ownership is only `vendor/ktrs-parser-0.5.1/src/parsing/declarations/parameters.rs`.
C15/C16 assertions, classifier recovery, worker/API/dependencies/limits and C12
constructor/function-type/resource fixtures are unchanged. No server provider or
syntax/semantic parity claim. Parents #83/#7 remain open; T008 stays held.

## Source-derived expectations frozen before runtime

`tests/recovery_defaults.rs` is the exact numeric preorder projection for every
selected declaration, parameter/list, type and annotation node, with immediate
parent kind, nearest declaration span and all direct declaration names. These
inputs contain no annotation nodes; the complete projection rejects unexpected
ones. LF, CRLF and lone CR damaged originals preserve the damaged FUN0..19,
VALUE_PARAMETER_LIST10..19, VALUE_PARAMETER11..19, TYPE_REFERENCE/USER_TYPE14..17.
The complete healthy FUN starts at19+newline bytes and ends17 bytes later, is a
KT_FILE child with no declaration owner, and directly owns healthy at+4..+11.
Its VALUE_PARAMETER_LIST is+11..+13. Token spans partition the original input;
UTF-16 positions retain line0/column19 and healthy line1/columns0,4,11,17; the
interior of CRLF is rejected. Snapshot identity/stale revision and healthy-followup
checks traverse the production parse_snapshot/tree_for path.

Expected diagnostic is exactly `(19,19,"Expecting an expression")`. The default
entry emits the first error item; after closing the damaged parameter the loop
emits `Expecting ')'`. Whitespace balancing places both at the same original
lexeme, and upstream duplicate_error_items retains the first/deepest error item.
This preserves that existing policy; it does not impose blanket equal-offset
suppression for error markers. C15/C16 diagnostic messages/order remain unchanged.

Four LF valid controls retain the exact existing anonymous-function/object literal
source, lambda-local named function under BLOCK/default, nested calls and a
same-line anonymous expression function. All have no diagnostics and frozen
nested declaration/parameter/type extents and direct names. The nonempty named
header control retains the parser's existing nested expression ownership; this
is a residual parser behavior assertion, not a claim of Kotlin validity.
Scratch-threshold cases freeze accepted/declined ownership and diagnostics at
4096/4097 header bytes and255/257 copied raw tokens. Declined cases retain the
original whole damaged FUN/default-nested healthy FUN and both EOF diagnostics.

## Context and bound review

Function callers pass `(false,false,...,true)` to parse_value_parameter_list;
primary/secondary constructors pass recover_declarations=false; function types
use the separate try/fallback branch. The ordinary-list closure carries recovery
as a call-local argument through parse_value_parameter_2/function_parameter_rest.
Direct catch-parameter and try/context-parameter calls pass false. Nested parsing
and speculative rollback cannot mutate an outer call's context. The existing
in_parameter_recovery_probe suppresses recursion independently of this context.

At post-EQ entry only, a read-only live builder view supplies an isolated scratch
builder. The window copies at most256 complete raw tokens (including internal
trivia),4096 bytes, and ends within the minimum active nonnegative truncated EOF.
It stops after four nontrivia tokens. Four grammar expect calls require FUN,
IDENTIFIER, LPAR, RPAR; there is no recursive grammar call or forward text search.
A cheap non-FUN miss does not allocate scratch. Leading trivia inspection is
allocation-free and bounded by the64KiB input. Each eligible call follows one
consumed EQ; scratch work has the per-call caps above and cannot recursively
probe. Failed/successful scratch parsers are discarded; live cursor, production,
remaps, diagnostics, parser flags and newline/join/layer stacks are untouched.
No live-marker rollback is relied on. Complex/nonempty/generic/extension/modifier
headers remain outside this child's recovery guarantee.

## Validation and delivery state

Pre-runtime expectations were written before any runtime validation. Initial
architect plan review approved the finite four-terminal isolated probe and
call-local context. Two independent source reviewers and finished architect review
are required before publication. No local parser/native/runtime/performance/JVM
execution is permitted for this child; #96 remains OPEN/UNKNOWN. Linux Native Rust
CI is the authorized runtime route with unchanged pinned fmt/clippy/workspace-test
commands. Exact reviewed combined-head and merged-main checkout/parents/tree and
completed logs must be recorded separately from source-only reviews.

Ordered pristine-archive replay recomputed9 behavioral files,49 formatting files,
54 final source files and5 pristine survivors. Original archive checksum,
upstream source hashes and notices are unchanged. Final reproduction and graph
checks remain required. Runtime/review/merge evidence will be recorded in the PR
and coordinator receipt; this note is not a completed-delivery claim.

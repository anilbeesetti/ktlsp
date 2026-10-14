# C13 longer simple generic receiver recovery — source candidate

This T007/#7/#83 child extends sibling recovery only to unqualified nonnullable
receivers with three or more simple unqualified nonnullable identifier type
arguments and a plain DOT suffix, for example `fun Box<A,B,C>.healthy() = 2`.
It adds no semantic behavior or server provider. Broader C13/B3/B4 remain GAP,
full B11 UNVERIFIED, T008/T008a HOLD and #96 Darwin UNKNOWN.

## Exact inputs and reach

Assigned delivered base `66d2a99c752f901dbe7a46a7fc5254cdd8a90484`, tree
`c3e117c44097f533ef4a5440411be1e5b25a27a4`, sole parent
`9bf15b653219f105b99a239121098cef2fd932df`. The explicit nine-path source grant
has SHA256 `5ce8d8b6df203d600fe9894e02843a1585126b4f8824c84c1cf702aa8b3e48da`.
Separate incoming reconciliation verifies all1002 base paths and the actual
PR253 ten-path inputs against accepted provisional253 source before writes.
Preparation remains frozen at fe280/provisional68a; it is not relabeled main.
Frozen CONTRACT_READY SHA256
`6904e1e788d1b5535d2206431b84824fef786ce1dcb2709864002b720ff16299`, corrected
candidate `f7eee3691a9ce3a1b440fd6a784c7fc264b6512e635e4bd3d2c8405c0a05ba24`.

The existing private `at_generic_sibling_function` is called only before a
classifier parameter and after a completed parameter/bound. `classes.rs` enables
it for class/interface, excluding object; function type-parameter lists remain
disabled. After the existing first comma and second identifier, the predicate
consumes additional comma/identifier pairs. Each successful iteration consumes
both terminals; a failed identifier ends the loop and refuses the header. It
then requires GT and, for three or more arguments, plain DOT. One- and
two-argument DOT/SAFE_ACCESS/(QUEST DOT) behavior remains unchanged. Direct
function IDENTIFIER followed by observable LPAR is still required.

Nullable N>=3 receivers, nullable/nested/qualified/annotated/variance/star/
function/intersection/parenthesized arguments, empty/repeated/trailing/missing
arguments and bare suspend receiver/arguments remain declined. Hard in is not
an identifier; soft out, escaped identifiers, dynamic and trivia keep existing
syntax behavior. Live type parsing, completed-bound checks, scratch isolation,
modifier/annotation gates and recursive-probe exclusion are untouched.

## Frozen tests and historical preservation

The focused test embeds all165 frozen objects,48 complete CST expectations and
231567 original UTF16 positions unchanged as JSON values and original source
bytes. It compares token-inclusive preorder and actual parent indices directly;
the independently authored marker/leaf views use a separate node-index map.
All cases require strict parse success, exact snapshot/source association,
complete lossless original token partitions, immediate parents, subtree/span
bounds, nearest declaration containment, all byte-position mappings, four stale
identities, and same-Parser healthy revision2 with exact declaration/name/list
bounds and zero diagnostics/errors. Old trees retain exact source, snapshot,
ordered elements and diagnostics. Ordered ERROR_ELEMENT rows preserve duplicate
zero-width EOF markers. Coverage/resource/refusal never substitutes for success.

Declined controls require original CLASS/list retention beyond the candidate and
no candidate-root FUN. Aggregate one-extra/exact/one-less controls distinguish
candidate admission from refusal and require fresh-list recovery. Source65536,
leading4401 trivia and disabled object/function entries have explicit ownership
and bounds; source65537 separately rejects admission. LF/CRLF/lone CR fixtures
preserve literal source bytes and frozen UTF16 maps. The three ordinary ending
labels intentionally share newline-free bytes and do not prove newline coverage.

The complete historical audit found one logical expectation change: twoarg131
case44 `extra-third-argument`, exact source
`class Box<T\nfun Triple<String,Int,Any>.healthy() = 2`. Additive stable-ID/source
equality dispatch selects its independently frozen new record. Its raw payload,
all other130 objects, existing nullable-two eight-ID overlay and every literal
source byte remain unchanged. Nullable-two case51 is explicitly unchanged and
still declined. No other historical dispatch, helpers or raw corpus changes.

## Bounds and validation limits

The unchanged caller FIRST copies at most256 raw tokens/4096 normalized bytes,
whole tokens constrained by effective EOF; repetition runs only inside that
finite scratch builder. The whole copied window is debited before grammar from
the shared4096 classifier allowance, including failed windows/trailing trivia.
Leading trivia and cheap prefilter misses retain existing accounting. No new
argument-count cap or total-work promise is introduced.

For these literal sources, compact N names cost2N+7 raw tokens/2N+17 bytes through
LPAR: N124 costs255 raw, N125 needs257; one comment raises N124 to256. Prefix and
trivia costs change the largest admitted list.64-byte identifiers admit N62 at
4047 bytes while N63 requires4112 despite only133 raw tokens. Raw255/256/257,
byte4095/4096/4097, all terminal cuts and whole-token straddles stay frozen.
Aggregate prior debit4082/4083/4084 leaves14/13/12 tokens for the13-token header;
fresh class resets4096. These are static contract derivations, not runtime proof.
64KiB source/16384 elements/128 depth/256 diagnostics/4096 message bytes/4MiB
wire/500ms exchange/64MiB cumulative requested allocation remain unchanged;
RSS, stack, hard cleanup time and full active-EOF B11 are not established.

This implementation stage has performed static source/data checks only. Root
owns the five vendor provenance/patch/hash artifacts, retained independent
correctness/adversarial/architect reviews, formatting and compile-only locked
Clippy. No parser/tokenizer/worker/server/test/bench/JVM/oracle runtime ran here.
Hosted pinned format/locked Clippy/full-workspace tests and actual SAME3 reviews
remain separate required evidence. Source readiness grants no publication or
merge; root controls that later gate and exact-head/base lease.

Native product/build/core/default tests remain Rust-only with no bundled JVM,
JDK/compiler/JNI/C/C++ analyzer/tree-sitter C. Optional existing external tools
and isolated official oracle remain at their separate boundary, unused here.

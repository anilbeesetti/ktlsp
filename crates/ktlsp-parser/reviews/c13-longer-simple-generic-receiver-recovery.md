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
syntax behavior. Live type grammar, completed-bound checks, scratch isolation, modifier/annotation
gates and recursive-probe exclusion are unchanged. The later N261 draft below
adds bounded error consumption only for a verified declined prefix.

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

## N261 unpublished bounded declined-prefix repair draft

Original PR261 head `efabccee5eb4eee6213d606b4bf9be16da67f70f`, base
`46fd28ddba16ddd6d000588cae56548a7af3d732`, ran original Linux attempt1
38661078501/job116182203658. The runner checkout `04f87b22d108a3b71b66e9d5690b492ac73edfe4`
had the exact reviewed tree `1fc1f56d50a252ee99bd8aa8f865e3a2392dd8ad` and all1023
source files. Format and locked all-target Clippy passed; workspace tests failed
at `compact-n125` with `OutputLimit` after147 preceding N cases passed. Later17
N cases and full-workspace completion remain unobserved. The partial run reported
499 Rust passes, one failure and two ignored helpers; the expected656 total was
not achieved. First raw output, original expectations and failed inspections are
preserved separately and are never replaced by this draft.

Source trace explains the failure: N125 needs direct LPAR at raw257, outside the
unchanged256-token scratch window. Scratch correctly declines. The live missing-GT
fallback consumes only FUN, then258 remaining significant terminals each receive
a top-level error; together with MissingGT this yields259 diagnostics, exceeding
the unchanged256 worker cap. This is a declined-path cascade, not widened scratch
admission or a reason to raise worker limits.

The exact seven-path draft grant SHA256
`446f3aafeca10268b5bd44efd1c33e67853ec88609efb9c0222b4b03b7d3bdcc` permits only
existing type_parameters.rs private predicate/classifier callers, this review
text and five derived vendor artifacts. The same already-debited scratch parse
may return a verified exclusive raw endpoint only after a clean N>=3 simple
nonnullable receiver, GT, plain DOT and direct name, when final LPAR is absent
because scratch reached EOF. Capture occurs immediately after name, before any
trivia-skipping query; checked translation confines the endpoint to the copied
window and effective EOF. Successful found, N1/N2, property, recursive, unverified
and explicit-next-nonLPAR paths carry no endpoint.

Both classifier call sites handle a returned endpoint immediately at the unchanged
candidate cursor. Live recovery groups precisely that prefix into one MissingGT
error, keeping recovered_function=false. Raw-index guards precede EOF/token
queries; explicit EOF/progress protection prevents spinning. The following LPAR,
trailing trivia, unverified suffix and later declarations are not consumed by
this grouping. No second scan/copy/parser, new dependency or public API is added.

Source-only retained reviews identify six affected ownership-only controls:
`no-lpar`, `raw-cut-12`, `byte-cut-12`, `compact-n125`, `byte-lpar-over`, and
`aggregate-one-less`. Each still declines sibling recovery, retains original
CLASS/list beyond the candidate and has no candidate-root FUN. Aggregate fresh-list
recovery and all48 exact full-CST expectations remain required. Independent
historical audits of482 raw executable cases and14 overlay/erratum records find
no new expectation amendments: historical case44 already observes LPAR and takes
found=true, nullable/malformed/N1/N2 controls remain excluded.

All165/new and131/historical test bytes, raw payloads, overlays, expected CST,
ranges, diagnostics, UTF16 maps and stale identities remain unchanged. Raw256,
byte4096, shared4096, diagnostics256, source64KiB and all worker limits remain
unchanged. Static review and compilation do not prove the17 unobserved outcomes,
allocation/deadline behavior, full active-EOF B11, crash safety or full parity.

This is an unpublished source draft only: no new commit/push/fetch/incoming merge,
public-body update, local parser/tokenizer/worker/test runtime or CI rerun is
admitted. Remote PR261 remains pinned to originalEFabc through257 OWNER+ROOT
delivery. Retained SAME2/architect changed-source reviews, pinned formatting,
compile-only locked workspace/all-target Clippy and one fresh pristine ordered
patch replay are required before REPAIR_SOURCE_READY; later runtime/publication/
final-cut/merge gates remain separate. #96 Darwin UNKNOWN, C13/B3/B4 GAP,
fullB11 UNVERIFIED, T008/T008a HOLD and #7/#83 OPEN remain unchanged.

The archived initial behavior-to-final diff for type_parameters.rs also contained
an inherited comma-arm fallback/nesting change; its formatting-only classification
was inaccurate. This draft places that inherited behavior in recovery.patch by
reconstructing the behavior file from repaired final source and restoring only
three known formatting spans (imports and two expect calls). The other53 accepted
behavior files remain byte-identical. New behavior-to-final differences for this
file must be whitespace-only. Original patches and their hashes remain preserved;
new phase counts and hashes are derived from the actual data, not assumed.

Completed draft checks: pinned Rust1.99.0 formatting and compile-only locked
workspace/all-target Clippy both exited0 for source SHA256
873331a853582595578acb34cb5bd738ab19b648484cf6129e86a9a8f4bbf3a0.
No test bodies or local parser/tokenizer/worker runtime were executed. One fresh
pristine extraction and one current recovery-then-formatting replay passed with
zero offsets/fuzz and exact54 final byte/hash matches; notices remained unchanged.
Derived counts are16 behavior files,50 formatting files,54 final files and4
pristine survivors. The two reconstructed single-line expect calls retain final
trailing commas, so this file's new formatting delta is strictly whitespace-only.
New draft phase hashes:
- recovery.patch SHA256 `fcb338c7e9bdc60757e76f4f202c2585ae2ec11ab1626dfe97dde668e5f77eb5`
- formatting.patch SHA256 `4b1abb4ab303923222fa4b0a2121afea2b9f8eaed33f380f95149ad4e3448aa4`
- formatted-sha256.json SHA256 `0b56fc9a47b5b164e2ef4bea073d19af2ce924fddc3412dab30ba6f510c95aa1`

These checks and source-only reviews do not replace the preserved first failed
Linux run or establish a passing repair runtime. Remote head remains original
EFabc; unpublished draft readiness still requires completed retained SAME3
packet review and ROOT admission for any later action.

## N261 aggregate preceding-comment assertion erratum — preparation only

The original accepted-aggregate assertion equated the recovered FUN start with
its FUN_KEYWORD boundary. Existing top-level declaration closing uses
PrecedingComments: one newline before a plain block comment and one newline
after it bind that comment and the following newline to the function. The
damaged CLASS/list ends before the comment; the recovery candidate remains the
direct FUN_KEYWORD. This follows unchanged preamble.rs, declaration closing in
abstract_kotlin_parsing.rs and binders.rs, rather than observed CST output.

Only scratch_match=true aggregate-one-extra/exact gain inline assertions. Their
unique direct KT_FILE-owned FUN is identified by its direct FUN_KEYWORD token at
16458/16459. Exact FUN ranges are [12360,16487)/[12361,16488), with direct-owned
4097-byte block comments [12360,16457)/[12361,16458). The newline before each
comment and after each function remains root-owned; the newline between comment
and keyword is function-owned. Strict damaged CLASS/list nonoverlap and all
existing candidate/class/list/fresh-class/healthy checks remain required.

This narrow contract erratum does not alter the shared start locator, the
declined aggregate case, production/vendor/binders/budgets, historical tests or
any frozen literal. All165 source/stale objects,48 full-CST expectations and
231567 UTF16 entries remain byte-identical in the raw payload. The earlier
test-byte preservation statement describes the preceding bounded-prefix repair;
this later proposal changes only inline assertion code and this review note.
No parser/tokenizer/runtime/format/build or CI rerun occurred in this preparation.
The two-file proposal requires a new exact root source grant before application.

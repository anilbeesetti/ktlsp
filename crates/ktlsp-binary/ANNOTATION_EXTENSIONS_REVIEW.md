# T017c3a / #115: parameter annotations and defaults

Frozen source base `61142f66f047a6ac45af5032f4237b30ce6134f6`.
The [accepted contract](https://github.com/anilbeesetti/ktlsp/issues/115) pins
JVMS 23 §§4.7.16.1, 4.7.18–19, 4.7.22 and local JLS 9.6.1–2 shape constraints.
This child leaves #71/#17 open: type annotations, Code/Record interiors,
bytecode/linkage and Java/Kotlin semantic validation remain pending.

## API and shared helpers

`annotation_extensions::decode(&ClassFile)` returns declaration annotations and
one method-extension entry per method. Parameter attributes preserve visibility,
order, empty entries and encoded positions; positions are never aligned to method
descriptors, MethodParameters or signatures. Defaults distinguish absent values
from empty arrays. Values retain the existing borrowed UTF-16/numeric-bit model.

The root-approved helper change exposes only crate-private Budget and its
nodes/work/text methods, annotation/value, and decode_with_budget. Budget fields
stay private. The existing public declaration decoder remains a fresh-budget
wrapper around its mechanically extracted body, preserving checks/error order.
The combined decoder uses that same budget for both scans. Raw attribute payloads
and member/container nodes are counted once; repeated attribute-name reads,
consumed method descriptors and extension values consume work. Combined calls can
legitimately exhaust work earlier than declaration-only calls.

## Bounds and limitations

One call caps all visited raw payloads at16MiB, aggregate nodes at65,535, work
at64Mi units and annotation/value nesting at64. The node equation is
F+M+A+P+R+E+V+N, as defined in #115. Counts are charged before growth/descent;
default root values start at depth1 with one explicit node charge. Added shape
matching charges 1+3*descriptor length per root/direct array entry and
min(descriptor lengths)+1 for enum/annotation candidate comparison. Method
name/descriptor validation runs once per consumed method, regardless of extension
attribute count. Shared annotation decoding/duplicate-name charging is unchanged.

Recognized extensions are method-only, unique by attribute-name content, and
require class versions49.0–67.0 under the existing conservative version policy.
Default-bearing methods must locally resemble annotation-interface elements;
0x0800 is rejected as ACC_STRICT at49–60 and ignored as reserved at61–67.
Return/value shapes are checked without resolving referenced classes: matching
enum/annotation descriptors do not establish actual class kind, element/constant
existence, Class generic bounds or assignability. Default application and external
annotation-element validation are absent. Nested annotation values are structural
data. Raw ignored attributes/Code/type annotations remain unvalidated. Existing
Error is unpositioned; truncation cases label payload-relative prefixes only.

## Permanent tests and data provenance

The focused suite uses independently authored Rust bytes. It covers both
visibilities, counts0/1/255, omitted/synthetic descriptor mappings, empty methods,
every value tag/exact bits/UTF-16, local shape mismatches and unknown structural
candidates, placements/duplicates/version/flag boundaries, consumed special-method
and255-slot rules, forged CP indices/types/wide holes, every-byte payload prefixes,
trailing bytes/count precedence, declaration-error precedence, mixed65,535/65,536
node totals,16MiB payload accounting, exact64Mi shared work and earlier combined
exhaustion, depth64/65/drop and finite deterministic byte mutations.

The unchanged existing Kotlin Metadata.class fixture checks all eight named
defaults. The new766-byte ReadAfterEOFException.class.bin fixture checks its
runtime-invisible Nullable constructor parameter. Its archive/class/notice hashes,
artifact URL and compiler-provenance limits are retained in
tests/fixtures/annotation_extensions/provenance.json. It is an unmodified entry
read from the already-existing kotlin-stdlib2.3.20 ZIP; no compiler/JVM/Gradle/Maven
or oracle ran. LICENSE/NOTICE remain unchanged. No dependencies/Cargo/lock/status
files or server capabilities change.

## Reviews and execution gates

Two independent contract reviews and architect review approved #115 after
correcting version-sensitive ACC_STRICT, exact counter formulas, truncation
precedence and combined-work behavior. Root separately approved the precise helper
delta and implementation/data-fixture stage.

Separate functionality and adversarial source reviews checked implementation,
budgets/tests/fixture hashes and the candidate-specific worker graph. Initial
fixture expectation errors (NotNull versus actual Nullable, seven versus eight
Metadata defaults) were corrected and both reviewers verified actual binary data.
Neither reviewer ran local product/native/parser/runtime checks. No parser/server
dependency or worker code/supervision/workflow change exists; the new binary suite
adds workspace test work only. Local compile-only clippy passed after correcting
four initial test borrow-scope compilation errors. Format is checked separately.

Product runtime validation is reserved for the existing Linux Native Rust CI using
all three unchanged pinned commands on the final reviewed head and exact merged
main. Exact head/check URLs/counts and final review rechecks belong in the PR and
coordinator report. Until those gates pass, execution is unverified. No Darwin
runtime retry is permitted in this lane; #96 remains open/unknown. Linux success
does not establish macOS reliability, T008 admission, semantic/full LSP parity,
exhaustive crash safety or a performance comparison. Merge requires root's stable
integration turn and expected-SHA grant.

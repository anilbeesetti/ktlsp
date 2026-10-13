# #142 method type annotations: source stage

Child of #71/#17: https://github.com/anilbeesetti/ktlsp/issues/142.
Initial implementation base8548ead597146a4ca52d3a4a2ec6ffd724fac7a3,
tree3f98c1ec916437445fb6e499a85caa04c5959e63. Contract v2 SHA256
6f2737f117be751626bb36d4cbd846b78cde0a3c557c434f267eae4a446a1eff is copied
unchanged into METHOD_TYPE_ANNOTATIONS_SPEC.md. Its earlier CONTRACT-ONLY wording
is historical; root admitted finite source/shared helpers/issue publication,
local format and compile-only Clippy. Push/PR/CI/integration/merge remain held.

## Implementation and compatibility

New method_type_annotations::decode wraps existing FieldTypeAnnotations plus
method-aligned type annotations, sharing one Budget through declarations,
extensions, fields and methods. Initial supported target0x14 is encoded data;
five other legal method targets return existing UnsupportedTag immediately,
remaining target_info/path/values unvalidated. Illegal method targets return
Malformed("method type annotation target"). All failures return no partial
output, preserving original raw ClassFile/Attribute access.

Shared delta is exactly field decode_with_budget extraction, existing vector
helper crate visibility and additive lib export. Field public API/result/error/
charge order, decode_fields, vector/extra_capacity and original unit tests remain
unchanged. No Budget/old Vec/core errors/Cargo/lock/dependency/metadata/server/
parser/vendor/project/workflow/root-status changes. New method vectors reserve
requested nodes before allocation and observed capacity excess before retention;
outer/lazy wrapper/root/path charges share existing limits. This is no RSS or
whole-operation owned-byte guarantee.

Focused source assertions cover21 frozen hex fixtures; all256target dispositions;
mixed unsupported/first-malformed/duplicates/versions/CP/unconsumed method refs;
void/nonvoid/constructor data; path/value/bits/UTF16/depth/prefix/mutation cases;
method-only5+X and mixed25+X nodes,16MiB bytes±1,64Mi work±1; extra +3name control;
private reservation/work/exhaustion precedence and field-error-before-method
outer reservation. Existing helper synthetic capacity tests remain unchanged.

## Provenance and reviews

Unmodified licensed Guava collect/AbstractIterator.class2361B SHA256
6af0f4300e51fcb4d4720371f113e8f50e39b8f8e8c5f7f654a1297b275e0087,
primary archive a1f8a4bafe5a77232d333356843e194599b30837574a7f4be1063daaaaab102c.
Real0x14 method payloads at1548/1669,field0x13 and opaque class0x11. Exact LICENSE,
source-header NOTICE,manifest,provenance and21authoredhex copied BEFORE code.
Compiler identity UNKNOWN; manifest JDK11 is distribution metadata only. No JVM/
compiler/Gradle/Maven/oracle/native product execution authored this data.

SAME retained correctness01a1344f-7078-7700-af8a-c3644672a92f,
adversarial01a1344f-a3b6-7bf2-8993-ab1f932577b0,own architect
01a1344c-09be-7c51-92c9-ea06ee9eb2a4 approved the v2 contract. Architect's missing
exact duplicate error label finding was fixed and all three rechecked. Source
review found a test portability gap: an outer vector can have observed capacity
above its requested count, exhausting nodes before trailing-byte inspection.
The private boundary assertion now permits only that node error or the trailing
error; the mixed earlier-field control likewise permits only node overflow.
Successful combined outputs still check actual capacity equations, and logical
+1 always fails. Product behavior/fixtures/expectations did not change. SAME
reviewers recheck the correction and exact final manifest; actual final approval
turns/head/tree receipts belong in the coordinator report to avoid self-reference.
The adversarial review had one model-capacity failure; the SAME reviewer/model/
settings were retained without replacement and resumed normally.

Initial format check failed with formatting diffs only, then cargo fmt applied
and all later format checks passed. First compile-only workspace/all-target
Clippy failed one leftover `.fields` test adapter; corrected to `.methods`.
Second compile-only Clippy passed (3.19s); after test portability corrections the
same locked workspace/all-target command passed again (final0.50s). Exact commands:
`cargo fmt --all -- --check` and
`cargo clippy --locked --workspace --all-targets -- -D warnings`.
Final Clippy log SHA2564258affcd6e94a57a30973f9edffcc8b9631f139de119bbec5ec0ee7859cbb49;
passing fmt log is empty SHA256e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855.
These checks compile tests but execute no product test/parser/worker/server.
No runtime test or CI result is claimed. All failures/corrections are preserved
in owner evidence. Git diff whitespace check reports original manifest CRLF
lines/final blank line; the upstream data stays byte-identical, with no source
whitespace repair or provenance change. Issue142 was published with exact frozen title/body; one
misdirected own admission comment on140 was immediately deleted, then the same
status comment posted to142 and readback verified. No persistent140change.

No local native/runtime/tests/parser/worker/server/bench/JVM/oracle/Gradle/Maven
execution is admitted; #96 remains OPEN/UNKNOWN. Eventual existing unchanged Linux
Native Rust all3 must validate one root-assigned final combined head/tree/original
attempt, followed by SAME3 source+completed-CI finals and root exclusive expected-
head guarded merge. Identical reviewed-tree postmerge verification belongs to
owner and root under#134 policy; changed identity/failure/new finding renews
relevant reviews. Parents71/17 remain OPEN;T008/T008aHOLD;B3/B4GAP;B11UNVERIFIED.
Full metadata,semantic/LSP parity and official speed/RAM comparisons remain pending.

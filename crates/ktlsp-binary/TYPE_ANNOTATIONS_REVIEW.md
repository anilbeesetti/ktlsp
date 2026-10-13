# #130 field type annotations: source-stage evidence

Child of #71/#17: https://github.com/anilbeesetti/ktlsp/issues/130.
Frozen contract source b910df05ba8942350700e2985d497511459952b7; exact assigned
implementation base3a2eede237fa1d52b91974059958e8251230673a,
tree06d0725b79109e284a8de3c44602ce49b68e0b97. No main chasing.
Frozen v2 contract SHA256e084e19334ef603c474bd6ba28e945554864cb2be4ed5a5d6669ecc0837848fa.
Root admitted source/issue publication only, with optional local fmt/compile-only
Clippy; push/PR/CI/integration/merge remain separate gates.

## Implementation and compatibility

New type_annotations::decode wraps existing AnnotationExtensions with aligned
field type data; it creates one Budget, calls mechanically extracted crate-private
annotation_extensions::decode_with_budget, then field-only third pass. Existing
public decode/result/validation/error order and opaque type acceptance remain.
Shared diff is only that extraction and additive lib.rs module export. No shared
Budget/old Vec/metadata/Cargo/lock/workflow/server/vendor or root status changes.

The field reader recognizes both visibility attrs at52.0–67.0, target0x13 with
empty target_info, encoded u1 paths and existing annotation values at depth1.
Duplicate visibility attrs compare name content per field. Exact exhaustion
precedes lazy wrapper reservation. Field name/descriptor remain unconsumed for
forged inputs; other class/method/Code/Record type attrs remain opaque.

All raw payloads are charged once during declarations. Field-only name work and
target/path work share existing budget; new requested vector slots are charged
before fixed reserve, observed capacity extra before retention. Formula
F+M+A+P+R+E+V+N+Q+C+T+L+O remains as frozen. Inherited Vec spare capacity,
allocator overhead/RSS and semantic applicability are unproved/excluded.

Focused assertions cover the14 frozen hand-authored hex cases; value bits/UTF16/
nested arrays; field alignment/empty/absent/order; all255 wrong target tags;
path0/1/255 and all kind/index boundaries; versions; duplicate content and
declarations/extensions-first precedence; consumed CP/name/descriptor checks;
every-byte valid prefixes/trailing; depth64/65/drop; fixed byte mutations;
mixed/empty-member node boundaries; exact payload/work sharing; private synthetic
capacity reconciliation; and licensed precompiled Guava field bytes.

## Provenance

tests/fixtures/type_annotations contains the unchanged2893-byte precompiled
Guava class, exact LICENSE/source-header NOTICE and provenance JSON.14 authored
hex files are unchanged from pre-runtime contract preparation. Primary archive
and cached archive hashes/class bytes match; cache directory discrepancy remains
explicit. Compiler identity and cryptographic signature authenticity are unknown.
No compiler/JVM/Gradle/Maven or oracle ran; Rust data reader tests remain unexecuted
locally. The complete frozen contract is copied unchanged into the local spec;
its CONTRACT-ONLY wording records the earlier stage, this document records the
later explicit source admission.

## Reviews and validation

Same retained reviewers: correctness01a133a8-eb69-7440-ac1b-47c6efd7c00b,
adversarial01a133a9-0e03-77b1-9f94-4f4552128bfa; owner architect
01a133a6-66cc-73a1-ac88-04a024aab046. Correctness source review approved with no
actionable findings. It independently verified source/test/shared-delta hashes,
all14 frozen hex fixtures, primary class/LICENSE/NOTICE/provenance, the mixed
node/exact-work equations, old endpoint compatibility and inherited full graph.
Adversarial source review found no product-code blocker; its one actionable
fixture README relative-link error was corrected. README now also explicitly
identifies frozen provenance's contract-era stage wording as historical.

Architect source review found an acceptance-test gap: one attribute-name unit
adds3 work units across the scans, not the required exact+1. Corrected construction
uses an empty string consumed once by the new field decoder: fixed103 units plus
340*3*65536+3*87347 equals64Mi. Appending one value unit tests exact+1; clearing it
then appending one name unit retains separate+3 scan-overflow coverage. This is a
source-derived correction made without runtime. All SAME correctness/adversarial/
architect reviewers rechecked and approved with no remaining actionable findings.
Their source/graph approvals are static, not runtime/CI or merge approval.

Reviewed decoder SHA256dde8205150aa7c974fb149fa65caf952741e4bfae9368b8af0499d1cc1858ddb;
focused tests9334c9246b628c1ee02956c89d8e736887f3312e914bbbd329e54efd13d1963c;
extension105f12e344b8e8e7b12e6e4f1ee6c58620086c27a9e500d1b92b55054d676834;
libd20526e58a7c14ef5d1730fd510a54245257244783814651124dd62aaa2b7956.
Final clean-head/tree receipts belong in the coordinator report to avoid a
self-referential commit hash. Same reviewers remain reserved for integration/CI.

Initial `cargo fmt --all -- --check` exited1 with formatting diffs only; owner
applied `cargo fmt --all`. Compile-only
`cargo clippy --locked --workspace --all-targets -- -D warnings` passed on the
first attempt (dev profile,4.70s). This type-checks test code; it executes no
product test/worker/parser/server binary. Final format check passed. After the
source-derived work-test correction, both `cargo fmt --all -- --check` and the
same compile-only locked workspace/all-target Clippy command passed (Clippy0.47s).
First Clippy log SHA256ca464c714eb596b5781c85894e4aa66cedea151e1a42de6e72b99983f8b2a887;
changed-source Clippy log414a34006954f0eee9f5d4f857ff815c8a91fe2ae561c9822bff0631a2372c63.
Passing fmt logs are empty SHA256e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855.
No runtime test result is claimed. Coordinator-reported model-capacity service
interruptions and an adversarial reviewer capacity error were retained separately
from product results. SAME settings/issue/checkout/fixtures/reviewers were retained
without recreating them; current receipts were inspected before further actions.

No local native/runtime/test/parser/worker/server/benchmark/JVM/oracle execution
is admitted; #96 remains OPEN/UNKNOWN. Format/compile-only Clippy receipts will
be recorded without claiming runtime success. Existing unchanged Linux Native
Rust all3 on actual final reviewed combined head/tree and exact merged main,
root-assigned final integration base, expected-SHA merge-window and same-reviewer
final readbacks remain required. No passing subset closes #71/#17 or establishes
full Kotlin LSP parity, semantic nullability, generic applicability or Darwin
reliability. T008 HOLD; B3/B4 GAP; B11 UNVERIFIED.

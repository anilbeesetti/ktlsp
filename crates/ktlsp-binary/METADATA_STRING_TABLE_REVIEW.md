# #125 review record

Contract-first stage. Source base6b76477/tree920b14a is frozen. Architect reviewed
raw-input/shared-budget plan before implementation. Two independently reserved
reviewers derived schema/fixture expectations from pinned source, without runtime.
Contract/helper admission and full independent fixture review remain pending.
No implementation or runtime validation is claimed. Keep source-derived corrections
and any later runtime failures in separate entries here.

Source-derived preimplementation clarification (architect, before freeze/runtime):
requested capacity now uses deterministic logical1/2/4 growth from Vec length,
charging even allocator surplus/no-op reserves. This removes allocator dependence
from exact B expectations without an output wrapper. Added independent duplicate
predefined/string, packed replace, wrong-wire operation and unknown-order fixtures.
Both reviewers had independently confirmed basework34+capacity and original values.
No empirical correction or native runtime execution.

Preimplementation freeze recheck: same correctness and adversarial reviewers
approved all revised contract/byte expectations with no findings. Own architect
confirmed plan comments addressed. Root implementation/helper/export admission
remains required. No source or runtime validation is represented as complete.

Root PhaseA conditional admission clarification before implementation: requested
retained capacity increments are not allocator churn/RSS or temporary old/new
buffer accounting. Cursor boundaries derive only from validated payloads; helpers
are called before EOF, and helper errors immediately propagate without resetting
consumed budgets. Same-two plus architect recheck of this wording pending.

Source-derived test transcription correction before ANY runtime: additional wrong-wire
fixture body is28 bytes (5+9+2+5+2+5), length byte corrected1b->1c by manual count.
This fixture was added during test authoring; frozen original expectations unchanged.
No test execution informed this correction.

Pre-runtime same-two/architect finished source review found no decoder/helper
correctness/safety blockers. Added reviewer-independent packed signed/nonminimal
fixtures, exact unknown count and nested failed-read work probe before runtime.
Adversarial arithmetic explanation corrected rootwire5->6 before execution;
expected thresholds16/17 unchanged. Local fmt/compile-only workspace Clippy passed.
Stale metadata/mod.rs summary correction awaits root's export-scope amendment.

Root admitted exact metadata/mod.rs two-line summary correction and additiveexport.
It describes StringTableTypes data and excludes name/type/semantic interpretation.
Root now requires held source approval before any push/PR/CI until one assigned
stable integration turn. Same-two/architect complete-candidate readback pending.

Complete-candidate held-source approval: same correctness and adversarial reviewers
and own architect actually read back decoder/helper/all nine tests, corrected module
summary, spec/expectations/provenance and notices. No actionable findings remain.
All three notice files match existing wire notices byte-for-byte; Unit.class hash
matches existing provenance. Source SHA256d9a5d1be30e5be86c0ca5610773d60439d7a3c2f943c3cef2656866180b8ba73;
tests5a1a6f822a61a319a2a17f7909adabaea3d9e694826a9a33a554df27b52ae7e9;
wireff31d666c143dbc9a5b0282ad933e8776ce2991d68451e73cf962392bf20043b.
Local format and full-workspace compile-only Clippy passed after final test additions.
No local native tests/runtime ran. Publication/CI/integration remain HELD for root's
one stable integration turn; no runtime/CI/merge/delivery approval is implied.

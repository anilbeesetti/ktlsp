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

# Selected TypeParameter review and validation

Child #173; frozen base941c72efd9896b0847ab342ec0dd22c10a000f3b.
Own architect approved plan and corrected finished preparation. Separate retained
correctness/functionality and adversarial reviewers approved CONTRACT v1 and
FIXTURES v2 before source. The v1 prefix2 catch-all was corrected to MissingName@2;
work/owned hard clamps are inspected, byte/record clamps runtime-testable. Initial
bool/span transcription corrections came from pinned source, never implementation.

Source adds one data-only record decoder, ten dedicated tests, three authored hex
fixtures, source notices and spec. Only shared changes are the additive module
export (root handoff released) and pub(super) visibility for existing push/integers; no
helper behavior or existing public API changes. Shared wire/table regressions,
Cargo/lock/workflow/parser/server/root status remain unchanged. Raw64 Boolean,
known-enum retention, required EOF precedence, borrowed body spans and aggregate
packed/output accounting follow the preimplementation contract.

Architect and both retained independent reviewers approved actual source with no
actionable findings. Fixture hashes and copied notices unchanged. Local format
passed; locked workspace all-target compile-only Clippy passed with -D warnings.
Original unchanged Linux full-workspace runtime CI pending. No local native product/tests/parser/
worker/server/bench execution under #96. SAME reviewers plus architect must read
changed source and completed original CI before root-exclusive expected-head merge.

No Type/Annotation/extension recursive initialization, name/type lookup, generic
semantics, server integration or full metadata/LSP parity. Requested retained
capacity excludes allocator surplus/churn/RSS; header/d1/table/resolver budgets
separate. T008/T008a HOLD, broaderC13/B3/B4 GAP, B11 UNVERIFIED, #7/#83/#96 OPEN,
Darwin UNKNOWN. Parents #18/#68 remain open.

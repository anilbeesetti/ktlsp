# Preparation provenance and review

This packet was authored before any lowering implementation or execution.
Numeric ranges were derived from literal original UTF-8 bytes, not captured from
a first passing implementation. Reference repository commit:
`941c72efd9896b0847ab342ec0dd22c10a000f3b`.

The existing `baseline_receivers_and_locals` declaration assertion establishes
the selected kinds and BLOCK ownership pattern; `malformed_parameter_lists_restore_original_snapshot_declarations`
establishes the incomplete named function/healthy-sibling pattern and end-of-header
zero-width `Expecting ')'` error. These are source-level inputs at
[declarations.rs](../../../crates/ktlsp-parser/tests/declarations.rs), not fresh
runtime evidence for this packet's exact files.

[Parser source](../../../crates/ktlsp-parser/src/lib.rs) defines immutable
SourceSnapshot/tree_for association, original-source byte remapping, rooted flat
preorder/parent/subtree validation, exact token partition and one syntax diagnostic
per ERROR_ELEMENT in preorder. The worker serializes these syntax facts; it
implements no declaration lowering. These existing constraints motivate the
prospective acceptance rules; error retention/incomplete observations in this
packet are newly proposed fixture requirements, not a delivered HIR policy.

The owner architect (`/root/architect`), correctness/functionality reviewer
(`/root/correctness`) and separate adversarial reviewer (`/root/adversarial`)
approved preparation only with no actionable findings. They independently checked
source hashes/lengths, all ten declaration/name ranges, UTF-8 boundaries, ownership
and the zero-width error; source inspection supports the documented CST/snapshot
constraints. Data-only relative-link checks passed for the six proposed paths.
No source admission, publication, PR or completed CI approval is recorded here.

Residual limits: two LF fixtures only; no CRLF/lone-CR, absent-name or repeated
error-range fixture, no runtime validation, no lowering budget/API implementation,
no symbol identity/resolution/provider/full grammar or parity evidence. T008/T008a
remain HOLD; existing parser adoption and Darwin gates are unchanged.

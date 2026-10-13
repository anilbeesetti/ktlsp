# #138 typed metadata payload composition

Stage2 status: the coordinator admitted this finite source/helper/export scope and
published #138. The frozen Stage1 contract below is preserved as historical
acceptance text; its implementation/publication HOLD has been superseded only for
source work and the issue. Integration/push/PR/CI/merge remain held. No local runtime
is permitted. Formatting and compile-only Clippy are allowed; tests below are
authored coverage, not executed evidence. Parents #68/#18 remain OPEN.

# Typed metadata payload composition: stage1 contract (historical freeze)

Frozen source: 64234587e333ebcd32d96600a7a193b06b0ac9df; tree 3f6828ce85a8b74a9142c4fd6d9893ad107210d3.
Own independent clean detached checkout: /private/tmp/ktlsp-payload-contract-01a13424.
This is an auxiliary proposal only. No source admission, publication, runtime, Cargo or JVM execution.
Child under OPEN #68/#18. Delivered #98/#112/#128/#135 are reused, not reimplemented.

## Exact public API proposal

New metadata::payload module exports `pub use super::string_table::Limits;` and:

```rust
pub struct Payload<'a> {
    pub prefix_span: std::ops::Range<usize>,
    pub prefix: &'a [u8],
    pub string_table_span: std::ops::Range<usize>,
    pub string_table: super::string_table::StringTableTypes<'a>,
    pub payload_span: std::ops::Range<usize>,
    pub payload: &'a [u8],
}
pub fn decode(bytes: &[u8], limits: Limits) -> Result<Payload<'_>, super::wire::Error>;
```

Payload derives Debug, PartialEq, Eq, matching existing result conventions.
Input is ALREADY DECODED d1 bytes, supplied only after independently checking the existing supported header outcome (k1/2/5, mv[2,3,0]). This API cannot enforce header kind/version or determine class versus package. No kind parameter or schema reconstruction.
Leading existing rawVarint32 delimited StringTableTypes is typed; ALL remaining bytes are borrowed wholly opaque. NO flat-wire validation, field vector, reads or work debit for the remaining payload. Invalid tags, groups, arbitrary bytes and truncated class/package messages in that remainder succeed. Total input byte ceiling includes those bytes. Empty input fails framing; [00] succeeds without establishing valid Class/Package metadata.
Prefix, string_table_span and payload_span are d1-relative half-open spans. Prefix/payload and table.raw borrow exact original bytes, including duplicates/nonminimal encodings. No duplicate raw table field/vector. Record.span and unknown Field.span INSIDE the reused typed table remain table-relative exactly as in string_table::decode; convert by adding string_table_span.start when needed. All NEW API errors are d1-relative, including root table failures at an empty table's start. No name resolver/header execution or budgets folded into this operation.

## Exact minimal private helper proposal (root gated)

wire.rs:
- Add `pub(super) struct Envelope { pub(super) prefix_span: Range<usize>, pub(super) string_table_span: Range<usize>, pub(super) payload_span: Range<usize> }`.
- Add `pub(super) fn envelope(bytes: &[u8], budget: &mut Budget) -> Result<Envelope, Error>` using existing Reader with pos0/end bytes.len(), existing record(0), length(), boundary(length). Return ranges; no field traversal/allocation/byte check/new framing code. Prefix ends at Reader.pos and table ends at checked boundary.
- Existing public wire::decode retains its initial byte guard and creates its wire Budget exactly as before. Call envelope with that budget, then same Reader::section for table and payload from envelope.string_table_span.start. No public type/error changes or new charges. Existing wire callers/tests retain behavior, including absolute offsets and precedence.

string_table.rs:
- Make existing Budget and its existing wire field `pub(super)`; nodes/depth/owned remain private.
- Add `pub(super) fn new(limits: Limits) -> Self` on Budget containing exactly the current initialization/clamping.
- Extract current body beginning budget.node(1,0) into `pub(super) fn decode_with_budget<'a>(bytes: &'a [u8], budget: &mut Budget) -> Result<StringTableTypes<'a>, Error>`; remove now-extra `&mut` at call sites. No new checks, loops or changed charges.
- Existing public decode retains byte check, initializes Budget::new(limits), then delegates. Existing table/public resolver callers/errors stay table-relative with unchanged budgets.

payload.rs:
- Check entire bytes length against limits.bytes.min(MAX_BYTES), error Limit(bytes) at0.
- Create ONE string_table::Budget::new(limits), call wire::envelope(bytes,&mut budget.wire), then string_table::decode_with_budget on the validated table slice using SAME budget. Never call public wire::decode/string_table::decode.
- Rebase ONLY the table decoder's error offset by table start, exactly once (wire envelope errors are already d1-relative). Use checked_add; unreachable arithmetic failure maps to existing Limit(bytes) at bytes.len(). All normal table errors have offset<=table.len(), and checked table start+table.len()<=input.len()<=16MiB. Offset translation costs no additional budget; it is constant bounded bookkeeping.
- Construct borrowed spans/slices without scanning payload. No public common error, header/name_resolver/lib/dependency changes.

metadata/mod.rs: additive `pub mod payload;` only (module summary may add composition). No reexports displacing existing APIs.

## Frozen accounting, order and limitations

Reuse string_table::Limits, hard ceilings bytes16MiB, records65,535, nodes65,535, depth2, work64Mi, owned_bytes16MiB; caller can only tighten. Applies to THIS composition call alone.
Order: total byte guard; envelope record+work, prefix reads, envelope availability; table root depth/node/work; typed table fields/nested records/packed ints/reservations; result borrowing. Envelope is one record, NOT a schema node/depth. Opaque payload contributes no records/nodes/depth/work/owned capacity. No budget reset/refund. Failed reads/validation/reservations retain debit and return immediately, no retries or partial result.
Existing wire rules apply unchanged to prefix and table: low32 tag/length truncation, nonminimal <=10 byte encodings, negative signed low32 lengths fail, tenth continuation malformed, field0 before wire validation, groups unsupported in typed traversed fields. Successful envelope availability charges no content; failure charges available bytes+1 before Truncated. Table scanning pays wire record1/work1 + each read byte + dispatch1; nested Record body and packed values reread under same budget. Packed elements each record/work1 + read bytes + interpretation1; unpacked repeated ints interpretation1. Raw strings/unknown bytes remain unvalidated UTF8/opaque, no recursive unknown traversal.
Table root depth1/node/work1 even if empty; known Record depth2/node/work1; no recursive class/package depth. Depth before node before work. Field failures before dispatch. Output logical capacity increments at len0 and positive powers of2: slots=len.max(1), checked len+slots and slots*sizeof(T), owned debit then work debit then try_reserve_exact(slots). Allocation rejection Limit(allocation); no partial output. Charge even if allocator surplus makes reservation a no-op. No extra envelope/output vectors. Stack Payload/Envelope structs and borrowed spans are outside owned capacity.
Owned bounds requested RETAINED logical vector backing capacity for Record/Field/i32 slots. Does NOT bound allocator surplus/rounding/bookkeeping, realloc churn/old-new coexistence, stack/RSS, wall CPU or header/d1/resolver/wholepipeline work. No UTF8 validation or speculative lengths allocation. Error precedence/offset labels match frozen existing wire/table contracts except documented single table error rebase.

## Owned paths and integration reach

Prospective exclusive paths: crates/ktlsp-binary/src/metadata/payload.rs; crates/ktlsp-binary/tests/metadata_payload.rs; crates/ktlsp-binary/tests/fixtures/metadata_payload/**; crates/ktlsp-binary/METADATA_PAYLOAD_SPEC.md; crates/ktlsp-binary/METADATA_PAYLOAD_REVIEW.md.
Root gated exact shared paths: wire.rs, string_table.rs, metadata/mod.rs as above. Any additional helper/API/path requires proposal.
Callers: new module tests only; existing wire and string_table tests are regression coverage; name_resolver tests indirectly cover unchanged table wrapper. No runtime/server caller. Existing specs are untouched; new spec documents differences precisely. No Cargo/lock/config/workflow changes.
No declaration/type/class/package/schema semantics; parents #68/#18 remain OPEN, T008/T008a HOLD. #96 OPEN/UNKNOWN. Full parity and official speed/RAM comparison unverified.

## Validation gates (future only)

Hand-authored fixtures and expectations in FIXTURES.md precede implementation/runtime. Keep failures, do not tune assertions to outcomes. No local Cargo/fmt/compile/runtime during Stage1. Later source grant may admit fmt/compile-only Clippy; runtime only unchanged Linux Native Rust fmt/Clippy/full-workspace tests on ONE root-assigned final combined head/tree. Same two reviewers + own architect must review actual source and completed PR CI before guarded expected-head merge after root exclusive grant. Owner/root independently verify same reviewed squash tree/sole parent/workflow/executor and exact-main all3 after merge. No fresh workflow/executor or JVM. This contract approval is not source/CI/merge/delivery approval.

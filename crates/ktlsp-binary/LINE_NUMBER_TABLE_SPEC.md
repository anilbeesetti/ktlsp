# Applied source contract

The frozen preparation contract below was admitted for its exact eight paths on 2026-10-14. Contract SHA256 `06036bd6628b39996d400856a2fdedcf55f8244bbec14c245cd42e558c7b03a5`; expectations SHA256 `5c70887c5262fe40b1ba2fc164a438c8edaa5666191ea04bfc8b8d84fa79e4db`. Its prospective/source-held wording records the pre-source stage. Source admission now includes the sole additive export; publication, local runtime and hosted CI remain held. Behavior and literal expectations are unchanged.

# T017 / #17 / #71 child: bounded LineNumberTable data

Preparation only, based on clean delivered `3a93659d6fa6ad252298684ead15efd32ee6ab7b`, tree `bcf3179f95721f29dae4afb8eaf15001649d122b`, sole parent `28ff42d743b6290b9a1e587e36eced80cebd6438`. No source, export, publication or runtime admission. This contract and its literal expectations precede implementation.

## Scope and API

Add one sibling `line_number_table` module. Existing Code framing deliberately leaves all nested attributes opaque. Decode every nested LineNumberTable in ONE caller-supplied `code::CodeBody`, preserving selected-table order and each entry's order and repetitions. Do not scan other CodeBody instances, methods, fields, class attributes or Record. Repeated calls have separate budgets. Code framing and classfile parsing retain their separate budgets. No combined framing+LNT entry point is proposed.

Prospective public types (no source written yet):

- `Limits { bytes: usize, records: usize, work: usize, owned_bytes: usize }`, Clone/Copy/Debug. Default hard caps are crate MAX_BYTES (16777216), MAX_RECORDS (65535), MAX_VALIDATION_UNITS (67108864), MAX_BYTES respectively. Each caller value clamps independently, including zero and usize::MAX.
- `LineNumber { start_pc: u16, line_number: u16 }`, Debug/PartialEq/Eq.
- `LineNumberTable<'a> { name: u16, raw: &'a [u8], entries: Vec<LineNumber> }`, Debug/PartialEq/Eq. `name` is the original selected Attribute.name; `raw` borrows its entire payload (no six-byte outer header).
- `decode<'a>(body: &'a crate::code::CodeBody<'_>, cp: &[crate::Constant], limits: Limits) -> Result<Vec<LineNumberTable<'a>>, crate::Error>`.

Only new tests call this API. No existing caller, struct layout, helper, Cargo/dependency/lock/config/workflow/vendor/server/parser/metadata/public-status file changes. A later separate exact root grant is needed for `pub mod line_number_table;` in lib.rs. Reuse ancestor-private Reader take/u16, utf16 and eq directly; Rust descendants can access these, so no helper exposure/extraction is needed. Existing code::Budget remains private and unchanged: its labels/scope/storage policy differ.

No input mutation or partial result on error. `raw.as_ptr()` must equal selected Attribute.bytes.as_ptr(), even for empty retained slices where applicable. Entries copy only numeric u16 data. No UTF16/string/payload copy. Repeated/aliased payloads remain distinct ordered output tables. No public byte offsets are added: Error has none. Inventory offsets are half-open absolute class or payload-relative spans, explicitly labelled.

## Pinned format and deliberately finite validation

Use existing crate baseline JVMS Java SE23, §4.7.12, checked 2026-10-14: https://docs.oracle.com/javase/specs/jvms/se23/html/jvms-4.html#jvms-4.7.12 . Specification permits multiple attributes in any order. Its entry fields are u2 start_pc and u2 line_number. There is no sorting/uniqueness constraint here. Preserve empty tables, decreasing PCs/line numbers, identical tuples, repeated PCs with different lines, repeated lines with different PCs, line_number zero and 65535. Match only exact 15-unit UTF16 `LineNumberTable`; no Unicode normalization/case conversion.

The payload is big endian: u2 count followed by exactly count repetitions of (u2 start_pc, u2 line_number), with exact EOF. Numeric start_pc must be < body.bytecode.len(). No opcode or instruction-boundary validation. Bytecode contents are never read. No PC-to-line lookup, stepping, source-file association, SourceDebugExtension, semantic or JVM verification claim.

Public CodeBody/Constant values may be forged. Check cp length 1..=65535 (else Malformed("line number constant pool size")), then bytecode length 1..=65535 (else Malformed("line number code length")), even with no attributes. No class version is available at this API boundary; classfile/Code callers retain their own version checks. Do NOT validate body.raw, max_stack/max_locals, exceptions, raw-to-bytecode/attribute containment or unrelated CP entries/slot-zero shape. These fields are unused; success certifies the selected data only, not CodeBody provenance or classfile validity. Attributes may borrow unrelated buffers safely. Every visited attribute name is checked even when its payload/name will be ignored. Zero name always rejects, including forged Utf8 at slot0; out-of-range/Unusable -> Malformed("constant index"), other kinds -> Malformed("UTF8 index"). Arbitrary valid UTF16 including NUL/lone surrogates/empty names remains unknown and accepted.

## Exact stage/error precedence

Stop on first failure; drop all temporary output.

1. CP length, then bytecode length prechecks. Initialize one aggregate budget for this whole call. Charge records(body.attributes.len()) before any attribute loop. No output reservation for zero selected tables. Empty attributes with valid CP/bytecode succeed under four zero limits.
2. For each nested Attribute in order: work(1), bytes(payload.len()), work(payload.len()), then text(name). Text means work(1) for lookup, explicit zero rejection, crate::utf16, then work(full UTF16 length) before comparing via crate::eq. Thus payload limits precede invalid names and all text work is charged even if matching exits early. Unknown payloads remain wholly opaque but are conservatively charged once as bytes/work. Aliased payloads and repeated names charge each occurrence.
3. For exact LineNumberTable: checked charge size_of::<LineNumberTable>() logical output storage, then out.try_reserve(1), BEFORE reading its payload. This charges each retained table once and reserves enough for its imminent push; capacity surplus is excluded. Reservation failure -> Limit("line number allocation"). Amortized reservation avoids the exact-one growth pattern that permits quadratic copying with a moving allocator; no allocator timing or RSS bound is promised.
4. Reader { bytes: attr.bytes, at:0, records:0 }; only take/u16 primitives. Read count u16 (missing -> Truncated). Charge records(count), then checked count*size_of::<LineNumber>(), charge storage, try_reserve_exact(count) on fresh empty entries Vec (zero skips reserve). Count/storage limits precede missing entry bytes. No minimum-length preflight ahead of these count charges.
5. For each entry: work(1), read both u16 fields in order, then reject start_pc >= bytecode.len() with Malformed("line number start pc"). Missing line_number is Truncated even if the already-read PC would be invalid. Retain all u16 line_number values unmodified. Push the entry only after validation.
6. After count entries, require Reader.at==attr.bytes.len(), else Malformed("line number trailing bytes"). Push selected table. An earlier malformed selected payload precedes later unknown-name/duplicate/budget failures: parsing is immediate, not a second-pass scan. Identical selected names, including distinct CP slots with same UTF16 content, are allowed.
7. Return Vec in encounter order. Reader.checked slicing/offset arithmetic returns Truncated. No unchecked indexing of CP, payload or bytecode, recursion, descriptor lookup or hidden re-decoding.

## Aggregate budgets and storage

Counters zero ONCE, each checked_add; overflow or exceeding clamped limit returns Limit("line number bytes"), Limit("line number records"), Limit("line number work"), Limit("line number owned bytes") respectively. Record and storage budgets stay distinct. Reader.records is unused.

For a successful call, with A visited nested attributes, E selected entries and T selected tables:

- Bytes B = sum of EVERY nested attr payload.len(), including ignored payloads. Excludes body.raw, bytecode, exceptions and CP storage (unused/borrowed). Aliases charge repeatedly.
- Records R = A+E. Each selected table is already one visited-attribute record; do not double charge it. A is charged up front, E per selected table count before allocation/read.
- Work W = B + 2*A + sum of each visited name's UTF16 length + E. One attribute visit + one lookup + full text length; opaque bytes are conservatively charged once; one selected entry step. No method/body work count or per-scalar double charge.
- Logical retained storage O = T*S + E*L, where S=size_of::<LineNumberTable>() and L=size_of::<LineNumber>(). Charge table storage one at a time before its first payload read; charge each entries vector count before allocation. checked_mul overflow -> owned-bytes label. Fresh entries vector reserves once. Output append uses fallible amortized try_reserve(1), no infallible growth beyond reserved capacity. Logical elements requested exclude capacity surplus, allocator metadata, input buffers, temporary stack, transient allocations, whole-pipeline peak and RSS.

Default/hard caps cannot be raised by caller. With all other limits adequate: each exact required B/R/W/O succeeds, and independently one less fails with that named limit. Zero minima stay zero (no underflowing test). Multi-table/unknown-interleave fixtures prove there is no reset, including empty selected tables. Private budget tests preseed usize::MAX to cover checked-add overflow and count*size multiplication overflow without enormous public allocations. Allocation failure handling is statically reviewed; no custom allocator or claimed real OOM recovery test.

## Evidence and delivery boundary

EXPECTATIONS.md and authored.json freeze literals, malformed/prefix/boundary cases and minimum arithmetic; genuine-unit.json records the independent byte walk of the existing unchanged licensed Unit.class (781B, SHA256 896588212a98f36148f2fbe1ea2878d6ba23c6376c6346c765cec5ac0c5716ce). Only two genuine tables, each single-entry, exist here: init (0,13) and toString (0,14); clinit has none. All broader cases are explicitly authored. No download, JVM/compiler/oracle/product decoder execution was used.

Future native genuine test MUST parse the actual include_bytes Unit.class through parse_class -> code::decode -> this API for all three bodies; assert full original method/Code/nested tuples, selected name38/raw spans/entries and pointer identities against the original class allocation, as well as immutable inputs. Literal payload tests remain distinct and cannot substitute for this pipeline. Each body's API call has its own LNT budget; no total-of-three-body bound is claimed. Code framing retains its independently frozen 121/11/270 and storage3*OptionCodeBody+4*Attribute arithmetic.

T017/#17/#71 stay open; T008/T008a HOLD, C13/B3/B4 GAP, full B11 UNVERIFIED, Darwin/#96 UNKNOWN, semantic/platform/performance/full parity unfinished. Repo public docs at this base still assess 048c344; root's separate docs243 assessment28ff597 and delivered3a9 proof must not be conflated. Root delivery-clear hash d907de7c6b0ea5d583a1403e2c75a568bcd054542f9a1c5567b318d103f54bba establishes the assigned base evidence, not LNT validation. Queue237->239->243->241 unchanged.

Preparation SAME3 (architect, correctness, adversarial) review contract/data only. Source/export requires later exact root grant and renewed source reviews. Then pinned format/locked all-target Clippy/original hosted full workspace CI and original exact-main proof are required; no local native/runtime under #96. Preserve first failures/no output tuning/no unchanged rerun/bypass/admin/main-force/settings change. Exclusive base/head lease, standalone read-only exit0 completePASS guard, separate ordinary expected-head merge, squash tree/sole-parent verification and independent OWNER+ROOT original exact-main proof remain delivery gates. No current source/CI/merge authority follows from READY.

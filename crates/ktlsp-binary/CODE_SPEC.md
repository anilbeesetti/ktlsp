# T017 / #17 / #71: bounded Code attribute framing

Frozen preparation contract SHA-256 `c52da303d8aa37f303312dbaf9d87e06aa18d532b0fc509dc6f98876322f2fd0`, independently reviewed before source. Initial admitted source base `e3a79570895d8cd0af4681424eebca819e725361`, tree `4752d1895447a839a8510d17cc50f153a4ca6055`, sole parent `015057d40ea8739502e179b246b9385be016b5b2`. The initial integration changed only six checkpoint docs; all other888 original source/data pins were preserved. This version records the applied API and the same frozen behavior. Preparation approvals do not establish implementation/runtime correctness.

## Smallest useful contract

The current ClassFile reader bounds the outer Attribute payload only. Add one isolated, data-only endpoint to frame Code interiors. It selects ALL class.methods, in their existing order; no selector, additional pipeline, or annotation decoding. Return one Option body per method, including None for ordinary abstract/native methods. Duplicate identical method records are independent ordered inputs, not deduplicated. No field/class/Record attribute scanning.

Public API:

- code::Limits { bytes: usize, records: usize, work: usize, owned_bytes: usize }, Clone/Copy/Debug; Default uses crate MAX_BYTES=16777216, MAX_RECORDS=65535, MAX_VALIDATION_UNITS=67108864, MAX_BYTES respectively. Caller limits clamp independently to those hard caps, including zero; usize::MAX cannot raise a cap.
- code::ExceptionHandler { start_pc: u16, end_pc: u16, handler_pc: u16, catch_type: u16 }, Debug/PartialEq/Eq. catch_type zero remains zero (catch-all).
- code::CodeBody<'a> { raw: &'a [u8], max_stack: u16, max_locals: u16, bytecode: &'a [u8], exceptions: Vec<ExceptionHandler>, attributes: Vec<crate::Attribute<'a>> }, Debug/PartialEq/Eq.
- code::decode<'a>(class: &'a crate::ClassFile<'_>, limits: Limits) -> Result<Vec<Option<CodeBody<'a>>>, crate::Error>.

Borrow raw and bytecode from the supplied Attribute, and nested payloads from raw. Retain exact nested name indices and source order. No borrowed text is copied. Existing ClassFile, Member, Attribute, parse_class and annotation/metadata APIs stay unchanged. Any failure drops all temporary output and returns no partial result; input is immutable. No public byte offsets or new error wrapper: existing Error has no offset, and Attribute does not retain absolute class offsets. In inventories only, spans are half-open, Code-payload-relative unless explicitly marked absolute class spans; PCs are bytecode-relative.

Consume only version, CP shape, method access_flags/name, outer method attribute names/payloads, Code scalar/count/framing fields and consumed nested name/catch indices. Names are lossless UTF16 from crate::utf16, matched by crate::eq without String allocation; arbitrary valid UTF16 names (including empty/NUL/surrogates) may remain unknown. Method name is only used to distinguish exact <clinit>; member-name grammar/descriptors, unrelated pool entries, Class name links and unrelated class state are not revalidated. Forge-safe consumed CP checks apply even to hand-built public structs.

Supported version predicate is exactly the existing reader: major=45, minor=0..3; or major=46..67, minor=0. Other/preview versions return UnsupportedVersion, even with no methods. CP vector length must be 1..65535, otherwise Malformed("code constant pool size"); slot zero must not be used as a name/catch index regardless of its forged contents. Unconsumed slot-zero shape and other CP contents are not validated.

## Disposition, framing and error order

Source reference: [JVMS23 §4.7.3](https://docs.oracle.com/javase/specs/jvms/se23/html/jvms-4.html#jvms-4.7.3), with bounded framing deliberately narrower than JVM verification. Spec check 2026-10-14; no new artifact/oracle research. Note the special initialization-method qualification in that rule: exact <clinit> requires Code even with native/abstract bits in a forged model. These flag combinations are not otherwise certified legal.

Run stages in this exact order, stop at the first failure:

1. Check version, then CP length. Create one budget for the entire operation. Charge records for class.methods.len(); reserve the output vector under retained-storage accounting before the method loop. Zero methods returns an empty vector with no charges beyond checks.
2. For each method, charge work(1). Resolve its name through text operation below. required = exact name <clinit> OR access_flags & 0x0500 == 0 (ACC_NATIVE=0x0100, ACC_ABSTRACT=0x0400). Charge records for method.attributes.len() before visiting that list.
3. In outer attribute order, charge work(1), bytes(payload.len()), work(payload.len()), then text(name). Unknown payloads remain unexamined but are charged. For a recognized Code: if !required, return Malformed("unexpected Code attribute"); otherwise if previously seen, return Malformed("duplicate Code attribute"); otherwise remember its borrowed payload. Scan ALL outer attributes before parsing the remembered payload. Thus a duplicate, invalid later name or later outer budget failure precedes a malformed first Code body. Distinct CP slots containing Code are duplicates by text. Unknown duplicate names are allowed.
4. After scanning, required with no Code returns Malformed("missing Code attribute"); !required with no Code appends None. Required with one Code parses it, then appends Some(body).
5. Reuse crate::Reader with at=0, records=0; call ONLY take/u16/u32. Read max_stack, max_locals and code_length, in order. Require 1<=code_length<=65535, else Malformed("code length") before taking bytecode. Length conversion uses usize::try_from(u32), failure Truncated. Read the exact bytecode slice, then exception_table_length u16. Charge that many records and reserve exception vector before reading entries.
6. For every exception in order: work(1), then read start_pc/end_pc/handler_pc/catch_type as four u16 values. Check start_pc < end_pc <= bytecode.len() AND handler_pc < bytecode.len(); otherwise Malformed("code exception range"). end_pc==code_length is allowed. Only after range checks, if catch_type!=0: work(1), crate::constant lookup (zero/unusable/out-of-range -> Malformed("constant index")); wrong kind -> Malformed("class index"). Constant::Class(any u16), even a forged bad name link, is accepted here; no Throwable/array/name/assignability validation.
7. Read nested attributes_count u16; charge records(count), reserve Vec<Attribute> before entries. Each entry: work(1), read name u16, text(name), read length u32 and usize::try_from (failure Truncated), then Reader::take(length), retaining Attribute { name, bytes }. Nested payload bytes are already included in outer Code payload; do NOT double charge bytes/work for them. All nested names, including Code/StackMapTable/known annotation/debug names, are opaque: no recursive Code decoding, context, version or duplicate rules. Duplicates/zero-length unknown payloads allowed.
8. Require reader.at==raw.len(), otherwise Malformed("code trailing bytes"). All bounded reads retain Reader's Error::Truncated on missing data or checked offset overflow. A valid code_length with insufficient bytes is Truncated, not a code-length error. Reader never allocates bytecode/payload copies.

Text operation order: work(1) for CP lookup, crate::utf16 (using crate::constant, rejecting zero explicitly even forged UTF8 at slot0), work(text.len()) BEFORE scanning/comparing UTF16. Existing errors are Malformed("constant index") for zero/out-of-range/Unusable; Malformed("UTF8 index") for other CP kinds. Work exhaustion may precede content matching. No UnsupportedTag is needed.

## ONE aggregate budget, exact arithmetic

Four checked counters start at zero ONCE. Each increment uses checked_add; overflow or total > clamped limit returns Error::Limit with labels "code bytes", "code records", "code work", "code owned bytes" respectively. Failed counters need not be reused. No per-method, per-Code, per-exception or nested resets. Reader.records is unused. Logical requested storage and records are separate, never silently combined.

- Bytes = sum outer method attribute payload lengths visited (Code and ignored). Repeated/aliased input slices count per occurrence. Nested payloads and bytecode are already contained and never double counted. CP UTF16 is borrowed and charged only as work.
- Records = methods + outer method attributes + exception entries + nested attributes. Counts are charged before their loops and allocations, including declared counts whose later bytes are absent.
- Work = one per method, outer attribute, exception, nested attribute; plus one per referenced UTF16 lookup + its UTF16 length; plus one per nonzero catch lookup; plus every visited outer attribute payload byte once. The last charge conservatively covers all Code framing/bytecode/nested bytes even though opaque bytes are skipped. No per-scalar duplicate work. Version/CP length, checked arithmetic and vector reservation are O(1) operations outside work.
- Owned bytes = SUM of n*size_of::<T>() requested for exactly three vector families: outer Vec<Option<CodeBody>>, each exceptions Vec<ExceptionHandler>, each nested Vec<Attribute>. checked_mul overflow -> Limit("code owned bytes"); charge before try_reserve_exact(n) on a fresh empty vector; reservation failure -> Limit("code allocation"). Zero requests do not reserve. Each vector reserves exactly once, then pushes at most n items. Excludes borrowed input/CP buffers, stack values, allocator surplus/metadata, peak total process memory and RSS. No claiming physical-capacity or whole-pipeline bounds. Private preseeded-budget/checked-arithmetic tests may exercise usize overflow; no public custom allocator.

Order when reserving: records count first (where specified), then checked storage multiplication, storage charge, fallible reservation. Work for individual entries comes later. Fixed payload byte work is charged before reading it. Successful count/length arithmetic cannot wrap. No independent nested-reader record budget, annotation passes or metadata budgets enter this call.

## Independent inputs and expected cases

See tests/fixtures/code/EXPECTATIONS.md and authored.json (literals/spec-derived values prepared before source); An independent data-only walk confirmed the unchanged licensed 781-byte Unit.class, SHA256 896588212a98f36148f2fbe1ea2878d6ba23c6376c6346c765cec5ac0c5716ce. Its genuine exception tables are EMPTY. Nonempty tables below are authored; never represent them as genuine/compiler output. No Rust implementation, JVM, compiler or oracle produced these expectations.

Every proper prefix of each valid authored payload and each of three genuine Code payloads must return Truncated under adequate limits, with unchanged method/CP framing. Full payloads succeed; appending one byte returns code trailing bytes. Prefix tests operate on public ClassFile attributes, not parse_class of a clipped full class. Full genuine input also receives parse_class->code::decode integration assertions when source/runtime is later admitted; test execution evidence is recorded separately in CODE_REVIEW.md.

Malformed inputs and budget precedence assertions are mandatory as frozen in the inventory. Finite deterministic bit mutations and forged containers must return a controlled Result, never panic; they provide finite safety evidence, not exhaustive validity. No future output-derived expectation tuning: disagreements require a preserved first failure and specification/data review before a revised contract.

## Source ownership

- ADD crates/ktlsp-binary/src/code.rs: public endpoint/types/private budget, private arithmetic/budget tests only where integration access cannot cover them.
- ADD crates/ktlsp-binary/tests/code.rs: focused literal/genuine/forged/prefix/aggregate/mutation coverage. No existing test edits expected.
- ADD crates/ktlsp-binary/tests/fixtures/code/README.md, EXPECTATIONS.md, authored.json: authored literal inputs, expected values/provenance; no copied new third-party class, no opaque unexplained hex directory. Reuse Unit.class + existing provenance/LICENSE/NOTICE read-only.
- ADD crates/ktlsp-binary/CODE_SPEC.md and CODE_REVIEW.md: ticket-local contract and actual final review/CI evidence.
- MODIFY crates/ktlsp-binary/src/lib.rs: ONE separately serialized additive line `pub mod code;` only. All existing functions/Reader/callers remain byte-identical. Root granted the sole additive export lane.

No Cargo/dependency/lock/workflow/config/vendor/metadata-mod/server/parser/public-status edits. No private helper extraction proposed: Reader primitive reuse needs none; annotation Budget/vector helpers are deliberately not suitable to these distinct counters. Callers touched are only new tests; no server consumer or provider/capability change. Existing tests/API/raw-attribute behavior must remain unchanged. Additive API imposes no existing struct layout/field migration.

## Remaining limits and delivery gate

Instruction boundaries, opcodes/operands, stack/local/parameter correctness, StackMapTable contents, nested attribute legality/multiplicity, Throwable assignability, control flow, Java/Kotlin semantics, library symbols and server consumers remain unchecked. T017/#17/#71 remain OPEN; no full parity or comparative performance claim. #96/Darwin remain unresolved; no local native execution here. Native product/build/core/default tests remain Rust-only, without JVM/JDK/compiler/JNI/C/C++ analysis dependency.

Root admitted exactly these eight paths after actual SAME3 contract/data review. Local validation is pinned formatting and locked compile-only workspace/all-target Clippy, with no local product/test/JVM execution. Hosted original Linux CI, actual retained SAME3 source/CI reviews, coordinator integration clearance, a separate expected-head/base lease and read-only guard are required before an ordinary expected-head merge. No source admission itself grants a merge lease.

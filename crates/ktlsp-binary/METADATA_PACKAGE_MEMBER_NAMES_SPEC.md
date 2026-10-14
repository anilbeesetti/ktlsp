# Package member-name composition — admitted contract

Coordinator admitted exact contract/data/source/private-helper/compile-only/publication/
original unchanged Linux CI on 2026-10-14. This preserves the preimplementation freeze
below, including its historical preparation-only HOLD. Source and completed CI review
are separate. Fixture README text uses paths relative to the temporary freeze packet;
PROVENANCE.json and EXPECTATIONS.md are preserved alongside it in the fixture directory.

Frozen22-file packet manifest SHA256795c9095384a7385a7202ef640757dce593b3f7d26d76f3c92d357fc95087099.
CONTRACT SHA25686f90cf8811f37f25f2be8c693a71024ad42e8e456507b5905cabb44140ba30d.
EXPECTATIONS SHA2565343c8aa8e6e57d0ca4a318247020985e5abc295fa88e461a0a4fae000e2f01c.

# Package member-name composition — contract v1, preparation only

Coordinator: 01a1300d-2ba0-7832-97d0-91d070e2d297/local.
Exact clean base 7980480e015150b75b982001c871ad1a5b54f35a,
tree e6a0d62d387f72b0ef1118109179cceec87cba40,
sole parent fe4f4a73f0bf9190f7668bf74a9b963f70114449.
Owned managed worktree /Users/anil/.codex/worktrees/t018-package-composition-contract/ktlsp,
branch codex/t018-package-member-names-contract. Packet is temporary data only.
Source/API/shared-helper/export/compile-only/publication/original-CI admission is
HOLD until the coordinator reviews the exact packet and grants exact paths.
No source/product/Cargo/workflow/root-doc changes, commits, publication or native
execution have occurred. Pinned research is reused, not restarted.

## Finite API

New metadata::package_member_names::decode(&metadata::Outcome, Limits)
-> Result<Vec<Member>, Error>. Require Outcome::Decoded, k in {2,5} and
mv exactly [2,3,0]. Absent/Unsupported/forged wrong-kind/version =>
ErrorKind::UnsupportedContainer at d1 offset0, before all other preflight.
KIND5 is selected part data only: xs/pn do not join facades or assign packages.

Limits contains bytes,records,nodes,depth,work,owned_bytes,entries,range_slots,
queries,output_units; defaults/clamps match delivered class_member_names Limits,
independently defined in the new module (no new public budget framework).
Member has Kind::{Function,Property,TypeAlias}, ordinal:usize, span/body_span:
Range<usize>, name:Vec<u16>, and flags/old_flags/getter_flags/setter_flags:
Option<i32>. Shapes follow delivered Class member output but have separate types.
Ordinal is position among ALL encoded Package fields, starting at zero. Both
spans are absolute in already-decoded d1; span includes outer tag/length, body_span
covers just the selected body. Raw data remains accessible through these spans in
caller's unchanged d1, not an owned copy. These are not SymbolIDs.

Only wire2 fields3/4/5 select Function/Property/TypeAlias. Preserve interleaving,
duplicate names, duplicate member records and unknown/wrong-wire fields in ordinal.
Function accessor flags=None; TypeAlias old/accessor flags=None. Raw flags preserve
presence/signed value: absent is None, even where the schema has a default.
No flag conversion/default injection/accessor inheritance. Alias IDs and inline
Types are not exposed as reconstructed types; original bytes/spans retain them.
Names retain UTF16 units including empty/NUL/lone surrogates; no normalization or
identifier validation. Name lookup uses the delivered pinned resolver.

Error { offset:usize, kind:ErrorKind }; kind is UnsupportedContainer,
Wire(wire::ErrorKind), Function(function_header::ErrorKind),
Property(property_header::ErrorKind), TypeAlias(type_alias::ErrorKind),
Name(name_resolver::Error). Preserve typed reader error variants, do not flatten.
All offsets d1-relative; nested reader offsets rebase by selected body start.
Name query errors use member BODY START and preserve requested index/record context;
resolver-constructor errors use table start. MissingName uses selected body end.
No Class MissingFqName gate applies to Package (no required parent scalar).

## One operation and deterministic precedence

Order is container guard; O(1) d2-entry preflight; combined byte preflight;
payload::decode_with_budget; Package node(depth1 at payload start); shared Resolver
constructor; ONE outer Package field scan in encoded order. Each completed outer
field pays dispatch work1. For wire2 selected field: member node(depth2 at body
start), selected shared reader scan, one shared resolver query, one output push.
Every success traverses the complete Package outer framing. First error aborts the
whole call; never return a partial Vec. There is intentionally no preliminary
outer Package scan: an earlier selected missing-name/name-index error wins over a
later malformed outer tail. Conversely any selected header's later structural
error precedes its own clean-EOF MissingName and name resolution. Typed table
framing errors precede resolver/member errors. This differs from Class's required
header prevalidation and is explicit, not a parity claim.

Wrong-wire member fields remain opaque/nonselected, as do Package type_table30,
version_requirement_table32, extensions and all other nested fields. Selected
Function/Property scan complete header wire framing with nested types/extensions
opaque. Selected TypeAlias scans complete framing and retains ALL underlying4/
expanded6 inline bodies per delivered reader, with full vector charges even though
this composition consumes only name/flags and drops those vectors afterward.
No full nested protobuf initialization, type merging, alias expansion or semantic
applicability is established. Opaque malformed nested bytes can succeed.

## Shared Budget proposal (before source)

Allocate exactly ONE existing string_table::Budget from the six matching Limits
fields, covering framing/table, Package/member nodes, all outer/member field reads,
capacity growth, UTF16 name work and name storage. Existing #193 seams are named
`decode_with_budget` (not parse_with_budget). Reuse them directly; never nest public
standalone decoders. Header/classfile/annotations/d1 decoding predates this input
boundary and is excluded. No whole-pipeline RSS/churn/allocator-surplus guarantee.

Before looping any caller-constructed d2 slices, d2.len() > min(entries,MAX_RECORDS)
=> Name Limit(entries),indexNone/recordNone,offset0, even with bytes/work/records0.
Combined checked input bytes = decoded d1.len()+sum(d2 unit lengths*2), checked
against min(bytes,MAX_BYTES). Table string bytes already belong to d1, no double
input count. Byte overflow/limit => Wire Limit(bytes)0. Resolver still enforces
aggregate entries (records+d2+local+transform lists), range slots, lifetime queries
and output units. Length-only constructor preflight remains separately hard-bounded
as existing resolver requires. Resolver Limits map entries/range_slots/queries/
output_units directly; input_bytes=bytes; remaining work is transferred from ONE
shared wire.work. Existing new_with_budget/get_string_with_budget already transfer
work/owned allowances and preserve query/output lifetime counters. Reuse untouched.
UTF16 output bytes debit work then shared owned before allocation, with existing
Name error context. Output Member vector uses existing deterministic Budget::push.

Exact sole shared-helper request: type_alias.rs import string_table; remove its
duplicate private Budget and push implementation; public decode preserves the
existing first byte check and creates string_table::Budget using original records,
work,owned_bytes limits plus default bytes/nodes/depth (nodes/depth unused by the
reader); extract pub(super) decode_with_budget(bytes,&mut string_table::Budget)
with the unchanged scan and output. Its pushes already have the exact same checked
slot/owned/work/reserve ordering as string_table::Budget::push; map wire Error via
existing From. No alias node charge inside the helper (composition supplies it).
Standalone empty input with zero budgets STILL MissingName0; no new constructor
validation or node/work charge. Public Limits/TypeAlias/Error signatures, defaults,
clamps, spans, raw borrowing, every-inline retention and error precedence unchanged.
No other alias caller/helper/export/test is modified. Existing standalone alias tests
already cover these compatibility controls. See SCOPE.md for exhaustive reach.

## Acceptance gates and exclusions

Before any implementation: SAME3 architect/correctness/adversarial contract review,
fix/recheck findings, literal fixture/source/hash freeze, exact coordinator grant.
Later source review and completed original CI review reuse SAME3 agents. No local
native/product/parser/worker/server/test/bench/oracle execution under #96. Future
compile-only fmt/Clippy requires coordinator admission. Future original unchanged
Linux Native Rust CI must run pinned cargo fmt --all -- --check;
cargo clippy --locked --workspace --all-targets -- -D warnings;
cargo test --locked --workspace. Preserve first outcomes, no rerun-to-green.
No current CI or product success is claimed by this preparation packet.

Future push only own branch/open+attach PR/publication requires root admission;
merge only expected reviewed head/base with root-exclusive passing merge lease,
then owner+root exact original-main proof. No bypass/admin/force/settings changes.
No issue or PR is posted here; ISSUE-DRAFT.md is unposted.

Parents #68/#18 stay OPEN. No symbols, SymbolIDs, overload resolution, facadelinking,
package semantic assembly, Type reconstruction, library/server semantic provider,
JAR ingestion, performance comparison or full official Kotlin parity. T008/T008a
HOLD; broader C13/B3/B4 GAP; B11 UNVERIFIED; #7/#83/#96 OPEN; Darwin UNKNOWN.
Selected malformed protobuf UTF8 remains UnsupportedUtf8. Existing separate public
reader wrappers retain their separate standalone budgets and compatibility limits.

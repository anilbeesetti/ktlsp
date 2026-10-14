# T018/#18 VersionRequirement and VersionRequirementTable preparation contract v1

Preparation only, 2026-10-14. No product edits, native/compiler/JVM/oracle execution,
push, issue publication or CI. The exact clean owned worktree is
/Users/anil/.codex/worktrees/t018-version-requirements/ktlsp, branch
codex/t018-version-requirements-contract, HEAD7980480e015150b75b982001c871ad1a5b54f35a,
tree e6a0d62d387f72b0ef1118109179cceec87cba40, sole parent
fe4f4a73f0bf9190f7668bf74a9b963f70114449. Later source/API/export/compile-only,
publication/original Linux CI and integration each need coordinator grants.

## Scope and exact reach

ONE new module crates/ktlsp-binary/src/metadata/version_requirement.rs, ONE new
integration test crates/ktlsp-binary/tests/metadata_version_requirement.rs, fixture
directory tests/fixtures/metadata_version_requirement/ (frozen hex, expectations,
provenance/readme and copied license/notices), local crate documents
METADATA_VERSION_REQUIREMENT_SPEC.md and METADATA_VERSION_REQUIREMENT_REVIEW.md.
The sole shared-source edit is serialized metadata/mod.rs export
`pub mod version_requirement;` after explicit grant. No existing callers need
changes; rg at this exact base found no VersionRequirement decoder or consumers.
No wire/string_table/helper, Cargo/lock, server/parser/worker, rootdocs/status,
workflow, benchmark or oracle edits. Existing public APIs keep their behavior.
Raw metadata is data only; no symbols, names/message resolution, version-policy
precedence/diagnostics, availability checks, full initialization/parity or LSP
provider claims. T008/T008a HOLD; C13/B3/B4 GAP; B11 UNVERIFIED;7/83/96 OPEN;
Darwin UNKNOWN; no performance comparison. Native implementation remains Rust only.

## Minimal public API

Module version_requirement reexports wire::{Error, ErrorKind}; no new error wrapper.
`Level { Warning, Error, Hidden }`, `VersionKind { LanguageVersion, CompilerVersion,
ApiVersion }` derive Copy/Clone/Debug/PartialEq/Eq.
`VersionRequirement<'a> { pub raw: &'a [u8], pub span: Range<usize>,
pub version: Option<i32>, pub version_full: Option<i32>, pub level: Option<Level>,
pub error_code: Option<i32>, pub message: Option<i32>,
pub version_kind: Option<VersionKind> }`.
`VersionRequirementTable<'a> { pub raw: &'a [u8],
pub requirements: Vec<VersionRequirement<'a>> }`.
Structs derive Debug/PartialEq/Eq. No redundant entry wrapper/unknown-field vector.
Raw bytes preserve unknown/wrong-wire fields, recognized duplicates and ordering;
children retain complete body bytes and body spans; table raw retains outer order.
`decode(bytes: &[u8], limits: wire::Limits) -> Result<VersionRequirement<'_>, Error>`
is allocation-free and separately bounded. Its span is 0..bytes.len().
`Limits { bytes: usize, records: usize, work: usize, owned_bytes: usize }` derives
Copy/Clone/Debug and Default, defaults16MiB/65535/64Mi/16MiB; callers only tighten.
`decode_table(bytes: &[u8], limits: Limits) -> Result<VersionRequirementTable<'_>, Error>`.
Only table owns a vector. Borrowed input cannot be modified or destroyed. No caller
constructible spans are accepted. No schema-selection/header API promise: caller
selects supported [2,3,0] and one unframed body, not delimited d1 or a class/package.
Private `decode_span(bytes, span, &mut wire::Budget)` is shared; table calls it with
original table bytes and validated child span, never public decode or a reset budget.
Errors/spans use supplied-input-relative offsets (table-relative inside all children).

## Pinned rules

Kotlin v2.3.20 commit d57eb4a26ed01ab03cf195da686c8047032ff16f. Copied existing
pinned data only, no upstream rediscovery or execution. metadata.proto Git blob
18cfd13e9e512c491f2f62ec241f49c81d6e5e81 SHA256
a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2 lines520..558.
ProtoBuf.java blobfc424d5e7aac8661deb862c8e791d4fcf779c194 SHA256
da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716:
VersionRequirement parser27543..27629/defaults27923..27940,
VersionRequirementTable parser28563..28612.
Patched CodedInputStream.java SHA256
7fbb3aeebd3c3d5414d7171bc3ba2785de50b40863a4f381c4b43986e15784fe;
wire README documents protobuf2.6.1 pin and notices. Sources remain temporary,
not shipped/compiled. Preserve fixture Apache2 and protobuf BSD license/notices.

Requirement selects ONLY varint fields1 version,2 version_full,3 level,
4 error_code,5 message,6 version_kind. Int32 = low32 cast signed, including negative
and huge/nonminimal raw values; no index/range/policy validation. Both versions
remain independently present, no priority computation. Int32 absent schema default0
is documented but Option stays None. Known level0/1/2=>Warning/Error/Hidden, absent
schema default Error. Kind0/1/2=>LanguageVersion/CompilerVersion/ApiVersion, absent
default LanguageVersion. Recognition uses low32 signed; unknown values do not set
presence or overwrite previous recognized values. Every recognized scalar last-wins.
Packed wire2 for these singular scalars is wrong-wire opaque, including malformed
packed contents. Other wrong-wire/unknown fields retain raw, not typed projections.
Table selects only field1 wire2; every occurrence appends, including empty and
duplicate entries; known field1 wire0/1/5 is opaque. A field1 wire2 is a child
message, never packed scalar data: its complete framing is decoded, not opaque.
Unknown table bytes fields stay opaque, even malformed protobuf-looking bodies.
All six fields optional: empty Requirement and empty table succeed, even limits0.
No missing-field error or semantic/default accessor. Wire groups unsupported;
invalid tag0 invalid wire6/7 malformed/short values fail. Raw bytes retained differ
from generated Java unknown-field reserialization; byte-for-byte JVM equivalence
is not claimed. Inherited wire slow-path low64/nonminimal low32 tag/length behavior
and stricter groups/envelopes remain disclosed; no new overflow policy.

## Exact processing/error order and one aggregate budget

1. Whole supplied bytes.len()>min(bytes,MAX_BYTES) => Limit("bytes") offset0,
   before any record/work/storage check. Clamp usize::MAX inputs; do not disable caps.
2. No root/node/depth charge or recursion. Fixed table->Requirement depth is2.
   Per field: wire::field(original bytes, cursor, validated end, same budget), which
   charges record first (records before work), one work for that record then all
   actual tag/value/length/body reads. Then charge dispatch work1 at tag_start,
   including unknown/wrong-wire fields. Wire error beats dispatch/storage checks.
3. Table selected child: parent bytes (including whole body) already read/charged;
   decode_span rereads child fields using SAME wire budget and table-relative
   boundaries; child's dispatch/read errors precede entry storage charges.
4. Once child fully decoded, use existing string_table::Budget::push at parent TAG
   offset. Reuse existing Budget only for aggregate wire and vector accounting;
   nodes/depth/integers helpers unused. No new shared helper or Budget type.
   Push logical slots1,1,2,4,... at len0,1,2,4,...; checked capacity addition and
   checked slots*S precede owned subtraction; Limit("owned_bytes") before work
   reservation charge, which precedes try_reserve_exact/Limit("allocation").
   No partial result on any error. Borrowed bytes allocate nothing. Unknowns don't
   allocate. Standalone decode uses only wire::Budget and never owns storage.
5. Continue complete input; later malformed fields fail after known valid values.

Let B=input bytes, I=sum child body bytes scanned, F=outer+inner wire fields,
N=selected children, S=size_of::<VersionRequirement>(), C=0 if N0 else next power
of2 >=N. Successful W=B+I+2F+S*C for tables; standalone W=B+2F. No root/entry
extra work. Input B<=16,777,216, I<=B, F<=65,535, requested retained logical vector
capacity S*C<=16,777,216, W<=67,108,864. N<=outer field count<=F, hence C<=65,536.
Checked arithmetic and existing fallible reservation must remain intact. The
formula charges parent body bytes AND child scan, and does not reset across siblings.
These are requested allocation/work/record bounds, not RSS, stack, allocator
surplus, transient relocation or complete header/d1/table/name pipeline bounds.
Host allocation failure cannot deterministically be induced; static review covers it.
EOF/truncation ordering inherits wire::Reader::short: charge remaining+1 work at
attempted take offset first, then Truncated at boundary. Invalid 10th continued
varint => MalformedVarint at tenth byte; terminating10th byte accepts low64.

## Acceptance and delivery gates

EXPECTATIONS.md and literal fixtures are authored independently before native
runtime and hash-frozen. Required original unchanged Linux CI later runs pinned
cargo fmt --all -- --check; cargo clippy --locked --workspace --all-targets -- -D
warnings; cargo test --locked --workspace. Local native runtime held under#96;
no local compiler/format/clippy/test invocation in this preparation stage.
SAME architect + correctness/functionality + adversarial reviewers review this
packet, changed source, and completed original CI. Findings fixed and rechecked.
Only after root source/API/export grants implement exact files; only after root
publication grant create issue/push own branch/open+attach PR. Preserve first
failures, no rerun-to-green/settings changes. Merge only reviewed expected head
with root exclusive base lease; owner+root verify original exact-main proof.

## Source-stage admission and preserved freeze

Coordinator admitted the exact source/API/export/compile-only/publication/original
unchanged Linux CI stage on 2026-10-14. Preparation-only restrictions above are
historical freeze context; local native runtime remains held under#96 and merge
requires a later exclusive head/base lease. No Java/proto source caches ship.
The frozen EXPECTATIONS.md/PROVENANCE.md mention preparation HASHES.json; that
manifest is the temporary preimplementation receipt, not an additional shipped
fixture. Its SHA256 is a035ad4f2a787b83f12a67540f52963e2c5fdf89809f3f07f12ddcddc838a049.
The exact frozen expectations SHA256 is
a2971b3cfffc3de5a3b347567bb36641b5824178113b2cbd6779f3d013e0e29d.
All shipped data hashes from that manifest are reproduced below.

| Fixture file | SHA256 text | Decoded SHA256 if hex |
|---|---|---|
| EXPECTATIONS.md | a2971b3cfffc3de5a3b347567bb36641b5824178113b2cbd6779f3d013e0e29d | — |
| LICENSE.txt | cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30 | — |
| NOTICE.txt | 0b09a83d3ef7795c7dec65e815866597c066c53898f2852a76599b8f5552941a | — |
| PROTOBUF_LICENSE.txt | 2f706d67271d8db967046638e30f7887a5ba6e49593e6395e8145a7c9feab96a | — |
| PROVENANCE.md | 89cd9f50d56d40171cc7a01da5756c6ff8eaee287a3e30adf8712eab4d47f2a7 | — |
| duplicates.hex | 8739141fbb22b189d37787cd90abbc20713c468953e980d4932423443a5ab31f | de18248e040821851fbdae52b5751174b2f6d0d2f37902d72628be0142dcaee7 |
| empty.hex | 01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b | e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 |
| explicit.hex | 37317474dc9d7b93dfaf4433f3041402a552f11ed5d093f2fed3b4e7481af8b2 | 7fafe290f4abba71da9fec36824a832b7e5a5a8d3a558164ecbb0794c4f1b89f |
| nonminimal.hex | f9fc79ac314aaaa1900460ab423240c2d1d6deaab5a4077cf52ce9fea98ecd85 | 02994bdbaa12274585c644da8470fb4c2d4afe177f51180d688dba1c5afed439 |
| signed.hex | b467d115fae0ee1bb40624e82dfc72604bf20d5b7f495fc642b2f7a6a6bc05bd | 7e0432e2d72eb2f2f70f3d465c7d684c236d6e9b1eeffbed7549ffa5e72864dd |
| table_aggregate.hex | 83fc8a9a8921222db98ad43700ae5a685caa17cb39e1266cb024010dc0df919e | 861afd778bb755ded46d6bd4998a7d01205dd3eead5a29ad8134dd984f9b542c |
| table_malformed_child.hex | 8ae9cdb398a27f303c57d9b9adea62515464071b6e368d1d537e6f6ae85e0825 | e104feb0303f5f40bc2d060cb360734cb44c656005cd4cacf2e82747b4f63a10 |
| table_mixed.hex | 580f934493078ad330fc92d41b99f3a2eef09ead6e0f8124535cb19ac5bee48a | 4d3a8772930ba71332a909e1f3b7a1f5c2ed8ec3e738cac2dc43337e243f73e2 |
| table_ordered.hex | a29b98eb8c4345d7f94da842cb3f03cb479ed1f987f8598eb6afb2cecbf8eb79 | 3326e170e6bedfaf628cc55682946e9dc870401649041c674cc2bf4b2c582e4e |
| table_truncated_parent.hex | 6975b0063854e04837495d9a0e8c52ea3465e506d61483d13583c90edfa383b7 | 9046d8336bb49af4ef8bae538a6fd91ef71500f0e5021e7ec05d0c0e3c9bcd24 |
| unknown_enums.hex | d1a943cb6942f7fddc7c646f81896f85586c6c54c3c1104e04ac814bc7821543 | 9706ebd391a921c92e95c54b0f1c5fc05a33c2d54ebacd4b1a811f70e5d8fcf2 |
| wrong_wire.hex | ea4864e4692680ea3bb00de8cf1feca388d6049ad6ea4e5e81e23a96e2759262 | 097c4e49f21cfc9bdde694b42b00f2745da6ddb33341fb5d4de6eb513ec5de35 |

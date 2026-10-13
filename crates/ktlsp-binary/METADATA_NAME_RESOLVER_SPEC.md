# T018a2 held name-record contract, revision 2

Preparation only, 2026-10-13. Child to be published by coordinator under #68/#18.
Frozen delivered base b910df05ba8942350700e2985d497511459952b7. Existing Git
objects were read with git show; managed HEAD 61142f66 is not the source base.
Implementation BLOCKED on delivered #125, its API/ownership release and root's
explicit source-stage grant. Provisional #125 head ebc4b301e58e9ce33961fa60f3cd211dee9da3ae
at /private/tmp/ktlsp-string-table-20261013 was inspected read-only, never imported.

## Finite result and reach

Resolve JVM StringTableTypes name records to lossless UTF-16 units and local-name
membership only. No declarations, type reconstruction, name grammar validation,
symbol identity, class linking, server wiring, provider advertisements or parity.
Parents #68/#18 remain open. Metadata version/kind selection remains the caller's
responsibility: supported delivered header outcome is mv=[2,3,0], kinds 1/2/5.

Eventual exclusive files: metadata/name_resolver.rs, tests/metadata_name_resolver.rs,
fixtures/metadata_name_resolver/** and ticket-local contract/review documents in
crates/ktlsp-binary. One serialized additive metadata/mod.rs export requires root
grant. No wire/string_table/header/lib/Cargo/lock/dependency/vendor/server/root-doc
changes. Existing metadata callers at b910 are header/wire integration tests only;
no production consumer. New tests call delivered #125 decode and resolver; existing
wire/header regressions must remain unchanged. No caller API changes or data writes.

## Proposed smallest public API (pending delivered #125 reconciliation)

metadata::name_resolver::{Resolver, Limits, Error, ErrorKind}.
Resolver::new<'t,'a>(table: &'t StringTableTypes<'a>, d2: &'t [&'a [u16]], limits: Limits)
  -> Result<Resolver<'t,'a>, Error>.
Resolver::get_string(&mut self, index: i32) -> Result<Vec<u16>, Error>.
Resolver::is_local_class_name(&mut self, index: i32) -> Result<bool, Error>.
Qualified class name is exactly get_string; no duplicate wrapper method.
Resolver fields private; no Clone/reset/replenish. Borrow table/d2, no expanded
record vector, no index/cache allocation. Linear bounded scans are sufficient.
Typed fields are authoritative even for caller-constructed tables. Raw bytes,
unknown vectors and span fields are never interpreted, compared, cloned or trusted.
Constructor validates ALL consumed field lengths/range sums under its own limits,
independent of decoder success. It does not pretend to verify raw/typed equivalence.

Error { kind: ErrorKind, index: Option<i32>, record: Option<usize> }.
Kinds: InvalidIndex, MissingString, UnsupportedUtf8, Limit(&'static str).
Record means original compact record ordinal, never expanded ordinal/wire offset.
No errors quote names/content. Rust errors replace JVM indexing exceptions; explicit
budget and unsupported-text errors are documented subset boundaries, not JVM results.

## Pinned source rules

Kotlin v2.3.20 d57eb4a26ed01ab03cf195da686c8047032ff16f:
core/metadata.jvm/src/org/jetbrains/kotlin/metadata/jvm/deserialization/
JvmNameResolver.kt Git blob ad1dccca91fb9b07f98ab2546b51d56713216e2d,
SHA256 e605e733e62ca692725d0b23d74186e40959825f9d1f0a1bfedf3b0ad649c78c;
JvmNameResolverBase.kt blob 6dbce51558533d26fbe184602056d371d95281cc,
SHA256 58fcd7277be88502de9f5be47824dae32d654eb9ae7d2b028db675a51a8a9326;
libraries/stdlib/src/kotlin/util/Standard.kt blob 440c6cd32099a5fabf435eab642f88dd4f754430,
SHA256 86b7320d868d66492d277ebb5d5d8ad78da074d592dacf32f186a83d8d3196a0.
JvmProtoBuf.java same #125 pin/hash b1ad864ae872315c06740d6551d2d7233060f5bff3e9ad9b8f80f531018bc3a1;
jvm_metadata.proto hash 5a80e63021b800afd2297b3713c4cfc12739cbc8e62c726578e1c5d1dfc771f9.
All source reads are data only. Kotlin Apache-2.0 notices must accompany derived
predefined strings/rules; existing #125 fixture license/notice are reused unchanged.

Record range absent=1. Effective count=max(range,0): Kotlin repeat uses 0 until
times, so negative and zero contribute nothing. Sum positive counts with checked
arithmetic and cap BEFORE mapping/allocation. Index i uses the first record whose
cumulative exclusive end exceeds i. No requirement sum=d2.len(); fallback indexes
the original requested i, not the compact record number or offset in the range.
Negative i or i>=sum => InvalidIndex. Zero records is valid construction.

Base precedence: present string (including empty bytes) wins; otherwise a present
predefined_index in 0..44 wins; otherwise d2[i]. Absent predefined index does NOT
select predefined0. Negative/out-of-range predefined values fall back without error.
Missing d2 only fails MissingString if fallback is actually selected. All 44
predefined values and their ordering are copied exactly from the pinned source;
boundary 0=kotlin/Any, 14=kotlin/String, 43=kotlin/collections/MutableListIterator.

Transform in order: substring, replacement, operation. If substring list has >=2
entries, use first two; apply only 0<=begin<=end<=UTF16_length, else ignore. Extra
entries and lists of 0/1 entries have no semantic effect. If replacement list has
>=2, truncate first two signed int32 values to low16 u16, replace ALL matching
UTF-16 units once. No iterative replacement; extra entries ignored.
Absent operation=None. InternalToClassId replaces every '$' unit with '.'.
DescToClassId first strips first/last UTF16 units if length>=2, then replaces '$'
with '.'. It does not require L/semicolon; strings of length0/1 are unstripped.
Do not replace '/' or normalize names. #125 retains recognized operations when
unknown enums occur; resolver uses typed operation only and ignores unknown fields.

local_name is a set membership in JVM, but linear contains gives the same result:
duplicates, negatives and values outside resolved slots are retained and accepted.
is_local_class_name may query any i32, including -1; no name/index validation there.

## Text boundary and explicit gap

d2 already contains raw Java UTF-16; unpaired surrogates, embedded NUL, surrogate
splits by substring, replacement-created surrogates, and descriptor stripping through
surrogate pairs are preserved as exact units. Never convert d2/output to Rust String,
slice UTF8 by a UTF16 index, call lossy decoding or reject noncanonical names.
Present record.string bytes are protobuf standard UTF8, not classfile modified UTF8.
Valid UTF8 decodes to UTF16 with Rust str::encode_utf16, including supplementary pairs.

JvmProtoBuf.Record.getString calls ByteString.toStringUtf8, which delegates to
LiteralByteString.toString -> Java new String(..., charsetName). Pinned protobuf
2.6.1 bba83652e1be610bdb7ee1566ad18346d98b843c sources in sources/:
ByteString.java blob 7da56127d70c94610f0f8f7358ccaf39c82a4b94,
SHA256 157e98eb64c33fea2528bedb0b945d5ff7717ffe99160cb73941e5c9446d657a;
LiteralByteString.java blob 83e71e93335450bb9449e9fde6ad90f858911000,
SHA256 f675be0f6acd7ad73ce5479beaef397abcbf168d83aa10d9426afb9a70adf4b5.
Java may replace malformed byte sequences; exact replacement segmentation depends
on its decoder. This child deliberately returns UnsupportedUtf8 on SELECTED invalid
UTF8 bytes, including CESU8/overlong NUL, before transformation. It does not claim
Java malformed-UTF8 result equivalence. Unselected invalid strings (zero ranges or
other lookup records) do not fail construction or unrelated lookup. Full malformed
protobuf-text compatibility remains an explicit follow-up, without lossy guessing.

Package provenance: the same Kotlin pin's dependencies/protobuf/build.gradle.kts
blob 9c6fb7d360deeb820f8ea6d95ad96e74ef941dfb selects protobufVersion2.6.1.
dependencies/protobuf/protobuf-relocated/build.gradle.kts blob
9bc9b5ebd7f10bf49e37354a0372d69c6d17e014 includes base protobuf-java and patches,
relocates com.google.protobuf to org.jetbrains.kotlin.protobuf, and source relocation
excludes upstream CodedInputStream only before adding patch sources. Both scripts
are retained as data in sources/; neither Gradle nor a Java binary was executed.
The ByteString delegation is a source/build-derived inference about the relocated
runtime, not a measured malformed-decoder result. UnsupportedUtf8 is a chosen finite
GAP, not a claim Kotlin rejects those bytes.

## Resource contract

Limits { entries, input_bytes, range_slots, queries, work, output_units }, caller
tightenable only. Hard defaults respectively 65,535;16MiB;65,535;65,535;64Mi;8Mi.
entries=sum(records.len+d2.len+local_name.len+all substring/replace list lengths).
input_bytes=sum(all present explicit byte lengths + 2*all d2 unit lengths), checked.
Counts include ignored list tails, duplicate locals and zero-range strings. Raw/unknown
data are not consumed and excluded. First perform an O(1) checked sum of top-level
records.len+d2.len+local_name.len against entries before visiting ANY elements.
Then inspect at most that bounded number of metadata entries: add each record's two
list lengths with checked additions and stop immediately on entries overflow/excess;
no traversal of list contents. Once entries succeeds, sum each explicit byte length
and each d2 length*2 against input_bytes with checked additions and immediate excess
return. This is bounded length-only preflight, not a content scan or allocation.
This preflight is separately bounded by the entry hard ceiling, independent of
caller work; it is excluded from metered traversal work. A work0 caller may incur
this finite preflight before failing retrospective entries/input work debits.
Range_slots is sum effective positive ranges; no expansion even when over cap.
After preflight, constructor charges entries against work, input_bytes against work, then validates
range sums in record order (range loop is covered by the record entry charge).
Constructor allocates nothing. Invalid UTF8 isn't scanned until selected.

One lifetime budget covers constructor and ALL method calls, failures included;
no refund, reset or per-record reset. A failed debit leaves its bucket unchanged;
all earlier successful debits remain consumed. Each method charges queries1 then work1.
get_string rejects negative indices, scans records charging work1 each before visit,
then InvalidIndex if absent. Once selected, determine base source; fallback bounds
check before text work. For explicit UTF8 charge bytes.len work BEFORE str validation,
then bytes.len work BEFORE UTF16 counting pass, then final-source unit length work
BEFORE streaming the output pass. d2/predefined lengths are known: charge source
unit length work before output pass. Substring/strip determine output span by units;
unit replacement and '$' mapping can compose in the single streaming emission pass.
Charge output span length against output_units then 2*length against work BEFORE
try_reserve_exact(length), even zero-length outputs. Allocation failure is Limit(allocation),
no partial output; successful output allocation is at most charged UTF16 units.
All counts checked; overflow reports the corresponding limit. Allocation rounding,
caller-retained outputs/inputs and allocator overhead are not RSS bounds.
is_local_class_name scans local_name in order charging work1 BEFORE each comparison,
stops on match (including negatives). It never scans records or allocates.
Limit order: entries, input_bytes, work(entries), work(input_bytes), range_slots
in construction; query/work then record scan/index/base/text/output/allocation order
in lookup. Context on constructor global limits=None; range limit names record;
method errors include queried index and selected record when known.
No recursion, subprocess, filesystem/network/external-tool calls or new dependency.
Decoder/header/framing limits remain SEPARATE; no combined end-to-end budget claim.

## Acceptance, not yet executed

Freeze fixtures/EXPECTATIONS.md first; manually authored bytes/UTF16 arrays, never
derive expected output from the implementation. Tests cover every rule above:
range defaults/zero/negative/shared-record indices/boundaries; empty-present precedence;
all44 predefined values; fallback/absent d2; substring invalid/short/extra/boundaries;
replace truncation and ordering; all operations with empty/1/2 units; local duplicates,
negative/out-of-range membership; NUL/astral/lone surrogates; selected invalidUTF8
Unsupported versus unselected invalidUTF8; retained known/unknown operation via #125.
Direct caller-constructed records with hostile lengths/ranges/raw spans exercise all
limits at exact/one-below thresholds and aggregate repeated calls, including failures.
Native deterministic generated malformed probes after source admission; no runtime now.

No local Cargo/build/compile/parser/server/benchmark/JVM/oracle. After explicit source
admission only optional local fmt/compile-only Clippy if root grants. Existing unchanged
Linux Native Rust CI runs fmt/clippy/full-workspace tests on exact reviewed combined
tree and exact merged main. Same two reviewers and architect must renew source/graph/
CI readbacks. #96 OPEN/UNKNOWN, T008 HOLD, B3/B4 GAP, B11 UNVERIFIED. No Darwin fix.

## Environment/readback evidence

First managed fetch exit255: cannot open shared FETCH_HEAD: Operation not permitted.
First tmp HTTPS clone exit128: Could not resolve host: github.com (transport corrected
only after root clarification). First explicitly authorized SAME SSH clone exit128:
ssh: Could not resolve hostname github.com: -65563; Could not read from remote repository.
No auto-review rejection; no retries, shared-Git writes, product edits or executions.
Root accepted existing b910 objects for preparation; future owned checkout still required.
Connected issue reads #68/#18/#125 OPEN. Commit-status read returned statuses=[];
PR-only workflow-run lookup returned workflow_runs=[]; neither proves missing/passing CI.
Frozen docs carry historical Linux receipts; this contract has no new CI/run evidence.

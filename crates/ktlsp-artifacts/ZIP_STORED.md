# Bounded stored ZIP payload (T020 child, held source)

Source admitted by root on 2026-10-13 against frozen prerequisite commit
`a1887d43905cdb37479fe4b54f1d7b24cbfc4df7`, tree
`cf71551e2a31347d83d609c3c06b5d573694fccb`.
The three independent finished-contract reviews approved exact contract SHA-256
`42a0dd539d8ccf158b94d3f3d8530f8e4716880cf087a7a05d5127aaa72db4d1`.
This file applies that contract to held implementation source; implementation
review, compilation and runtime evidence are distinct (see ZIP_STORED_REVIEW.md).
Delivered PR116/#114 remains the directory-only prerequisite with unchanged
acceptance. Parent #20 remains OPEN. No server or semantic consumer is added.
Local pinned formatting and compile-only Clippy are permitted; local native
runtime/tests remain prohibited while #96 is OPEN/UNKNOWN. Push, PR, CI,
integration and merge remain held for a later root base/grant.

## API and exact reach

```rust
impl<'a> ZipDirectoryIndex<'a> {
    pub fn stored_payload(&self, ordinal: usize) -> Result<&'a [u8], ZipStoredError>;
}
```

No second artifact, arbitrary entry handle, byte-slice entry point, name lookup,
winning duplicate, public constructor or mutable getter. Successful output is a
slice of the SAME retained VerifiedArtifact allocation indexed at construction.
Ordinal is central-record order. Duplicate names at distinct disjoint local
records may both succeed; aliasing the same local record rejects. Returning an
empty slice is valid. No provenance path access/re-read, payload/name copy,
cache, per-call allocation, recursion, decompression, filesystem extraction,
class parsing, server capability or semantic consumer.

Root admitted these exact private representation changes:

* Index adds private `bytes: &'a [u8]` and `directory_start: usize`.
* Index currently derives Debug. Replace ONLY its derive with a manual Debug
  that preserves the existing entries/archive_comment fields and omits retained
  bytes/directory_start; otherwise formatting would disclose/amplify the entire
  archive. Add a regression asserting payload sentinel bytes are absent from
  index Debug output, while existing metadata fields remain. Entry Debug stays
  metadata-only.
* Entry adds private `version_needed: u16` and `central_offset: usize` populated
  during the existing sole central parse. These have no new public getters.
  The offset permits exact error attribution without pointer arithmetic or
  rescanning the directory. Existing private constructors/getters stay private.
* New `src/zip_stored.rs` is a private CHILD module of zip_index (explicit path),
  allowing reuse of its checked accounting/read/error helpers without widening
  helper visibility. Re-export ZipStoredError through zip_index and src/lib.rs.
  Add no dependency/config/Cargo/lock/workflow changes.
* Recheck `count * size_of::<ZipDirectoryEntry>() <= 2 MiB` before reservation
  under the new layout and existing MAX_ENTRIES=16,384. Compile-time size-bound
  assertion plus existing private accounting test; no fabricated runtime count.

Admitted ownership: NEW artifacts `src/zip_stored.rs`,
`tests/zip_stored.rs`, `tests/fixtures/zip_stored/*`, `ZIP_STORED.md`,
`ZIP_STORED_REVIEW.md`; narrowly modify existing `src/zip_index.rs` for the four
private fields/module re-export/layout assertion/manual Index Debug and `src/lib.rs` for one
re-export. Root separately admitted ONLY two stale scope paragraphs in existing
`ZIP_INDEX.md`, corrected in documentation successor717eee without source/fixture
changes. No other existing documentation ownership was extended.
Existing index tests are unchanged except any explicitly needed
compile-fail example maintenance; retain directory-only accepted invalid-local
fixtures. No root/status docs, binary/parser/vendor, integrity/lookup code,
Cargo/lock/workflow files owned. Any additional shared/API/helper/Cargo/lock/workflow/consumer changes require
root coordination before editing. Frozen rg finds no
production callers: only artifacts index implementation, exports, its tests and
compile-fail documentation. Existing users retain directory-only behavior.

## Error surface and deterministic phase order

`ZipStoredError` has `InvalidOrdinal { ordinal, entries }`,
`Structure(ZipIndexError)`, `Disagreement { offset, field: &'static str }`,
`Overlap { offset, other_ordinal }`, and
`CrcMismatch { offset, expected: u32, actual: u32 }`.
Reuse existing structural/unsupported/limit categories and helpers by wrapping
ZipIndexError. Display is static labels/numerics only, never names/paths/comments.
No partial result on error. All offsets absolute from retained byte zero.
For disagreement use LOCAL field offset; for overlap use selected header start;
CRC mismatch uses selected payload start. InvalidOrdinal has no byte offset.
Use checked usize conversion/add/multiply, bounded get() before any read.

Perform these phases in order; within each list use its written order:

1. Ordinal exists (even on empty directory). Selected CENTRAL method must be 0
   (UnsupportedMethod at c+10); flags must be 0 or 0x0800
   (UnsupportedFlags at c+8, including descriptor bit3). Directory construction
   already rejected unsupported version/encryption/ZIP64/multidisk for ALL
   entries; do not change that boundary or retry other ordinals automatically.
2. Precharge `31 * entries.len()` work units at selected h BEFORE reading local
   headers. Checked complete selected `[h,h+30)` must end at/before directory d:
   Malformed(h, "truncated local header"). Then require local signature PK0304:
   Malformed(h, "local header signature"). Local compressed/uncompressed size
   sentinel -> UnsupportedZip64(h). Then method0 (h+8), version10..=20 (h+4),
   encryption bits0/6/13 (h+6), flags only0/0x0800 (h+6). Use existing specific
   Unsupported categories. The version interval is intentionally the existing
   directory policy, not an expanded ZIP compatibility promise.
3. Compare local/central in order: version(h+4), flags(h+6), method(h+8),
   CRC32(h+14), compressed size(h+18), uncompressed size(h+22). Any differs ->
   Disagreement(offset, field) with names "version", "flags", "method", "crc32",
   "compressed size", "uncompressed size". Local and central stored sizes must
   then be equal -> Malformed(h+22, "stored sizes differ"). DOS times/dates,
   creator version, attributes and comments remain uninterpreted, not compared.
4. Local name length n at h+26 must be nonzero -> Malformed(h+26, "empty name").
   n<=4096 -> Limit(h+26,"local name bytes",4096); local extra length x<=16384
   -> Limit(h+28,"local extra bytes",16384). Checked name end h+30+n and extra
   end p=h+30+n+x must be <=d; fail at h+30 with "truncated local name" or at
   name end with "truncated local extra". Obtain those bounded slices only.
   Require n==central raw_name.len() -> Disagreement(h+26,"name length").
   Precharge n+x work at h+30 before comparison or extra scanning; raw name
   equality -> Disagreement(h+30,"name"). Since flags/name match, central UTF8
   validation already covers the local name; no encoding guess/duplicate scan.
5. Parse LOCAL extra TLVs using existing framing and policy: incomplete header
   -> Malformed(t,"truncated extra header"); body beyond the local extra slice
   -> Malformed(t,"truncated extra body"); THEN tag0001 -> UnsupportedZip64(t),
   tags0017/0019/9901 -> UnsupportedEncryption(t). Tag errors follow framing.
   Unknown/repeated/zero-body/JAR marker/Unicode TLVs are opaque and allowed.
   Local and central extras need NOT be identical; neither overrides raw name.
   No local TLV copies/recursive parsing and no scan of unselected extra bodies.
6. After p<=d is established, require central compressed size<=d-p BEFORE
   adding it to p; otherwise Malformed(p,"payload crosses directory"). Then
   checked q=p+size is necessarily <=d. Bound against d, not merely EOF. Complete local/name/extra/payload
   bounds AND stored-size equality/agreement precede CRC. Then selected payload
   size <=8 MiB -> Limit(p,"stored payload bytes",8388608). Empty p==q is valid,
   including p==q==d. Payload size is never an allocation input.
7. Check selected `[h,q)` against each OTHER declared header/compressed-payload
   envelope in central-record order. For each j, checked header `[hj,hj+30)`
   must fit before d -> Malformed(hj,"truncated competing local header"); its
   signature must match -> Malformed(hj,"competing local header signature").
   Read only nj/xj lengths, not bodies; compute checked
   `bj=hj+30+nj+xj`. Require bj<=d AND central[j].compressed_size<=d-bj BEFORE
   adding the size; otherwise Malformed(hj,"competing local span crosses directory").
   Then checked `ej=bj+central[j].compressed_size` is necessarily <=d.
   If h<ej && hj<q -> Overlap(h,j). Adjacent end/start equality is allowed.
   This detects aliases and BOTH overlap directions, including earlier payload
   enclosing selected header, nested headers, variable-field overlap and empty
   payloads (header envelope still nonempty). Central order, not local order,
   chooses the first error. An unselected malformed envelope may conservatively
   prevent selected access; never silently ignore it.
8. Precharge payload length work at p, then compute CRC once across exactly
   `[p,q)`. Compare to the agreed central/local CRC. Mismatch ->
   CrcMismatch(p,expected,actual). Empty CRC is 0: agreed nonzero claims reject
   even though no byte is scanned. Only after equality return borrowed bytes.

Checked conversion/overflow errors use Malformed at the originating field with
static "local offset conversion" (c+42), "local span overflow" (h),
"payload span overflow" (p), "competing local offset conversion" (cj+42), or
"competing local span overflow" (hj). These are defensive AFTER the preceding subtraction-based bounds checks.
Directory-only indexing permits0xfffffffe size claims; adding those blindly
would overflow on32-bit. Public huge-size fixtures MUST get the same boundary
errors on32/64-bit before addition. Header/variable additions are bounded by
retained64MiB and u16 lengths; payload additions happen only after the remaining
span check. Private helper tests exercise otherwise unreachable overflow.

## What overlap success means

Success validates selected supported local/header/name/extra/size/CRC data and
nonintersection with all other DECLARED header + central-compressed-byte
ranges. Other entries may be method8 or descriptor-flagged, whose CRC/features,
local/central agreement and descriptor tails are NOT validated by this method.
Only their fixed header signature/length envelope is inspected. A descriptor
TAIL may overlap selected bytes without this child detecting it; this is an
explicit gap, not a complete local-record/whole-archive overlap guarantee.
Overlaps exclusively between two unselected envelopes do not prevent access
unless one envelope itself is malformed/out of bounds. Unindexed hidden local
records, gaps/prefixes, archive-wide overlap validity, manifest/multi-release/
signature semantics, filesystem path safety and whole-JAR validity remain
unproved. Preserve unsafe raw names/duplicates; consumers need separate policy.

## CRC, byte/work limits and trust

Freeze ZIP CRC-32/ISO-HDLC (IEEE reflected): width32, polynomial0x04c11db7
(reflected0xedb88320), init0xffffffff, reflected input/output, xorout0xffffffff.
Check ASCII123456789 ->0xcbf43926 and empty ->0. On-disk u32 little-endian;
compute over uncompressed stored payload, no local header/name/extra bytes.
Author a small Rust scalar implementation (one byte/eight bounded bit steps)
without a dependency or copied implementation code. At most8MiB*8 bit steps per
call; no table allocation. CRC and caller-SHA equality are integrity evidence,
not authenticity, coordinate/publisher trust or JAR signature verification.

All fixed limits are per invocation, with checked precharges and unchanged
counter on failure: payload<=8MiB; local name<=4096, local extra<=16384;
work<=16MiB with error field "stored work units" and limit16777216.
Checked header-reservation multiplication refusal uses the same Limit at h;
variable/payload charge errors use their stated charge offsets. Header reservation31*N pays30 inspected bytes plus one range
comparison per central entry (selected reservation includes its fixed header).
Selected variable reservation n+x pays name comparison and TLV work; a TLV is
at least4bytes, so this is conservative. CRC pays one unit/byte. Conservative
maximum `31*16384+4096+16384+8388608 = 8916992`, below16MiB, so public work-limit
failure is unreachable under constituent limits; directly test private exact/
over/overflow accounting. No second vector, string, buffer or payload copy.
Retained artifact64MiB, entries16384 and existing directory budgets remain.
No session/archive aggregate across REPEATED calls is enforced or claimed:
callers must bound total requested reads; no consumer/session is added here.
An archive-wide extract/decompression API needs a separate shared cumulative
budget contract. No wall time, RSS, allocator/stack, Darwin reliability or speed
advantage is claimed.

## Fixture provenance and later validation

`tests/fixtures/zip_stored/fixtures.json` pins every independently authored byte file, SHA256, exact
central/EOCD/local/payload ranges and expected ordinal outcome. The frozen preparation packet retains the independent Python authoring tool;
it imports no ktlsp code. Literal Rust expectations in fixture `cases.rs` are
transcribed from that manifest, with independently computed CRC constants; no
new manifest parser dependency is added. Expected errors are authored from this contract,
not observed from a parser. Manifest ordinal/expected ordinal "usize_max"
means target usize::MAX (32/64-bit), never a truncated u64 constant. Binary files use .bin to avoid archive tooling.
Layout mutations pair each structural/feature fault with bad CRC to freeze
pre-CRC precedence. Prefix truncations and large boundary cases are recipes in the preserved preparation VALIDATION.md; exact committed minimal cases are frozen now. Held Rust tests
assert expected fields/ranges/errors, borrowed pointer identity, unchanged
artifact digest/provenance, and compile-fail no second-artifact/arbitrary-entry/
constructor/mutation/lifetime bypass. Never call a compile-fail that fails for an
unrelated missing symbol evidence of API safety.

Existing licensed pinned Commons Lang3.17.0 JAR SHA256
`6ee731df5c8e5a2976a1ca023b6bb320ea8d3539fbe64c8a1d5cb765127c33b4`
contains396 class entries, all method8, zero method0. Existing pinned Guava
33.4.8-android JAR SHA256
`a1f8a4bafe5a77232d333356843e194599b30837574a7f4be1063daaaaab102c`
contains1932 class entries, all method8, zero method0. Directory-only Python
zipfile.infolist inspection, no read/extract/decompress/repack. Therefore no
genuine stored-classfile coverage is claimed or manufactured. Existing .class
files are not repackaged into synthetic archives to imply genuine coverage.

Reuse pinned primary [PKWARE APPNOTE6.3.10 FINAL](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT),
revised2022-11-01, specifically4.3.7/4.3.8/4.4.7/4.5 for the new local/CRC facts.
The existing index contract remains authority for directory policy. CRC
polynomial cross-check only: [RFC1952 section8](https://www.rfc-editor.org/rfc/rfc1952.txt).
References/independent arithmetic only, no copied prose/source. Narrow policy
rejections are not claims that rejected ZIP files violate general ZIP format.

After implementation: the same two reserved independent functionality and
adversarial/crash reviewers must inspect held source, fix/recheck findings, then
obtain finished architect review. A finite status-corrected child issue may then
be published under existing authorization. Push/PR/CI/integration/merge remain
held for a later root base/grant. Original existing unchanged Linux NativeRustCI alone runs pinned
fmt/clippy/full-workspace tests, with actual independent final source+completed
CI readbacks. No local native execution while#96UNKNOWN. Owner push/attach PR,
root exclusive expected-head grant, guarded ordinary squash, owner/root exact-main
all-three delivery proof; no protection/admin/force/retry-to-green/workaround.
#20 remains open; T008/T008a HOLD, B3/B4 GAP, B11 UNVERIFIED, #7/#83 OPEN and
historical parser audit unchanged. No semantic/full parity or performance claim.

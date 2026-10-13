# Bounded ZIP directory metadata (#114 / T020c)

`index_zip_directory(&VerifiedArtifact)` returns an immutable borrowed
`ZipDirectoryIndex`. Its entries retain central-record order, raw name/extra/file
comment slices, method/flags and explicitly **claimed** CRC32, compressed and
uncompressed sizes and local-header offsets. The archive comment is a raw slice.
All bytes come from the retained verified allocation: no filesystem read,
provenance update, archive/name copy or unverified byte-slice entry point.
Caller-provided SHA256 equality does not authenticate the digest, coordinate or
publisher. This index does not provide payload access, extraction or name lookup.

## Acceptance policy

- Search at most 65,536 possible EOCD starts in the last 65,557 bytes. A candidate
  requires the signature, complete 22-byte header and comment length ending
  exactly at EOF. Reject zero candidates or more than one before parsing any
  directory. A second EOF-consistent candidate inside a comment is deliberately
  ambiguous even if one candidate's directory would be valid. No fallback/retry.
- Require single-disk fields zero, equal entry counts, absolute offset plus size
  exactly reaching EOCD, exactly that many central file records and exact table
  consumption. Reject impossible `count * 47 > size` and zero-count/size
  disagreement before fallible vector reservation. Each fixed/variable record
  and TLV must fit its directory/extra slice before it is read. No trailing bytes
  outside the declared archive comment or unexplained gaps/optional records.
- Empty EOCD-only archives and arbitrary raw archive comments are supported.
  Pretable bytes are ignored, with no prefix/concatenation offset correction.
  Claimed local offsets must precede the directory; aliases and reversed order
  remain intact. No local-header minimum length/signature/agreement check occurs.
- Methods 0 (stored) and 8 (deflate) are opaque metadata. `version needed` is
  limited to 10..=20. Bits 3/11 are accepted for both methods; bits 1/2 additionally
  for deflate. Encryption bits 0/6/13, other flags, methods or versions reject the
  entire index. DOS times, creator version and file attributes are uninterpreted.
- ZIP64 sentinels in EOCD/central size/offset/disk/count fields and extra tag
  0x0001 are unsupported. A locator signature exactly 20 bytes before EOCD is
  conservatively rejected without parsing its body, including identical bytes
  in otherwise opaque central metadata. Other opaque bytes are not searched for
  ZIP64 records. Central digital-signature, archive-extra and ZIP64 records are
  unsupported; encrypted/compressed central directories are not decoded.
- Names must be nonempty. Preserve duplicates and all raw path spellings,
  including NUL, traversal, absolute/drive/backslash names, trailing slash,
  file/directory, case and Unicode-normalization collisions. No deduplication,
  normalization, encoding guess, directory inference or safe-path guarantee.
  Bit 11 requires UTF8 name and file comment; unset leaves both raw. Archive
  comments are always raw. Unicode extras cannot override name/comment fields.
- Extras are flat length-delimited TLVs: framing/consumption checked, bodies
  uninterpreted. Reject ZIP64 and known encryption tags 0x0017/0x0019/0x9901;
  preserve unknown/repeated/zero-body/JAR-marker/Unicode extras as raw bytes.

Errors are explicit `ZipIndexError` categories with offsets and static reasons or
limit names. Unsupported entries never yield a partial/filtered success. Error
formatting contains no input names/comments/paths. ZIP64 and disk checks precede
other entry features, method precedes version/flags, framing precedes encoding.

## Fixed resource bounds

| Quantity | Maximum |
| --- | ---: |
| Retained artifact bytes (existing verified-artifact bound) | 64 MiB |
| Directory bytes | 8 MiB |
| Entries | 16,384 |
| Name / extra / file comment per entry | 4,096 / 16,384 / 16,384 bytes |
| Archive comment | 65,535 bytes |
| Aggregate names / extras | 4 MiB / 4 MiB |
| Aggregate file + archive comments | 1 MiB |
| Total extra TLVs | 65,536 |
| Requested entry-vector bytes | 2 MiB |
| Accounted work units | 16 MiB |

One vector uses `try_reserve_exact` after count/span/size checks; checked
`count * size_of::<ZipDirectoryEntry>()` must fit the metadata cap. No secondary
map/vector, per-entry heap allocation, recursion or input-driven decompression.
This bounds logical requested storage, not allocator overhead/capacity, RSS or
stack. No wall-clock deadline/performance claim is made.

Work charges each tested EOCD start, the declared directory bytes once before
parsing, name/comment bytes before UTF8 validation and each TLV. There is one
directory parse and no duplicate comparison. A conservative sum under the other
caps is `65,536 + 8,388,608 + 4,194,304 + 1,048,576 + 65,536 = 13,762,560`, below
16 MiB, so public work-limit failure is unreachable under this frozen contract.
Private accounting tests exercise exact/over/overflow refusal and unchanged
counters. The metadata cap is also unreachable at 16,384 entries on current
32/64-bit targets; private tests cover its layout-dependent floor and next count.
The defensive input cap cannot be exceeded through the public immutable
VerifiedArtifact API; integrity tests already cover its input boundary.

## Fixtures and evidence

All new byte layouts in `tests/zip_index.rs` are independently authored Rust test
data (2026-10-13). The fixed central-header/EOCD field offsets and asserted
metadata are independent of the production parser; no production writer exists.
There are no copied archives, zip-tool/JVM generators or decompression fixtures.
No external binary fixtures are required, so no fixture-file checksums are
invented. Existing approved `sha2` computes caller-provided digests solely to
construct VerifiedArtifact for these native tests; integrity vectors are separate.

The default native suite includes positive raw/UTF8/duplicate/path/claim data;
EOCD comments/ambiguity/trailing bytes; count/span/offset disagreements and fields
crossing into present EOCD/comment bytes; each flag bit, mixed unsupported
methods, version/disk/ZIP64/locator policies; exact/over field, aggregate, entry,
directory and TLV budgets; every proper nonempty byte-prefix truncation of three
single-EOCD fixtures; zero-byte rejection at the prerequisite API; deterministic
mutations and arbitrary bytes; immutable/lifetime/unverified-input compile-fail
examples. No-I/O evidence uses nonexistent/NUL provenance and retained bytes
after path replacement/deletion. A deliberately invalid local-header byte,
contradictory stored sizes, absent descriptor and extreme claimed expansion
sizes can still index successfully: this is intentional metadata-only evidence.

Implementation starts from approved prerequisite merge
`da5b3b4e75b41168a8428cc7f9d0d445c42ab64b`, tree
`a4baed56ed273c9070f506f3e535b9da62e9aee1`. The contract was separately reviewed by
functionality/adversarial reviewers and architect in
[#114](https://github.com/anilbeesetti/ktlsp/issues/114). Contract review is not
implementation/runtime evidence. Current execution policy prohibits local native
tests/parser/performance checks while [#96](https://github.com/anilbeesetti/ktlsp/issues/96)
remains OPEN/UNKNOWN; permitted local formatting and compile-only clippy are
separate from the existing Linux Native Rust CI runtime route. Final review,
exact reviewed-head CI and merged-main evidence belong in the PR/coordinator
report; no Linux result establishes Darwin reliability.

## Source and deferred scope

Primary format reference: [PKWARE APPNOTE.TXT 6.3.10 FINAL](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT),
revised 2022-11-01, read 2026-10-13; sections 4.3.12/4.3.16, 4.4/4.5, Appendix D.
Referenced layout only, no copied specification prose or implementation source.
The restrictions above are this finite child's compatibility policies, not a
claim that all rejected ZIPs violate the broader format.

Local-header/central agreement, descriptor/payload span and overlap validation,
CRC, bounded decompression/output/ratio/aggregate limits, JAR manifest,
multi-release/signature behavior, duplicate selection and safe consumers,
ZIP64/wider compatibility and authenticated digest acquisition require separately
reviewed children. No extraction safety, whole-archive/JAR validity, library
semantics, full T020, T008 admission, performance advantage or official Kotlin LSP
parity is established; parent #20 remains open. No server/import/project/SDK
consumer or capability is added.

# Selected original JImage 1.0 data (#398)

## Operation and authority

`index_jimage(&VerifiedArtifact)` returns a completely validated private
`JimageIndex` borrowing that **whole original artifact**. Its digest and legacy
coordinate/root/path provenance remain exactly caller-supplied and unverified;
they establish neither a Maven download nor an SDK/vendor association. There is
no raw-byte admission bypass, filesystem/network operation or inferred SDK path.
The existing 64 MiB `VerifiedArtifact` carrier ceiling is unchanged.

`index.select_resource(module_mutf8, resource_mutf8)` selects an exact pair of
canonical modified-UTF-8 byte strings, without terminators. It returns a private
`JimageResource` borrowing its originating index. The module cannot be empty or
contain `/`; the resource cannot be empty. Encoded NUL, supplementary UTF-16
surrogate pairs and unpaired surrogate units are preserved, never decoded
lossily. Four-byte UTF-8 and noncanonical encodings refuse. Names are data:
there is no path joining, separator/dot normalization, percent decoding or URI
mapping. Literal traversal-like spellings are not filesystem access.

The handle exposes immutable module/resource/full-image identity, original table
slot, location offset, full-file original stored span, compressed status,
original final size and the original artifact. There is no public constructor,
entry mutation or separately supplied payload/artifact. `resource.payload(scratch)`
borrows original bytes for stored data or caller scratch for decompressed data.
It clears scratch before every call and on every error. Hold the index/artifact
and scratch while borrowing results; compile-fail cases cover these boundaries.

`ktlsp_binary::parse_jimage_class(&resource, scratch)` uses the existing real
`parse_class`, then compares declared `this_class` losslessly with the selected
module-relative `.class` name. It returns the normal borrowed `ClassFile`, not
a second verified class artifact. Whole-image authority remains on the retained
resource handle. Payload, non-class selection, declared-name and class-reader
failures are distinct; all failures clear scratch. A small private error-only
cleanup guard uses a raw pointer to empty scratch only after failed class borrows
have ceased; it is disarmed before any borrowed class escapes. This avoids a
second parse/copy solely to satisfy the cleanup/borrowed-result contract.

## Pinned format and strict admission

Format evidence is OpenJDK `jdk-24-ga`, commit
[`6705a9255d28f351950e7fbca9d05e73942a4e27`](https://github.com/openjdk/jdk/tree/6705a9255d28f351950e7fbca9d05e73942a4e27).
Primary native format/hash sources are
[`imageFile.hpp`](https://github.com/openjdk/jdk/blob/6705a9255d28f351950e7fbca9d05e73942a4e27/src/java.base/share/native/libjimage/imageFile.hpp)
and [`imageFile.cpp`](https://github.com/openjdk/jdk/blob/6705a9255d28f351950e7fbca9d05e73942a4e27/src/java.base/share/native/libjimage/imageFile.cpp).
The original BSD notice is retained in `notices/OpenJDK-libjimage-BSD.txt`.
No GPL Java implementation is ported, compiled or executed. Java writer/reader
sources were read as format evidence, separately from the native BSD notice.

Both byte orders of `CAFEDADA` are recognized. The seven-u32 header is exactly
magic, version `00010000`, flags zero, resource count, table length, location byte
length and string byte length. Redirects are `T` signed u32-width values,
followed by `T` location offsets, location bytes, strings and resource bytes.
All sizes/spans use checked arithmetic. Header flags/versions refuse rather than
being guessed. Empty indexes (including zero slots) have a deliberate path.

Locations use `(kind << 3) | (width - 1)`, kinds 1..7 and 1..8-byte big-endian
values regardless of image endianness; absent values are zero. Attributes are
module, parent, base, extension, resource-relative offset, compressed size and
original final size. Zero ends a location. Duplicate/unknown kinds, other
kind-zero headers, truncated/unterminated streams, missing base, overlapping or
reused location streams and nonzero unindexed location bytes refuse. Zero gaps
and trailing location padding are allowed. These are deliberate stricter native
admission rules, not claims about OpenJDK's permissive Java reader.

All strings are scanned/validated **once**, including unreferenced strings and
padding. String offsets must name an actual NUL-delimited boundary, never the
middle of a string. The empty sentinel at offset zero is required. References
use a sorted compact boundary index, not repeated whole-table scans.

Identity reconstruction is `["/" + module + "/"] + [parent + "/"] + base +
["." + extension]`. Slash-containing base strings are retained, because internal
`/modules` and `/packages` records use them. All internal records count against
structural/count/storage/work limits, but the reserved `modules` and `packages`
identities (and moduleless records) are not selectable Java-module roots.

Hashing is wrapping `seed = seed * 01000193 XOR encoded_byte`, followed by
`& 7fffffff`. A first hash with seed `01000193` selects a signed redirect;
zero is absence, a negative value selects `-1-r`, a positive value is the second
hash seed modulo table size. Negative redirects use widened signed arithmetic,
including `INT_MIN` rejection. Every negative target must be a populated in-range
slot. Every admitted identity must route to its **actual original slot** before
any successful index/absence. Nonzero location count must equal resource count.
All complete reconstructed identities must be unique, even with different
attribute decompositions. The private complete index uses exact full-identity
binary search, not a hash candidate as proof of identity.

Each stored span is resource-region-relative, checked against the whole retained
image. Nonempty spans cannot overlap; physical order need not equal slot order.
Zero-sized spans at the end boundary are valid. Unclaimed image resource bytes
are not exposed or interpreted, and need not be zero. This is selected resource
consumption, not an interpretation of internal directory payloads.

## Compression

Location compressed size **zero** always means original uncompressed bytes.
Stored payloads are never probed for a compression magic.

Compressed resources use the exact packed 29-byte header, in image byte order:
`u32 CAFEFAFA`, `u64 payload_length`, `u64 layer_output_length`,
`u32 codec_string_offset`, `u32 compatibility/config`, `u8 terminal`.
The layer length excludes its header and must consume the entire current layer.
The final original length is frozen from the location, not replaced by an
intermediate layer length. Terminal must be 0 or 1, but does **not** control the
loop: pinned OpenJDK recognizes another compression header in decoded output.
See [CompressedResourceHeader](https://github.com/openjdk/jdk/blob/6705a9255d28f351950e7fbca9d05e73942a4e27/src/java.base/share/classes/jdk/internal/jimage/decompressor/CompressedResourceHeader.java)
and [Decompressor](https://github.com/openjdk/jdk/blob/6705a9255d28f351950e7fbca9d05e73942a4e27/src/java.base/share/classes/jdk/internal/jimage/decompressor/Decompressor.java).

Only codec `zip`, compatibility/config `ffffffff`, is implemented. This is
**zlib-wrapped DEFLATE**, not a ZIP directory, raw ZIP method-8 stream or ZIP CRC
policy. The installed `miniz_oxide` 0.9.1 bounded streaming core is used with
`TINFL_FLAG_PARSE_ZLIB_HEADER`, exact input/output checks and native Adler32
verification. Dictionary requests, bad wrapper/Adler, truncated/overlong output
and trailing compressed bytes fail. Existing miniz/adler notices/dependencies
are unchanged. No external tool or plugin is loaded.

Nested `zip` layers alternate two bounded output buffers. Unknown codecs,
`compact-cp`, any unsupported config and unsupported inner layers return
`JimageError::UnsupportedCompression`; no partially transformed class bytes,
stored fallback or lower-root retry can escape. Genuine `compact-cp`
reconstruction requires separate implementation and actual codec fixtures.

## Logical bounds (no raised existing limits)

| Resource | Ceiling |
|---|---:|
| Verified whole original image | existing 64 MiB |
| Whole index through string region | 8 MiB |
| Table slots / resources | 16,384 each |
| String boundaries | 65,536 |
| Individual raw string / reconstructed full identity | 4,096 bytes each |
| Aggregate reconstructed identity bytes | 4 MiB |
| Actual retained metadata capacities | 2 MiB |
| One complete index operation's modeled work | 16 Mi units |
| Selected stored span / original output / each layer output | 8 MiB each |
| All compression layers together | 8 |
| All layers' cumulative input + twice output + headers | 32 Mi units |
| Two live output Vec capacities, including supplied scratch | 16 MiB combined |
| Inflate input/output chunk | 32 KiB |
| Class reader | existing 16 MiB and existing version policy |

Index storage includes actual Vec capacities for entries, string-boundary
metadata and all retained encoded identities; capacity is preflighted before
fallible reserve and reconciled afterward. Tables and string bytes are borrowed.
Counts/reserves/identity emission, string validation, referenced-boundary searches,
hash passes, bounded numeric sorting and duplicate comparisons are charged to
one operation. Full-name heapsort is fallible and charges actual compared bytes,
including long shared prefixes, with no auxiliary allocation. No per-entry
fresh work allowance or entries-times-entire-string-table scan exists.

Decompression precharges each layer before output reserve/inflate; checks actual
retained capacities of both buffers; and never grants a fresh global budget to
an inner layer. An oversized caller scratch capacity refuses, even on a stored
path. Error clears length but retains caller capacity for reuse. Limits are
logical admission bounds, not RSS/CPU/OOM/sandbox guarantees; metadata/work
limits can bind before nominal count/input limits. Not all real runtime images
fit the unchanged 64 MiB carrier.

## Genuine fixture data and actual byte facts

Retained DATA comes only from MIT `hextriclosan/rusty-jvm`, pinned commit
[`792fda3b6bb64d8443c59cdb84ab347edd2b597e`](https://github.com/hextriclosan/rusty-jvm/tree/792fda3b6bb64d8443c59cdb84ab347edd2b597e).
Paths below are under `jimage-rs/tests/test_data/` and retained under
`tests/fixtures/jimage/`. Actual fetched length and Git blob SHA-1 were checked
against the four admission pins; SHA-256 and complete record/string/span/payload
inventories were calculated directly from those bytes as data, not by running
this Rust implementation, a generator, JDK or compiler. `inventory.json` includes
all 12 records per image, string bytes, payload hashes, small payload hex, layer
fields and unclaimed resource-tail bytes.

| Original path | Bytes | Git blob SHA-1 | SHA-256 |
|---|---:|---|---|
| `lib/non-compressed_little-endian.jimage` | 7535 | `4ddd3a7d010512b2f5192cf918488ee55874f0c3` | `6ec5a68e05f2e4f2b6eec5b88277c8ff3b97daf2cb4d66795be2e1fd74f332a8` |
| `lib/non-compressed_big-endian.jimage` | 7535 | `7aae383b2c34d5031896b2645630b2772d9f8327` | `9d962f97d8ba5f7b36d92e4673fedc67af653c1eff068bec36f27b5ebd4ff7a5` |
| `lib/compressed_little-endian.jimage` | 3640 | `19d43047defa62d38109b0f1fff126f3b1d30671` | `597286cc0850f048f02ef321ea1625eed7fb867d7d7ed9c791b23150e2004bfb` |
| `lib/compressed_big-endian.jimage` | 3640 | `def23c4a84ebe7638de2c054cec66c89f448447d` | `11c975ddb80f830008106601f53053c2ad263ce2996e6a669e5da3a8f6698b5c` |
| `mods/java.base/java/lang/Object.class` (`Object.class.bin`) | 160 | `4d67a10275ebbb7c5511bd9cb085690de4f77410` | `1467daa5037c5ca733b7042b3616630bde7413f75a9c281256ff5873ba25912d` |

The fixture MIT copyright/license is unchanged in `LICENSE-MIT` (1069 bytes,
Git blob `893ad4b50de47e1ef99a533c331b4cc2a8198070`, SHA-256
`58d0b6e4a236ff58b55a4242d1598bf60b9c512ce7d0c822b01e434ea272fc8e`).
Original provenance instructions are unchanged in `UPSTREAM-README.txt`
(471 bytes, blob `a9658b17f2c6bd321df6d51b2d059e0cf0524ab8`, SHA-256
`c55bb28558ece97ba81dc88f7986df5fc4587523647a63e85a1a90a01fb6d8e4`).
The unexecuted original generator
`utils/jimage_generators/src/main/java/com/github/hextriclosan/rustyjvm/jimage/ImageCreator.java`
is retained as `ImageCreator.java.txt` (2655 bytes, blob
`2d7460bf484c09b78e27f7e20845d2c7e7892b05`, SHA-256
`efa431101605332dec92c031f7a1172a4aac5b45e0c89d5de11ac952f44055c8`).
It calls actual OpenJDK `ImageFileCreator`/`DirArchive`/`DefaultImageBuilder` and
`DefaultCompressPlugin` configured `zip-9`. The generating JDK build is unpinned.
These are not hand-assembled ZIP substitutes.

Both plain images have header `(version=65536, flags=0, resources=12, slots=12,
locations=129, strings=171)`, resource start 424; compressed variants have
`(65536,0,12,12,143,175)`, resource start 442. There are five ordinary java.base
resources plus seven internal records, **not twelve Java modules**. The ordinary
resources are `java/lang/hank.txt` (6685 bytes), `java/lang/Object.class` (160),
`java/lang/without_ext` (29), `module-info.class` (148), `without_parent.txt` (25).
The extensionless payload is exactly `I'm file without an extension`. Object
is at slot 6 / location 63 / original span `[7109,7269)` in plain images and slot
6 / location 67 / original span `[3142,3306)` in compressed images. Its single
zip layer consumes 135 zlib bytes and outputs the exact original 160 bytes.
All four contain an unclaimed 8-byte resource tail, explicitly inventoried;
these bytes are retained in the whole artifact and never invented as a resource.

Object is an authored empty fixture at major **67**, minor **0**, not a stock
JDK implementation or proof of an actual SDK/LSP binding. Genuine coverage does
not include compact-cp, supplementary names or multiple ordinary modules;
separate independently authored adversaries cover name and cross-module cases.

## Checks and remaining limits

`tests/jimage.rs` and the binary crate's `tests/jimage.rs` cover all four public
index/selection/payload/class flows, original-byte equality, digest/provenance/
span/pointer facts, structural corruption, hash false absence/positives, exact
MUTF-8, internal/cross-module identity, spans, zlib/Adler/consumption, nested and
unsupported codecs, global/late capacity/work bounds, and class/name/version
failures. These are behavioral checks, not source-text/wording assertions.
They are authored but **UNEXERCISED in this component assignment**, as are
format, compile, lint, Rust data tests and external public-API smoke. ROOT owns
those later gates and the actual external payload/class tuple observation.

There is no new `ClasspathRoot` fork, SDK chooser, image discovery, importer,
server capability or Kotlin parser/runtime call. Standard `jrt:/java.base/path`,
virtual `/modules/java.base/path`, image `/java.base/path` and importer
`jrt:///jdk!/modules/java.base` remain distinct representations. Home paths do
not imply `jmods` or an image. Larger-than-64-MiB images, compact-cp, class versions
outside the existing reader policy, SDK association, full Java/Kotlin semantic
analysis and official Kotlin LSP parity remain unsupported or separate work.

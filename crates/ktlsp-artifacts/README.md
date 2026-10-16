# Exact offline artifact lookup (T020a / #61)

`Coordinate::new(group, artifact, version, classifier, extension)` validates an
exact coordinate; `lookup(&coordinate, &[absolute_repository_root, ...])` reads
one artifact from the Maven layout
`group/as/directories/artifact/version/artifact-version[-classifier].extension`.
Use classifier `Some("sources")` and extension `"jar"` for exact source lookup.
Coordinate getters and `relative_path()` are available to T021 and future
Maven/Gradle consumers. `ktlsp-binary` consumes selected archive class payloads;
no LSP capability or live project importer uses this library.

Success returns owned bytes and `Provenance` containing the coordinate, canonical
repository root, artifact path, actual byte count and `ChecksumStatus::Unverified`.
Lookup never reads checksum sidecars, validates ZIP/JAR contents, scans
directories, resolves transitive graphs, downloads, writes caches or runs tools.
Nonempty arbitrary bytes can succeed. Corrupt means empty entry, wrong filesystem
kind or an observed length change; it does not mean all data corruption is detected.

## Validation and errors

Fields use conservative ASCII letters/digits plus internal `.`, `-`, `_`, start
and end with a letter/digit, and are at most 128 bytes. Windows device basenames
are rejected case-insensitively. Group has at most 256 bytes/32 dot-separated
segments; aggregate field bytes at most 512; generated filename at most 255.
Dynamic versions/ranges, versions ending in `SNAPSHOT` (case-insensitively),
and timestamped snapshot forms (`base-YYYYMMDD.HHMMSS-build`) are unsupported.
Snapshot rejection follows [Maven's classification](https://maven.apache.org/ref/3.9.9/maven-artifact/xref/org/apache/maven/artifact/ArtifactUtils.html#L36).
Exact classifier/extension are required; no packaging/type aliases are inferred.

At most 16 explicit roots are accepted. Each must be absolute, have no parent
traversal, and use at most 4,096 encoded native-path bytes/256 components. Duplicate
normalized/canonical roots are rejected. Missing roots are allowed; absence in all
roots yields `Missing` with the searched roots. More than one nonempty regular
candidate yields `Ambiguous`, even if their bytes would match. Root/candidate
metadata, unsafe-path, size and corruption errors take precedence over
Missing/Ambiguous in supplied-root order. A unique candidate's open/read errors
are returned as `Io`; no successful root silently overrides a detected error.

Symlinks in existing root ancestors, directories or artifact leaves (including
dangling symlinks) are rejected. Intermediate entries must be directories and
leaves regular files. Callers must supply a canonical path when an intentional
alias such as macOS `/tmp` is involved. Root canonicalization happens only after
checking its components. Reads are limited to 64 MiB plus one sentinel byte,
independently of file metadata; oversize data returns `ArtifactTooLarge`.

This API is for trusted, quiescent local repositories. Standard-library pathname
checks cannot prevent hostile concurrent filesystem replacement, hard-link or
mount aliases; this is not a filesystem sandbox. Byte limits do not bound storage
latency or impose I/O deadlines. Provenance describes returned bytes; reopening
its path later may read different data. Consumers should use returned bytes.

## Verification and remaining work

### Caller-provided SHA-256 (T020b / #88)

`verify_sha256(artifact, expected: [u8; 32])` consumes an existing `Artifact`.
Before hashing it rejects actual empty bytes, actual size above 64 MiB, then an
inconsistent `provenance.byte_length`, in that order. `IntegrityError::Mismatch`
contains expected and actual digests. All errors consume and drop the artifact.

Success returns `VerifiedArtifact` with private fields and borrowed `bytes()`,
`provenance()`, `digest()` getters and `expected_digest_origin()` equal to
`ExpectedDigestOrigin::CallerProvided`. Verification hashes the existing byte
allocation without copying or accessing any path. Replacing/deleting the original
file cannot change the retained bytes. There is no mutable getter or unchecked
constructor. Original lookup provenance, including `checksum: Unverified`, stays
as supplied; the wrapper records the separate digest-equality evidence.

The caller is responsible for trusting the expected digest. Equality does not
authenticate that digest or the supplied coordinate/provenance, validate ZIP/JAR
contents, or guarantee anything about future path contents. Manually constructed
artifacts are accepted when their actual size/recorded length/digest agree.
No constant-time secret comparison is promised; digests here are public integrity
values. See [dependency and vector audit](INTEGRITY.md) and retained `notices/`.

### Selected ZIP payloads

After `index_zip_directory(&verified)`, `stored_payload(ordinal)` borrows method-0
bytes and `deflated_payload(ordinal, &mut scratch)` decodes method 8 into reusable
caller scratch. Shared validation checks selected local/central agreement,
name/extra framing, CRC and complete declared-envelope overlaps, including
competing descriptor tails. Bit 3 supports matching signed/unsigned non-ZIP64
descriptors with zero local CRC/size placeholders. Each compressed/decoded
payload is capped at 8 MiB; errors never return partial decoded bytes.

See [the payload policy and dependency audit](ZIP_STORED.md). Pure-Rust
`miniz_oxide` 0.9.1 uses no default or optional features; its active closure adds
only `adler2` 2.0.1. Published license notices are retained in `notices/`.
This is selected ordinal access, not archive-name selection, extraction,
multi-release/signature interpretation, whole-JAR validity or symbol resolution.

### JMOD 1.0 directory data

`index_jmod_directory(&verified)` checks exactly `JM\x01\x00`, then borrows the
embedded ZIP without copying or constructing another verified artifact.
`JmodDirectoryIndex::zip_index()` exposes the same directory and stored/deflated
payload operations. ZIP offsets, including errors, are relative to full-file
byte 4; the 64 MiB artifact cap includes the header. See the
[JMOD format and refusal policy](ZIP_INDEX.md#jmod-10-wrapper).
This is data only: no JDK/JVM execution, SDK discovery, section-name validation,
module visibility/resolution, or server capability.

### Selected JImage 1.0 resources

`index_jimage(&verified)` validates the complete endian-aware image index before
exact MUTF-8 module/resource selection. A private `JimageResource` retains the
whole image artifact, caller provenance, slot, location and original stored span.
Stored resources borrow original bytes; native `zip` uses bounded zlib/Adler32
decoding, including nested layers. Unknown configurations/codecs and `compact-cp`
refuse explicitly, without partial output or fallback. See [JIMAGE.md](JIMAGE.md).
The existing 64 MiB carrier limit remains; this does not discover or bind an SDK.

Non-JImage fixtures are independently authored temporary local files. JImage
tests additionally retain the four pinned MIT upstream images and original class
as data with full provenance/inventory; no generator is executed. Default tests
are Rust-only and execute no JVM/Gradle/Maven. No official oracle comparison is claimed.
Parent T020 remains open. Authenticated checksum acquisition and bounded archive verification,
remote transport/cache mutation, lockfiles, dependency graphs, metadata-driven
snapshot/dynamic resolution and Gradle layouts need subsequent child tickets.

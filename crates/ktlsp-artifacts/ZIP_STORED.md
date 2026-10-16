# Bounded selected ZIP payloads

`ZipDirectoryIndex` borrows retained `VerifiedArtifact` bytes. Its payload APIs
select exactly one central-directory ordinal, never a name or duplicate winner:

```rust
impl<'a> ZipDirectoryIndex<'a> {
    pub fn stored_payload(&self, ordinal: usize) -> Result<&'a [u8], ZipStoredError>;
    pub fn deflated_payload<'s>(
        &self,
        ordinal: usize,
        scratch: &'s mut Vec<u8>,
    ) -> Result<&'s [u8], ZipDeflateError>;
}
```

Method 0 returns a slice of the original artifact allocation with no payload
copy or allocation. Method 8 inflates raw DEFLATE into reusable caller scratch;
returned bytes borrow scratch, not a self-referential archive wrapper. No path is
reopened, no entry is extracted, and neither operation accesses the network.

## Shared validation

Both paths share selected local/central agreement, name/extra framing, envelope
and CRC validation. Directory construction remains metadata-only and unchanged;
index success alone does not establish payload validity. Selected access checks:

1. Ordinal exists and the selected method matches the requested API. Flags may
   contain only UTF-8 bit 11 and descriptor bit 3 (`0`, `0x0800`, `8`, `0x0808`).
   Directory-accepted DEFLATE option bits 1/2 remain unsupported by payload APIs.
2. Selected local fixed header is wholly before the directory, has the local
   signature, supported version 10..=20, no encryption or ZIP64 sentinels, and
   agrees with central version, flags and method.
3. Without bit 3, local CRC and both sizes must equal central claims. With bit 3,
   **all three local fields must be zero**; populated or mixed placeholders are
   rejected rather than guessed. Stored central compressed/decoded sizes agree.
4. Local raw name equals the central name. Local name/extra limits remain
   4,096/16,384 bytes. Extra TLVs must be bounded and reject ZIP64/encryption;
   unknown extras are opaque and need not equal central extras.
5. Central compressed size alone delimits the stream. That complete range must
   end before the directory; descriptor bytes are never inflater input.
6. Bit 3 requires a matching unsigned 12-byte or signed 16-byte descriptor at
   exactly the stream end. CRC/compressed/decoded fields must match central
   claims. Ambiguous forms, disagreement and truncation reject. A CRC equal to
   the optional signature is not sufficient to choose the signed form. Matching
   ZIP64 descriptor forms are rejected, including otherwise matching 32-bit
   prefixes; their wider tail must not be mistaken for an unvalidated gap.
7. The selected complete header/name/extra/payload/**descriptor** envelope must
   not intersect any competing declared envelope. Competitor header signatures,
   bounds, central compressed spans and descriptor tails are checked in central
   order. Competing local/central bit-3 disagreement rejects instead of hiding a
   descriptor. A malformed unselected descriptor can conservatively deny access.
   Unselected CRCs, names, extra bodies and full local agreement are not checked.
8. Stored bytes or completely decoded output must match the central ZIP CRC-32
   (IEEE reflected polynomial `0xedb88320`, initial/final xor `0xffffffff`).

Overlaps exclusively between two unselected entries remain unchecked. Hidden
local records, gaps/prefixes, archive-wide validity, extraction/path safety,
manifest/multi-release/JAR signature semantics and semantic identities are not
provided. Duplicate and unsafe names remain raw metadata.

## Bounds and failure behavior

Every limit is per call, not a session, and precedes decoded scratch growth:

- Stored payload: at most **8 MiB**; existing stored work cap **16 MiB**.
- DEFLATE compressed input: independently at most **8 MiB**.
- DEFLATE declared decoded output: at most **8 MiB**.
- Inflate work: **32 MiB** units, conservatively precharged for bounded headers,
  local name/extra, compressed bytes, declared output and its CRC scan. Header
  precharge also covers competing descriptors. No compression-ratio heuristic.
- Streaming core calls receive at most **32 KiB** compressed input and produce
  at most **32 KiB** output per call, retaining history in the caller buffer.

The inflater must finish, consume exactly the central compressed byte range and
produce exactly the declared decoded length. Malformed/truncated streams,
trailing compressed bytes, too much/little output, no progress and CRC mismatch
reject. Scratch is cleared on entry and on **every payload-access failure**; no
partial slice is returned. Existing scratch capacity may be reused. Allocation
failure is typed. Bounds are logical requested storage/work, not RSS, allocator
capacity or a wall-clock deadline.

`ZipStoredError` retains ordinal, structural, disagreement, overlap and CRC
variants with numeric offsets/static labels only. `ZipDeflateError` wraps shared
payload/structural errors and distinguishes invalid stream, trailing input,
output-length disagreement, excess output and allocation failure. No error
includes raw names/comments or silently retries another ordinal.

## Production consumer and regression evidence

`ktlsp_binary::parse_zip_class(index, ordinal, scratch)` dispatches method 0 to
borrowed stored bytes and method 8 to scratch, then invokes existing
`parse_class`. Classfile errors are returned as `ZipClassError::Class` without
fallback. Decoding success does not establish JVM verification or a symbol.

The committed genuine unchanged Commons Lang 3.17.0 JAR, its origin SHA-256,
member hashes and Apache notices live in `ktlsp-binary/fixtures/t017c1`. The
consumer regression uses a temporary explicit canonical Maven repository,
`lookup`, caller-provided SHA-256, directory indexing and ordinal 31 to assert
exact `AppendableJoiner$Builder` identity, fields, methods and original bytes.
Default tests need no network, Java or archive tools. Consumer-visible hostile
cases cover both methods/descriptors, placeholders, CRC, malformed/truncated/
trailing streams, exact sizes/limits, overlapping descriptor tails and ZIP64.

The existing 108 stored fixtures retain their original bytes and digests; four
expectations intentionally change for descriptor support/extent validation.
Earlier frozen-contract/review records describe the historical stored-only
implementation, not the extended API. In the authorized owner verification wave,
format check, locked workspace/all-targets Clippy and artifact/binary tests
passed. A separate offline throwaway API consumer also observed Builder class
52.0, flags `0x0031`, Supplier interface, four fields, seven methods and exact
2,510 decoded bytes/member SHA-256. This is data-consumer evidence, not server
integration, full JAR validation, parser-runtime evidence or Kotlin LSP parity.

## Dependency closure and notices

`miniz_oxide = "=0.9.1"`, default features disabled and **no features enabled**,
provides the pure-Rust allocation-free inflate core. Its only active dependency
is `adler2` 2.0.1 (defaults disabled). Published manifests have no build script,
C/JVM dependency, SIMD or serde feature in this configuration. Published crate
SHA-256 values match the lockfile:

- miniz_oxide: `b63fbc4a50860e98e7b2aa7804ded1db5cbc3aff9193adaff57a6931bf7c4b4c`
- adler2: `320119579fcad9c21884f5c4861d16174d0e06250625266f50fe6898340abefa`

Unchanged published MIT/Apache/Zlib and 0BSD/MIT/Apache notices are retained in
`notices/`. No dependency implementation code was copied into this crate.

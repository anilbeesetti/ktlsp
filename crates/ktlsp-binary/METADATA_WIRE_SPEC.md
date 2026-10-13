# T018a2 / #108 bounded wire contract

This child frames decoded d1 bytes and reads flat raw fields only. Parents #68/#18
remain open. No names, members, types, schema validation, compiler compatibility
or LSP parity is established. No dependencies, Cargo/lock or provider changes.

`metadata::wire::decode(&[u8], Limits)` borrows bytes from the existing
`metadata::Outcome::Decoded`. Callers must match that supported outcome (mv=[2,3,0],
k=1/2/5). The bytes-only wire API cannot enforce header kind/version and does not
reinterpret Absent/Unsupported. Header/d1 decoding retains its own existing budget.

Leading rawVarint32 length frames StringTableTypes content; the remaining bytes
are class/package content. Prefix, both sections and every complete field retain
borrowed original bytes and absolute half-open byte spans. Field order, duplicates
and unknown numbers 1..=536870911, including reserved 19000..19999, are retained.
Values are raw unsigned varint/fixed64, fixed32 (both fixed forms little-endian),
or borrowed opaque length-delimited bytes. No embedded/packed/UTF8/required-field
interpretation, recursion or depth machinery. Empty input is truncated framing;
`[0]` is two empty flat sections, without establishing valid Class metadata.

Varints terminate within ten bytes. Premature EOF is truncation; ten continuation
bytes are malformed. Nonminimal encodings are accepted and preserved. Tags and
lengths retain low32 bits, matching pinned rawVarint32 readers; interpreted signed
negative lengths fail. Wire0 deterministically retains low64 bits, including only
the low bit of the tenth byte. Mathematical overflow is low-bit truncation, not
canonical numeric equality. The pinned JVM raw64 fast path tests the tenth byte
without merging its payload and can disagree with slow path for nonminimal ten-byte
forms. This decoder follows the deterministic slow-path rule; universal JVM reader
compatibility is unestablished. Negative/short envelope rejection is intentionally
stricter than protobuf 2.6.1 LimitedInputStream's negative/early-EOF behavior.
Field zero and wires 6/7 fail; groups 3/4 are explicitly unsupported.

Hard limits: 16 MiB total input, 65,535 records, 64 Mi work; callers can only tighten.
One shared budget covers prefix and both sections. Envelope costs one record,
leaving 65,534 combined fields. Only a nonempty remaining section attempts a field;
clean EOF costs nothing. Each record charges one work unit before its tag/prefix.
Every read byte and opaque skipped byte costs one work unit, once. Failed byte
reads charge one attempted byte. Failed fixed/payload/envelope availability checks
charge available bytes plus one failed attempt with checked arithmetic. Successful
envelope availability checks charge no contents; section scanning charges them.
Lengths/remaining bounds are checked before slicing; only bounded field vectors
allocate, never declared-length payloads.

Absolute error offsets and deterministic precedence:

- Initial input-byte limit: 0, before parsing.
- Record limit: envelope/tag start, before its work charge.
- Work limit: current byte cursor, or requested bulk-read start, before truncation.
- Truncation: first missing byte, i.e. current section end.
- Malformed varint: tenth continuation byte.
- Field-number/wire/group errors: tag start; field zero precedes wire validation.
- Negative length: length-varint start.

Record/work errors take precedence over subsequent wire/truncation errors at that
operation. Error returns no partial sections. Corpus expectations are independent
byte transcription; every-byte truncation allows complete remainder-field endings.
Tests exercise shared limits, failed-read work, raw spans and supported-header
composition. No local test/runtime execution: coordinator authorized local format
and compile checks, plus existing Linux Native Rust CI's unchanged three workspace
commands on exact reviewed combined head and exact merged main. #96 stays open,
cause unknown. Two independent reviewers and finished architect review are required.

Source identity and notices are in tests/fixtures/metadata_wire/README.md. Kotlin
pin is the same as #97, v2.3.20 d57eb4a26ed01ab03cf195da686c8047032ff16f. Base
protobuf 2.6.1 commit bba83652e1be610bdb7ee1566ad18346d98b843c is source-only evidence;
no upstream code was executed or added to the native dependency graph.

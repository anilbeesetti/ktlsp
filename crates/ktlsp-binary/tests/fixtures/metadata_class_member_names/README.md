# Class member name composition fixtures

`mixed.hex` and `minimal.hex` are independently authored project data. Expected
names/order/spans/flags/cuts/errors/budgets were frozen before implementation in
EXPECTATIONS.md and the ticket-local spec. No compiler/JVM/oracle was executed.

The integration test reuses the existing unmodified licensed
`fixtures/t017c2/Unit.class`; provenance and notices remain there. Its SHA256 is
896588212a98f36148f2fbe1ea2878d6ba23c6376c6346c765cec5ac0c5716ce.
Metadata schema and resolver rules retain existing pinned Kotlin v2.3.20 and
protobuf provenance from METADATA_PROPERTY_HEADER_SPEC.md,
METADATA_NAME_RESOLVER_SPEC.md and METADATA_PAYLOAD_SPEC.md. The three notice/license
files are copied byte-for-byte from metadata_wire. No Java source/runtime copied.

Names and ordinals are encoded data, not semantic symbol identity. Nested
constructor/alias/type bodies remain opaque; malformed selected protobuf UTF8
remains UnsupportedUtf8. Parent #68/#18 stay open and T008/T008a stay HOLD.

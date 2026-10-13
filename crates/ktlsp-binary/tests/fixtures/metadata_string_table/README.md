# #125 data provenance

Raw hexadecimal fixtures and EXPECTATIONS.md are independently authored, with no
encoder/decoder execution. Kotlin v2.3.20 commit
d57eb4a26ed01ab03cf195da686c8047032ff16f is reused unchanged from metadata_wire.
Pinned sources read as data:
core/metadata.jvm/src/jvm_metadata.proto SHA256
5a80e63021b800afd2297b3713c4cfc12739cbc8e62c726578e1c5d1dfc771f9;
core/metadata.jvm/src/org/jetbrains/kotlin/metadata/jvm/JvmProtoBuf.java SHA256
b1ad864ae872315c06740d6551d2d7233060f5bff3e9ad9b8f80f531018bc3a1.
The generated Java parser establishes defaults/presence/wire compatibility; it is
neither copied into this crate nor run. Existing metadata_wire protobuf2.6.1 pin
bba83652e1be610bdb7ee1566ad18346d98b843c and notices remain unchanged.
Apache LICENSE/NOTICE and protobuf BSD notice are copied unmodified alongside.
Unit.class reused from ../../../fixtures/t017c2/Unit.class without modification;
its provenance.json and licenses remain authoritative. No JVM/compiler/oracle ran.

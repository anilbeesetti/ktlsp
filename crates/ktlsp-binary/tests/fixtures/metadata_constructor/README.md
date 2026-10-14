# Constructor fixture provenance — contract only

selected.hex,duplicates.hex,opaque.hex are independently authored literal data.
pair_constructor.hex is manually transcribed from genuine unchanged kotlin/Pair.class
in already-existing kotlin-stdlib2.3.20 archive; PAIR_EXTRACTION.json records exact
hashes/spans/provenance and intended, unexecuted composition. Two parameter bodies
are confirmed from bytes; no product/native/JVM/compiler/oracle execution.
Compiler provenance UNVERIFIED; distribution version is not compiler proof.

Kotlin source pin v2.3.20 d57eb4a26ed01ab03cf195da686c8047032ff16f:
metadata.proto SHA256 a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2;
ProtoBuf.java SHA256 da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716.
Sources read as temp data only, never shipped or run. Existing metadata_wire
README pins protobuf2.6.1 and wire caveats; retain those rules and notices.
LICENSE.txt/NOTICE.txt/PROTOBUF_LICENSE.txt copied byte-identically from metadata_wire.
Expectations independently frozen before source/runtime; no encoder under test.
No semantic, full initialization, upstream runtime-equivalence or performance claim.

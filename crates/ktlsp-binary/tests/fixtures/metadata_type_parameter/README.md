# TypeParameter fixture provenance

minimal.hex, mixed.hex and scalars.hex are independently authored project data,
frozen before the decoder. EXPECTATIONS.md is the unchanged reviewed v2 literal,
error/offset/default/span/budget acceptance freeze, SHA256
66205a7fa8864754354f7669509e325cd06499fd6a951f3550761c38c302c177.
No Kotlin compiler, JVM, oracle, encoder under test or product runtime produced them.

Kotlin v2.3.20 commit d57eb4a26ed01ab03cf195da686c8047032ff16f:
core/metadata/src/metadata.proto SHA256
a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2,
Git blob18cfd13e9e512c491f2f62ec241f49c81d6e5e81; TypeParameter177-197.
core/metadata/src/org/jetbrains/kotlin/metadata/ProtoBuf.java SHA256
da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716,
Git blobfc424d5e7aac8661deb862c8e791d4fcf779c194; TypeParameter7569 onward.
Patched CodedInputStream.java SHA256
7fbb3aeebd3c3d5414d7171bc3ba2785de50b40863a4f381c4b43986e15784fe;
readBool382 retains raw64!=0. Existing metadata_wire protobuf2.6.1 source pin,
slow-path limitations and provenance remain unchanged. All inspected as data only;
generated Java/reference runtime is neither shipped nor executed.

LICENSE.txt, NOTICE.txt and PROTOBUF_LICENSE.txt copied byte-for-byte from
../metadata_wire. Hex-text SHA256 in minimal/mixed/scalars order:
527916ed3220bd77678ab2733cce74cdd43cb2c26e77cc1185456e794c49271a,
27d37ce23b0b82de012014d119d7c796feea8fcf379b5457952bff2fda371af9,
87038e62bed5d80a866c64fee16aa320b3edecd227de8a40de543d1c713119b5.

Bound Type messages, annotations and extensions are opaque; success is not full
message initialization, generic semantics, names/types or LSP parity. Selected
malformed-UTF8 name-resolver GAP remains. Header/d1/table/resolver use separate
budgets; requested retained capacity is not whole-pipeline RSS or allocator churn.

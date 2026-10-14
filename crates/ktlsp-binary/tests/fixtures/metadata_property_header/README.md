# Selected Property-header fixture provenance

All four hex files are independently authored project data, frozen and approved
before implementation. EXPECTATIONS.md is the unchanged literal acceptance freeze
SHA256 0c15f92b2703a3455186ccfc13a20e64d80d1e10782dea7636fbcef0640b40b1.
No compiler, JVM, oracle or native decoder generated these expected results.

Schema/generated source pin (inspected as data only): Kotlin v2.3.20 commit
d57eb4a26ed01ab03cf195da686c8047032ff16f; metadata.proto SHA256
a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2,
blob18cfd13e9e512c491f2f62ec241f49c81d6e5e81; ProtoBuf.java SHA256
da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716,
blobfc424d5e7aac8661deb862c8e791d4fcf779c194. Wire/protobuf source pins remain
the unchanged ../metadata_wire/README.md baseline. LICENSE.txt, NOTICE.txt and
PROTOBUF_LICENSE.txt are copied byte-for-byte from that directory. Generated
Java/reference source and upstream runtime are not shipped.

Hex-text SHA256 in minimal/explicit/duplicates/opaque order:
87d20bd55a831dd84147d50239ef1ab8f4dce4efa12683ed7cca784b1ececef7,
f3623edc4b4f5215a6725e9ab59f9c022dbdd96a31c14efa4461b9d7ff5ee92a,
7412aa305cc42e6f95c66e124acc8b161e4037ca14895c01231f7cf62753ba77,
0b49f66c5b3eeb1847396f02e58a0441ef6ed2078604b899e12f8ba9df5b4c28.

Selected-header success permits opaque malformed nested fields and missing nested
required fields. It does not establish full Property initialization, semantic flag
inheritance, name/type applicability or official runtime parity. No genuine compiled
Property or oracle comparison is claimed. Header/d1/table/resolver operations have
separate budgets. Parents #68/#18 remain OPEN; Darwin #96 UNKNOWN.

# #108 wire-only data provenance

`mixed.hex` is independently authored, without any encoder under test: a 19-byte
string-table flat section with fields (1,varint300), (2,bytes ff0080), (1,varint1),
(19000,varint7), (9,fixed32 12345678), then a 27-byte remainder with fields
(1,fixed64 0102030405060708), (536870911,fixed32 89abcdef), (100,varint42),
(2,empty bytes), (2,opaque bytes 0b0c). Numeric fixed values are hexadecimal and
little-endian on wire. These are flat wire fixtures, not schema-valid metadata.
Expected spans/values are independently transcribed in metadata_wire.rs.

The data-only precompiled Unit.class is reused unmodified from
../../../fixtures/t017c2/Unit.class, artifact kotlin-stdlib:2.3.20, SHA256
896588212a98f36148f2fbe1ea2878d6ba23c6376c6346c765cec5ac0c5716ce.
Its JAR/extraction/license provenance remains in that fixture's provenance.json.
No compiler/JVM/Gradle/Maven/oracle was executed. Unit's independently transcribed
expected wire values are in the test; it demonstrates composition only.

Read-only source inspection at Kotlin v2.3.20 commit
d57eb4a26ed01ab03cf195da686c8047032ff16f (same pin as #97), GitHub contents API:

| Path | SHA256 |
| --- | --- |
| core/metadata.jvm/src/org/jetbrains/kotlin/metadata/jvm/deserialization/JvmProtoBufUtil.kt | d24dbeeb0d4213d5096c7c818ee892b1b6cd530f97499ca52443e6433c082c52 |
| core/metadata.jvm/src/jvm_metadata.proto | 5a80e63021b800afd2297b3713c4cfc12739cbc8e62c726578e1c5d1dfc771f9 |
| core/metadata/src/metadata.proto | a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2 |
| dependencies/protobuf/protobuf-patches/src/main/java/com/google/protobuf/CodedInputStream.java | 7fbb3aeebd3c3d5414d7171bc3ba2785de50b40863a4f381c4b43986e15784fe |
| dependencies/protobuf/protobuf-relocated/build.gradle.kts | 973a652217996d51ce1f6a45284ad5588e3c95f5fd626e2ecbd114967fb38a0a |
| dependencies/protobuf/build.gradle.kts | edbd5f3a64aeb45ab7885c81cb60a498625db7822ea7543ac02fb2c27d6051c5 |

The latter files establish patched CodedInputStream relocation and base protobuf
2.6.1. That version's source tag resolves to immutable commit
bba83652e1be610bdb7ee1566ad18346d98b843c (protocolbuffers/protobuf):
java/src/main/java/com/google/protobuf/AbstractParser.java SHA256
d791dd30cacbdea459c91ff3b06f5ee74d35e048c09376b4898008e1ee5975b8;
AbstractMessageLite.java SHA256
abacc3234b9402b8ce116f35fe75cc51356fdf4760f5f8ba0293043f37602ea6.

Kotlin framing/schema sources: Apache-2.0, accompanying LICENSE.txt/NOTICE.txt
preserved from the existing pinned metadata_header fixture. Protobuf wire source:
Google BSD notice in PROTOBUF_LICENSE.txt. Rust code is independently structured;
source-derived rules retain both notices. No generated code or upstream runtime
is bundled. Deterministic raw64 and stricter envelope limits are explicit in
METADATA_WIRE_SPEC.md; no universal JVM/schema compatibility claim.

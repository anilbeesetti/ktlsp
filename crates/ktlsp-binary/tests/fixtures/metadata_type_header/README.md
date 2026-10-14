# Selected Type-header fixtures

Literal expectations were frozen before source/runtime in EXPECTATIONS.md,
SHA256 b74771ca41036bdc0e5d1a96409e173b459a46f513d7255d0846a4d647a47dd4.
selected.hex and opaque.hex are independently authored project data. unit_type.hex
is the unchanged inline return Type from existing ../../../fixtures/t017c2/Unit.class,
d1[43..45], Function-relative[4..6]. It is not compiler/encoder/decoder output.
Original Unit.class provenance and Apache-2.0 notices remain authoritative:
org.jetbrains.kotlin:kotlin-stdlib:2.3.20, class SHA256
896588212a98f36148f2fbe1ea2878d6ba23c6376c6346c765cec5ac0c5716ce,
JAR SHA256 0ae12504a5040ebaf37703908483420d1a5624dd1d93f357665f8c77c848a01e,
distribution2.3.20-release-208; compiler provenance UNVERIFIED.

Schema/generated sources inspected only as data: Kotlin v2.3.20 commit
 d57eb4a26ed01ab03cf195da686c8047032ff16f, metadata.proto Type121–175
and ProtoBuf.java Type parser4944–5065/default6088. Patched CodedInputStream
readBool uses raw64!=0. Full source hashes/rules are in METADATA_TYPE_HEADER_SPEC.md.
Existing protobuf2.6.1 bba83652e1be610bdb7ee1566ad18346d98b843c baseline and
LICENSE.txt/NOTICE.txt/PROTOBUF_LICENSE.txt are preserved byte-for-byte from
metadata_wire. No upstream source/runtime is shipped, no new artifact generated.
No JVM/compiler/Gradle/Maven/oracle execution. Local native tests are held under
#96; validation evidence belongs to the exact original hosted CI receipt.

Raw duplicate preservation is not nested-message merging/initialization, type
resolution, semantic nullability, linkage, Kotlin/LSP parity or a RAM/speed claim.

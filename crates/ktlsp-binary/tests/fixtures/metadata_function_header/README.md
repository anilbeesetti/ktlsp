# Selected Function-header fixture provenance

EXPECTATIONS.md preserves the unchanged preimplementation FIXTURES.md freeze,
SHA256 fba565092833f4e2ac9c9240befb257896ba401d6cbe10c393e48cafe006eee8.
minimal/explicit/duplicates/opaque.hex are independently authored project data.
unit_function.hex is the unmodified10-byte body extracted from existing licensed
../../../fixtures/t017c2/Unit.class, not compiler/encoder/decoder output.

Original Unit.class artifact provenance remains in that fixture's provenance.json:
org.jetbrains.kotlin:kotlin-stdlib:2.3.20, unmodified781B, SHA256
896588212a98f36148f2fbe1ea2878d6ba23c6376c6346c765cec5ac0c5716ce;
JAR SHA256 0ae12504a5040ebaf37703908483420d1a5624dd1d93f357665f8c77c848a01e.
Compiler provenance UNVERIFIED; Implementation-Version identifies distribution only.
UNIT_EXTRACTION.json is actual data-only constant-pool transcription evidence,
SHA256 3a1da1537abbf80b80ea407c528e91c6a95055a1ee00861fa8fd11394779efa0:
CP31 classfile290..352 -> leading NUL+low8 d1 -> body d1[39..49], SHA256
c40ba211d99c8575494344d811f0af5168a99adeaac7f864c146d2c211ff9f99.
Separate name resolver can resolve name3 to toString. No compiler/JVM/oracle/
Gradle/Maven/product/native test execution was used to obtain these bytes.

Schema/generated Java reference pin, source inspected as data only:
Kotlin v2.3.20 commit d57eb4a26ed01ab03cf195da686c8047032ff16f;
metadata.proto SHA256 a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2,
Git blob18cfd13e9e512c491f2f62ec241f49c81d6e5e81;
ProtoBuf.java SHA256 da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716,
Git blobfc424d5e7aac8661deb862c8e791d4fcf779c194.
Wire/protobuf2.6.1 source pins remain ../metadata_wire/README.md baseline.
LICENSE.txt/NOTICE.txt/PROTOBUF_LICENSE.txt copied byte-for-byte from that directory.
Generated Java/reference source and upstream runtime are not shipped.

Five hex-text SHA256 in minimal/explicit/duplicates/opaque/unit_function order:
87d20bd55a831dd84147d50239ef1ab8f4dce4efa12683ed7cca784b1ececef7,
52d96c12adfcf8b2abd714703c09664f0ddd7a15e7db78589623fb4d1c17b75a,
b4c1ca46004a74f83b225863c847256c236265126ebe64f06931ae9ad8d8eb70,
0900f92cdb3a11e8695a23277fe6a3b8f1877e7207fc02074cd3d7a30a65b2be,
69e0d233301c83c153b56d3bbedec1b5f69c47e9308c1cf5ad370c6d8e313d76.

Selected-header success permits opaque malformed nested fields and missing nested
required fields. It establishes neither full Function initialization nor semantics.
Name lookup and header/d1/payload/wire scans are separate bounded operations.

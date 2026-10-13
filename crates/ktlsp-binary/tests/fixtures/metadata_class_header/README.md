# #146 selected Class-header fixture provenance

EXPECTATIONS.md is the unchanged preimplementation FIXTURES.md freeze, SHA256
c44057b6cf511c01d3eb2025bbadf2861aba14f149ce6347fbc819179fe39f23.
Four raw hex files are independently authored data, not generated/encoded by the
decoder or a compiler. SHA256 in minimal/explicit/duplicates/opaque order:
d6fe409dff0d63cf5338c5850681a22e66814ee8d7be31eb348b7cc372b68f84,
3f4b794929759c7591ca8b58551575fa380069bcbc2a76d417863cb6f9ed853e,
3f651367288bea02b9fc05be00d645beb751bf32e8bc8b5268c9423791266f80,
1b2599be14e937859efcfb2dd4c97c169624e39852e851c4f719954023c937fe.

Pinned Kotlin v2.3.20 d57eb4a26ed01ab03cf195da686c8047032ff16f:
core/metadata/src/metadata.proto SHA256
a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2,
Git blob18cfd13e9e512c491f2f62ec241f49c81d6e5e81;
core/metadata/src/org/jetbrains/kotlin/metadata/ProtoBuf.java SHA256
da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716,
Git blobfc424d5e7aac8661deb862c8e791d4fcf779c194.
Exact raw source was read as data, never executed or bundled. An earlier auxiliary
connector-content/apply_patch schema copy added one trailing newline and hashed
a34605a82b345ef19de542e0d769cb01a1372740af47a9b0d9a7098870e6a06d;
that copy is retained outside repo as retrieval evidence, not the pinned bytes.
Wire source/protobuf2.6.1 pins remain ../metadata_wire/README.md baseline.
Apache LICENSE/NOTICE and protobuf BSD notice copied unchanged from that directory.

Reuse ../../../fixtures/t017c2/Unit.class unmodified, SHA256
896588212a98f36148f2fbe1ea2878d6ba23c6376c6346c765cec5ac0c5716ce.
Its original artifact/JAR/license/provenance.json remain authoritative. Existing
header/wire tests supplied independent d1/field transcriptions; new test checks
selected flags326/fq0 and separate payload/resolver composition only.
No precompiled artifact or original provenance changed. No Java/compiler/JVM/oracle/
Gradle/Maven execution. New tests are authored, not locally executed.

Additional source-stage manual coverage (frozen expectations unchanged):
`18 00 4a 80 08` +1024ff has bytes1029/fields2/work1033. work1032 fails final
dispatch at2; work1033 succeeds, while nested malformed content stays opaque.
This is independently counted N+2F, not an observed decoder result.

# Selected ValueParameter fixture provenance

Four hex fixtures are independently authored project data. EXPECTATIONS.md freezes
literal results, all61cuts, budget arithmetic and error offsets before implementation;
SHA256 4c76ba39fc6906e992bb412e77fbd195edbc77f46d1f89aa4e8d735d8a92e32b.

Read-only schema/generated-source pin: Kotlin v2.3.20 commit
 d57eb4a26ed01ab03cf195da686c8047032ff16f.
core/metadata/src/metadata.proto SHA256
 a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2;
core/metadata/src/org/jetbrains/kotlin/metadata/ProtoBuf.java SHA256
 da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716.
ValueParameter generated parser23924..23990 selects scalars1/2/5/6, merges
singular messages3/4/8 and appends annotation7. This selected decoder retains
nested occurrences only in raw; it performs no nested interpretation or validation.

Existing wire/protobuf2.6.1 pins and caveats remain ../metadata_wire/README.md.
LICENSE.txt/NOTICE.txt/PROTOBUF_LICENSE.txt copied unchanged from that directory.
Reference Java/schema/runtime is neither bundled nor executed. No genuine artifact
or official runtime observation is claimed. #68/#18 remain open.

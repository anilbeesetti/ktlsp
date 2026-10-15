# TypeAlias fixture provenance

Five hex files are independently authored project data, not compiler/encoder/
decoder output or an extracted official TypeAlias. EXPECTATIONS.md preserves the
preimplementation freeze unchanged, SHA256
e637048f7a70760644f56db28e969372984bc790daf85543eea283f167883e8e.

Schema/parser references inspected as data only, same pin as metadata_function_header:
Kotlin v2.3.20 commit d57eb4a26ed01ab03cf195da686c8047032ff16f.
core/metadata/src/metadata.proto blob18cfd13e9e512c491f2f62ec241f49c81d6e5e81,
SHA256 a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2;
TypeAlias lines450..474. Generated ProtoBuf.java
blobfc424d5e7aac8661deb862c8e791d4fcf779c194,
SHA256 da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716;
TypeAlias parser25229..25282 assigns scalar int32s and MERGES singular Type messages.
This selected decoder retains every inline body for future correct interpretation.

LICENSE.txt/NOTICE.txt/PROTOBUF_LICENSE.txt are byte-identical copies from
../metadata_wire. Existing wire/protobuf reference pins remain there. No generated
Java/reference runtime ships; no JVM/JDK/compiler/oracle/Gradle/Maven ran here.
Opaque malformed nested bodies can succeed; no full metadata/alias/semantic claim.

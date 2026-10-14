# TypeTable directory fixture provenance

Four independently authored literal hex streams freeze ordered/empty/duplicate
opaque Type bodies and presence-preserving first_nullable data. EXPECTATIONS.md
is the unchanged preimplementation contract fixture freeze. No compiler, JVM,
oracle or native product execution generated these bytes.

Pinned reference schema, read as data: Kotlin v2.3.20 commit
`d57eb4a26ed01ab03cf195da686c8047032ff16f`, metadata.proto Git blob
`18cfd13e9e512c491f2f62ec241f49c81d6e5e81`, SHA256
`a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2`;
ProtoBuf.java blob `fc424d5e7aac8661deb862c8e791d4fcf779c194`, SHA256
`da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716`.
Existing wire/protobuf source pins remain in ../metadata_wire/README.md.
License/notice files copied byte-for-byte from that directory. Reference Java
is neither shipped nor executed. Empty/malformed Type bodies are intentionally
opaque; success does not establish nested initialization or semantic validity.

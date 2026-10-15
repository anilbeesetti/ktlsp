# Class TypeParameter composition fixtures

All hex files except pair_class.hex are independently authored literals. Original
PR285 CI exposed an authored nonminimal child-length typo: correct84→88 to encode
the intended8 bytes. EXPECTATIONS.md retains the preimplementation tuples and
documents that erratum (SHA25645c32441c9be86bdd5d651cadd11998ec33cc63983f79df2e5d5171a432a18b0).
The original accepted freeze SHA256d89f57198113b5fa090993f4a058a224497fac974323ffe953b95fa583ef2af5
and failed evidence remain preserved separately; the current copy is corrected.
EXPECTATIONS.json is its compact index; tests assert literal tuples from this
freeze, not decoder-produced expectations.

pair_class.hex is exactly the existing licensed Kotlin Pair decoded d1[47..285].
The native pipeline test reuses the unchanged metadata_constructor/Pair.class.bin;
no artifact/classfile copy, repack, compiler, download or JVM is needed. Source
authoring ran no product/native tests. Compiler provenance remains UNVERIFIED.
PROVENANCE.json records existing evidence pins and the static transcription.

LICENSE.txt/NOTICE.txt/PROTOBUF_LICENSE.txt are copied byte-for-byte from the
existing metadata_type_parameter notices. Preserve all three. Indices and bounds
remain data; no name/type reconstruction, semantic symbols or server capability.

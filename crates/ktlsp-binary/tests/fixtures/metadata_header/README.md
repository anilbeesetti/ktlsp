# T018a1 data provenance

No JVM, Kotlin compiler, Gradle or Maven runs are needed. Native tests reuse
`../../../fixtures/t017c2/Unit.class` unmodified, artifact
`org.jetbrains.kotlin:kotlin-stdlib:2.3.20`, SHA256
`896588212a98f36148f2fbe1ea2878d6ba23c6376c6346c765cec5ac0c5716ce`.
Original JAR SHA, extraction provenance and license/notice hashes are retained in
`../../../fixtures/t017c2/provenance.json`. The expected Unit d1 bytes were
independently transcribed from its constant-pool MUTF8 using `xxd`; no encoder
under test generates expected results. Synthetic expected vectors are authored
in `tests/metadata_header.rs` and state the bit-level derivation.

Encoding rules are derived from JetBrains Kotlin v2.3.20, commit
`d57eb4a26ed01ab03cf195da686c8047032ff16f`, Apache-2.0:

- `libraries/stdlib/jvm/runtime/kotlin/Metadata.kt`
- `core/metadata.jvm/src/org/jetbrains/kotlin/metadata/jvm/deserialization/BitEncoding.java`
- `core/metadata.jvm/src/org/jetbrains/kotlin/metadata/jvm/deserialization/utfEncoding.kt`

Pinned public sources were read through GitHub's contents API at that tag.
Copyright 2000-2018 JetBrains s.r.o. and Kotlin Programming Language contributors.
LICENSE.txt and NOTICE.txt accompany the derived Rust encoding implementation;
copied from the existing fixture's pinned Kotlin license/notice without changes.

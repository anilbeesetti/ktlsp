# Data-only parameter annotation fixture

`ReadAfterEOFException.class.bin` is the unchanged
`kotlin/io/ReadAfterEOFException.class` ZIP entry from
`org.jetbrains.kotlin:kotlin-stdlib:2.3.20`. The archive is the same existing
artifact recorded by `fixtures/t017c2/provenance.json`; no compiler or JVM ran.
The entry contains a runtime-invisible `Nullable` annotation on the encoded
constructor parameter. Tests assert the exact method descriptor and annotation.

`provenance.json` pins the public artifact URL, archive/class SHA256, entry, size,
distribution version and license/notice origins and hashes. The manifest's
`2.3.20-release-208` identifies the distribution; exact compiler provenance is
unknown. LICENSE/NOTICE are retained unchanged from the existing Kotlin fixtures.
The existing `fixtures/t017c2/Metadata.class` supplies defaults without a duplicate
binary copy. These are data fixtures, not product dependencies or oracle evidence.

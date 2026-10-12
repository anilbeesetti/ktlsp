# Exact offline artifact lookup (T020a / #61)

`Coordinate::new(group, artifact, version, classifier, extension)` validates an
exact coordinate; `lookup(&coordinate, &[absolute_repository_root, ...])` reads
one artifact from the Maven layout
`group/as/directories/artifact/version/artifact-version[-classifier].extension`.
Use classifier `Some("sources")` and extension `"jar"` for exact source lookup.
Coordinate getters and `relative_path()` are available to T021 and future
Maven/Gradle consumers. No consumers or LSP capabilities are integrated here.

Success returns owned bytes and `Provenance` containing the coordinate, canonical
repository root, artifact path, actual byte count and `ChecksumStatus::Unverified`.
The library never reads checksum sidecars, validates ZIP/JAR contents, scans
directories, resolves transitive graphs, downloads, writes caches or runs tools.
Nonempty arbitrary bytes can succeed. Corrupt means empty entry, wrong filesystem
kind or an observed length change; it does not mean all data corruption is detected.

## Validation and errors

Fields use conservative ASCII letters/digits plus internal `.`, `-`, `_`, start
and end with a letter/digit, and are at most 128 bytes. Windows device basenames
are rejected case-insensitively. Group has at most 256 bytes/32 dot-separated
segments; aggregate field bytes at most 512; generated filename at most 255.
Dynamic versions/ranges and `*-SNAPSHOT` indirection are unsupported. Exact
classifier/extension are required; no packaging/type aliases are inferred.

At most 16 explicit roots are accepted. Each must be absolute, have no parent
traversal, and use at most 4,096 encoded native-path bytes/256 components. Duplicate
normalized/canonical roots are rejected. Missing roots are allowed; absence in all
roots yields `Missing` with the searched roots. More than one nonempty regular
candidate yields `Ambiguous`, even if their bytes would match. Root/candidate
metadata, unsafe-path, size and corruption errors take precedence over
Missing/Ambiguous in supplied-root order. A unique candidate's open/read errors
are returned as `Io`; no successful root silently overrides a detected error.

Symlinks in existing root ancestors, directories or artifact leaves (including
dangling symlinks) are rejected. Intermediate entries must be directories and
leaves regular files. Callers must supply a canonical path when an intentional
alias such as macOS `/tmp` is involved. Root canonicalization happens only after
checking its components. Reads are limited to 64 MiB plus one sentinel byte,
independently of file metadata; oversize data returns `ArtifactTooLarge`.

This API is for trusted, quiescent local repositories. Standard-library pathname
checks cannot prevent hostile concurrent filesystem replacement, hard-link or
mount aliases; this is not a filesystem sandbox. Byte limits do not bound storage
latency or impose I/O deadlines. Provenance describes returned bytes; reopening
its path later may read different data. Consumers should use returned bytes.

## Verification and remaining work

Fixtures are independently authored temporary local files; default tests are
Rust-only and execute no JVM/Gradle/Maven. No official oracle comparison is claimed.
Parent T020 remains open. Integrity/checksum and bounded archive verification,
remote transport/cache mutation, lockfiles, dependency graphs, metadata-driven
snapshot/dynamic resolution and Gradle layouts need subsequent child tickets.

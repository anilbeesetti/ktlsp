# PR174 official-capture preparation inputs

These are unchanged copies of `docs/specs/lowering-acceptance/declarations.kt`
and `error-node.kt` from PR174 (`44f77b9`), with the reviewed eight-message
script. They are source data, not official observations or expected symbol
responses. The script opens each exact UTF-8 source and requests document symbols
sequentially, then shutdown/exit. It adds no readiness requests or replies to
unexpected server requests.

| Input | Bytes | SHA256 |
| --- | ---: | --- |
| declarations.kt | 146 | fa935b144a49838fd9e603bfc95462bcf3701eeb19877501d451ca069ce9e233 |
| error-node.kt | 44 | c052b78e7603a88f3531f5b06f3a533f8df5394ce8ce9eeab3eb2b3ce3f4bd2c |
| script.json | 2351 | 930a87ea1fa9c3f6e6a6b6008d010dd9435082329ea8b897a42084d7369ab008 |
| archive-members.json | 193661 | 4fbbe3b1f6a8d8cc98c559c86f629487ed91f5e3daf3cee6f84499d93d7fae26 |

The input validator compares actual input bytes with embedded bytes.
Supplied archive/launcher/hash claims are compared with pins, not authenticated.
Consistent proposed identity/preflight data validates successfully, granting no
launch authority. No output or process is created. No Java/archive is included.

## Retained distribution provenance

`archive-members.json` is the unchanged retained archive-header inventory from
the PR174 capture preparation (`provenance/archive-members.json` in the
retained contract). It describes JetBrains Kotlin LSP `263.6379.0`, Linux
amd64, not an executed server or an official response. It contains 1120 member
records: 778 regular files, 221 symlinks and 121 directory records, totaling
1199734503 uncompressed member bytes. The four repeated directory names
(`kotlin-server-263.6379.0` and its `lib`, `license`, `plugins` directories)
are retained unchanged; this does not authorize duplicate file writes or
conflicting directory metadata.

The exact original distribution is:

- Publisher archive URL:
  <https://download.jetbrains.com/language-server/kotlin-server/263.6379.0/kotlin-server-263.6379.0.tar.gz>
- Archive size: **368488700 bytes**.
- Archive SHA256:
  `ab8ca4455dc2fc5fe1a24db2bccc46c104254d2c465155c4251ee65df8f3f7cc`.
- Publisher checksum URL:
  <https://download.jetbrains.com/language-server/kotlin-server/263.6379.0/kotlin-server-263.6379.0.tar.gz.sha256>
- Retained checksum response: **97 bytes**, SHA256
  `502c8061f56ce5d3008ab866bfff3f85c38fcf32012ab259ec27f48eab6bd6d9`,
  with exact content (including a final LF):

  ```text
  ab8ca4455dc2fc5fe1a24db2bccc46c104254d2c465155c4251ee65df8f3f7cc *kotlin-server-263.6379.0.tar.gz
  ```

The retained preparation contract records acquisition over TLS from JetBrains
and equality of the adjacent publisher checksum and downloaded archive SHA256.
Retained HTTP headers record a publisher redirect to `download-cdn.jetbrains.com`
and a final HTTP 200 with the exact archive length, plus HTTP 200 for the checksum.
The retained archive and checksum bytes were also hashed directly when placing
this inventory. A co-hosted checksum is not an independent signature, proof of
source/binary correspondence, or launch authorization.

Preserve all distribution notices during any separately authorized
materialization: the complete `kotlin-server-263.6379.0/license/` directory,
including third-party-library notices and launcher notices, and the complete
`kotlin-server-263.6379.0/jbr/legal/` directory, including license texts,
additional license information, assembly exceptions and their in-tree links.
The unchanged inventory retains these member records and link targets; it does
not replace the notice contents in the original archive. No notices are removed
from the retained distribution. Neither that archive nor any vendor JVM/native
executable is copied into this fixture directory.

This inventory is optional preparation data only. It adds no dependency to the
default native build or tests, supplies no permission to download or execute the
distribution, and establishes no measured host, image or runtime facts.

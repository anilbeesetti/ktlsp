# Preparation provenance and genuine-specimen boundary

All12hex files are independently authored literal data; no compiler/JVM/oracle,
product decoder/encoder or native tests generated their bytes/expected outcomes.
Frozen data hashes and source/license hashes appear in HASHES.json. All sources
were copied read-only from existing prior contract source caches and compared to
committed pin hashes, not new research. CONTRACT.md pins exact original paths,
commits/blobs/hashes/line ranges. License/notice files are byte-identical copies
of tests/fixtures/metadata_wire/{LICENSE.txt,NOTICE.txt,PROTOBUF_LICENSE.txt}.

Existing genuine licensed Kotlin2.3.20 specimens inspected as data only:
- fixtures/t017c2/Unit.class, SHA256
  896588212a98f36148f2fbe1ea2878d6ba23c6376c6346c765cec5ac0c5716ce.
  Existing independently transcribed d1 in tests/metadata_header.rs and
  tests/fixtures/metadata_function_header/UNIT_EXTRACTION.json contains no Class
  field32/version_requirement_table (and no selected requirement record).
- fixtures/t017c2/Metadata.class, SHA256
  41096af263c5d71999c1bd876985999f728fc4530b84051b3f0349a05d8e45a0.
  Data-only xxd inspection of d1 CP bytes0x286..0x444 (MUTF8) shows a Class body
  with flags/fq_name/supertype/constructor/property/module-name fields, no root32.
- tests/fixtures/annotation_extensions/ReadAfterEOFException.class.bin, SHA256
  77f69a26812780209557707c6d3858bc7edb1ecd26058a3adeab346315c57e5a.
  Data-only xxd d1 inspection shows Class flags/fq_name/supertype/constructor/
  module-name fields and no root32. Existing Apache2 notices/provenance remain.

The remaining committed class specimens are Java (Commons/Guava), not positive
Kotlin metadata requirement coverage. This finite inspection supplies no genuine
VersionRequirement/Table positive fixture; no such coverage is claimed. Do not
fabricate a genuine specimen by inserting authored fields. If later root accepts
a genuine field-bearing artifact, pin its exact immutable archive/class/hash/
license and independently freeze extraction/expected values before native runtime.
This child currently uses only authored positive fixtures; genuine pipeline and
semantic availability evidence remain gaps. Artifact distribution version is not
verified compiler provenance. No new artifact download/compiler execution occurred.

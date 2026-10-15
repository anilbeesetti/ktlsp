# Authored Code type-annotation bytes

all_targets.hex is hand-authored from the already pinned JVMS SE23 encoding;
it is not compiler output or official Kotlin oracle evidence. No third-party
bytes are copied. CP slots supplied by tests:4 `LA;`,5 `value`,6 integer raw bits
0xfedcba98. The outer Code has four bytecode bytes, two local slots and one
catch-all exception row. Targets40..4b appear once each. Local40 has full range
(slot0) and zero-length range at PC3 (slot1); resource41 covers PCs0..2 slot1.
Catch42 selects row0. Offsets are0/3/1/2/0/3/1/2/3, and type argument indices
are0/255/1/2/255. Target44 has path[(0,0),(1,0),(2,0),(3,255)]. All roots
reference LA; target4b has value=[Int(0xfedcba98),Annotation(LA;,[])]. Other
roots have no elements. The hex payload is one visible OR invisible attribute.
Tests build outer Code framing around these exact authored payload bytes.

Existing genuine Unit.class is reused unchanged only to check empty results
through parse_class -> code::decode. Existing fixture notices/provenance remain
in fixtures/t017c2; no genuine Code type annotations are claimed.

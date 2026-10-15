# One-level Type/Argument fixtures

Nine literal fixtures and EXPECTATIONS.md were frozen and independently SAME3
reviewed before implementation. pair_type.hex is the already-pinned14byte Type
literal from metadata_type_argument/PAIR_EXTRACTION.json, not fresh acquisition.
Actual genuine composition reuses ../metadata_type_argument/Pair.class.bin.
That existing Apache-2.0 LICENSE.txt/NOTICE.txt and PROTOBUF_LICENSE.txt remain
unchanged; PROVENANCE.json pins their exact bytes, artifact hash and source/schema
references. No artifact copying/regeneration or compiler/JVM/oracle is needed.

Malformed inline Type00 deliberately stays opaque. projection/type_id Options
preserve absence; all repeated Arguments and inline bodies retain encoded order.
No message merge/initialization, lookup, substitution/type semantics or parity.
Phase2 source is exported and passes local format/compile-only Clippy checks.
No local native tests ran; original hosted runtime outcomes remain pending.

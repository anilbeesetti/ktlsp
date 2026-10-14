# Selected JVM method-signature fixture — historical preimplementation freeze

selected.hex(V3) and merged.hex(V4) are independently authored literals, frozen
before any product source/runtime. pair_parent.hex is byte-identical to existing
metadata_constructor/pair_constructor.hex. No encoder/compiler/oracle generated
these files. EXPECTATIONS revision2 freezes all values/spans/cuts/errors/budgets.

Genuine fixture is REUSED, not duplicated: ../metadata_constructor/Pair.class.bin,
org.jetbrains.kotlin:kotlin-stdlib:2.3.20 entry kotlin/Pair.class2983bytes SHA256
8fed0fa88eb2df5723812a95330dee6bc91fd66b13906acb7906f9a813da9736.
Existing PAIR_EXTRACTION.json documents JAR/source/license/compiler limitations.
PAIR_SIGNATURE_EXTRACTION.json records independent data-only CP/metadata/method
transcription, explicit signature indices7/8 and encoded method CPrefs8/9.
Future Rust composition MUST use the actual delivered resolver and classfile APIs;
the delivered test now contains that assertion; its execution awaits original
Linux CI. This historical data freeze predates the explicit source grant. No omitted inference,
constructor identity, source parameter mapping, linkage or semantic claim.

Kotlin source pin d57eb4a26ed01ab03cf195da686c8047032ff16f; proto/generated-source
hashes in SPEC. Protobuf GeneratedMessageLite source pin
bba83652e1be610bdb7ee1566ad18346d98b843c independently confirms repeated singular
MESSAGE merge. Source copies/extraction Python are auxiliary data-only evidence
outside repository, never product source/dependency or required native test.
LICENSE/NOTICE/PROTOBUF_LICENSE copied byte-identically from existing Constructor
fixtures; original upstream notices retained. Compiler provenance UNVERIFIED.

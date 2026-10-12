# Parity status

Baseline `kotlin-lsp/v263.6379.0`; merged checkpoint `19e6242cde54146c5e8f988379e36e771562ac7e` (2026-10-12). Isolated official observations and optional native recorded replay exist; no fresh official JVM ran in this delivery wave. Exact merged evidence and pending assignments belong in [the development ledger](development.md); assignment does not deliver a provider. Native verification and official comparison are separate:

| Requirement | Native status | Official comparison | Blocking work |
| --- | --- | --- | --- |
| Lifecycle/framing | native-tested for bounded stdio v1 | recorded shutdown/exit match only; initialize/notifications differ, original wire bytes unavailable | T002/T005 delivered; advanced progress/async lifecycle remains |
| UTF-16 document synchronization | native-tested for open documents | unverified | T001/T005 delivered; filesystem/watch integration remains |
| Lossless bounded lexical classification | native-tested | unverified | T003 delivered; opaque templates are lexical only |
| Parser safety adapter | native-tested bounded supervised worker; no server consumer | unverified | #62 delivered; 64 KiB versus 8 MiB documents, cumulative allocation cap is not RSS |
| Revisioned parser results/declaration corpus | native-tested bounded #86 contract; named recovery failures retained, no server consumer | unverified | #83/#7 open; missing-parameter/generic recovery loses later declaration nodes, T008a blocked |
| Complete Kotlin syntax/recovery | pending | unverified | T007b/#83 recovery gate before T008; safety/corpus alone is not full grammar acceptance |
| Token-aware folding | partial; T006 lexical subset native-tested over open documents | recorded block lines/kind match; line-only character/collapsedText mismatch retained | T006 delivered; imports/declarations/line-comment groups and full folding parity remain |
| Lexical/delimiter diagnostics | native-tested for bounded replacement/clearing push diagnostics | unverified | T006 delivered; distinct from compiler diagnostics |
| Document symbols | pending | standalone declaration fixture observed; native comparison unverified | T008, T027 |
| Scopes/imports/module resolution | pending | unverified | T009–T011 |
| Kotlin types/overloads/smart casts | pending | unverified | T012–T016 |
| Classfile structure reader | native-tested data-only subset; opaque attributes, Java 23 ceiling, synthetic fixtures | unverified | #58 delivered; raw attributes retained, signatures/nesting/metadata remain |
| Declaration annotations | native-tested bounded visible/invisible class/field/method data decoder; no semantic consumer | unverified | #69 delivered; parameter/type/default annotations #71 and metadata interpretation remain |
| Binary/Java/SDK/library symbols | pending | unverified | T017–T019 |
| Explicit project-model v1 loader | native-tested; data-only library | unverified | T004 delivered; symlink discovery and live import remain |
| Exported workspace.json model | partial; #63 pinned data-only subset native-tested; ordered sidecar retained | pinned source contract only; runtime unverified | #21 open; SDK/resources/scoped-exported libraries/facets/compiler/build metadata block atomically; no server wiring |
| Offline artifact lookup | native-tested exact Maven-layout byte/provenance subset; checksum unverified, no consumers | unverified | #61 delivered; #20 open for checksum/archive verification, graphs, remote/cache/snapshot resolution |
| Gradle/Maven/Android imports | pending | unverified | T004, T020–T023; existing-tool orchestration not implemented |
| JPS .idea/.iml compatibility | pending | source registered; runtime unverified | T048 |
| Completion/resolve | pending | unverified | T024 |
| Hover/navigation/references/highlights | pending | unverified | T025–T027 |
| Compiler/inspection/library diagnostics | pending | unverified | T028–T030 |
| Semantic tokens/inlay hints/signatures | pending | unverified | T031–T033 |
| Rename/move/extract/inline/intentions | pending | unverified | T034–T037 |
| Formatting/organize imports | pending | unverified | T038, T039 |
| Type hierarchy | pending | globally advertised; Kotlin language coverage unresolved | T049 |
| Call hierarchy | pending | unverified | T040 |
| Official custom protocol compatibility | pending | unverified | T041 |
| Main Run/Debug code-lens descriptors | pending | unverified | T047; execution tracked separately |
| Run/debug external-tool interoperability | pending | unverified | T042; user-existing JDK permitted, none bundled |
| Incrementality/cancellation/performance | pending | unverified | T043, T044 |
| Recorded-observation replay | native-tested optional standalone tool; six separate tool tests | genuine baseline differences, shutdown/exit matches only | #59 delivered; #45 open for full corpus/fresh captures/dynamic interactions/process-tree cleanup |
| Release packaging | pending | unverified | T046; Windows/cross-platform release builds and full matrix audit remain |

Ticket closure means its specified subset is delivered; it never implies broader feature parity. Later master-only additions are recorded in the official research report and require a future baseline update.

## Delivered ticket evidence

T001–T005 deliver their explicitly scoped contracts ([T005 PR56](https://github.com/anilbeesetti/ktlsp/pull/56), merge `8ea54a9bee315713e7fc74c713eb1954901b5657`). Historical checkpoint `0d54973` passed 76 native tests, including 24 server tests. Subsequent reviewed deliveries are PR65 hygiene, PR70 replay, PR72 classfile structure, PR76 exported model, PR73 T006, PR75 parser safety, PR74 offline artifact lookup and PR92 parser-result/corpus validation and PR79 declaration annotations. Recorded runs at `19e6242` passed 155 active native tests plus two ignored subprocess helper fixtures invoked by active tests; 38 server tests include real Java-free subprocess and provider coverage. Recurring local 500 ms timeouts of all eight declaration tests, including an isolated default-harness run, remain unexplained in [#96](https://github.com/anilbeesetti/ktlsp/issues/96); passing runs/CI are observations, not a reliability fix. Each implementation passed separate functionality/adversarial reviews, architect review, pinned native checks and exact-head hosted CI. Independent generated probes are documented in crate review evidence; they do not prove all edge cases or official parity.

The optional replay tool records differences without normalization; its published baseline uses historical revision `0d54973`. PR73 separately compares the captured official block fixture and replays it against the integrated native binary, preserving genuine differences: native line-only folding omits characters/collapsedText, and initialize/notifications still differ. Matching shutdown/exit and fold lines/kind is limited evidence, not complete provider verification. T045 remains open. Local macOS arm64 and hosted Linux execution are verified for declared native subsets; Windows execution, cross-platform packaging, native semantics and performance thresholds remain pending.

No faster-execution or lower-memory comparison is established. Native baseline work and narrow fixture replay must retain their workload and evidence limits; see [verification](../specs/verification.md).

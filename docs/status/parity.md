# Parity status

Baseline `kotlin-lsp/v263.6379.0`; isolated official runtime observations exist in docs/research/oracle, but no native differential verification has run at checkpoint `0d54973ce66f59362829360261fde6e46b4fc420` (2026-10-12). Active assignments and later evidence belong in [the development ledger](development.md); assignment does not deliver a provider. Native verification and official comparison are separate:

| Requirement | Native status | Official comparison | Blocking work |
| --- | --- | --- | --- |
| Lifecycle/framing | native-tested for bounded stdio v1 | unverified | T002/T005 delivered; advanced progress/async lifecycle remains |
| UTF-16 document synchronization | native-tested for open documents | unverified | T001/T005 delivered; filesystem/watch integration remains |
| Lossless bounded lexical classification | native-tested | unverified | T003 delivered; opaque templates are lexical only |
| Complete Kotlin syntax/recovery | pending | unverified | T007, T008 |
| Token-aware folding | pending | official block fixture observed; native comparison unverified | T006; broader folding coverage remains |
| Lexical/delimiter diagnostics | pending | unverified | T006; distinct from compiler diagnostics |
| Document symbols | pending | standalone declaration fixture observed; native comparison unverified | T008, T027 |
| Scopes/imports/module resolution | pending | unverified | T009–T011 |
| Kotlin types/overloads/smart casts | pending | unverified | T012–T016 |
| Binary/Java/SDK/library symbols | pending | unverified | T017–T019 |
| Explicit project-model v1 loader | native-tested; data-only library | unverified | T004 delivered; symlink discovery and live import remain |
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
| Oracle corpus/release packaging | pending | unverified | T045, T046 |

Ticket closure means its specified subset is delivered; it never implies broader feature parity. Later master-only additions are recorded in the official research report and require a future baseline update.

## Delivered ticket evidence

T001–T005 deliver their explicitly scoped contracts ([T005 PR56](https://github.com/anilbeesetti/ktlsp/pull/56), merge `8ea54a9bee315713e7fc74c713eb1954901b5657`). Each code ticket passed separate functionality and adversarial review agents, locked native checks and hosted CI. At checkpoint `0d54973` the integrated workspace has 76 native tests, including 24 server tests; server subprocess tests remove Java environment variables and use an empty PATH. Independent generated probes are documented in crate review evidence; they do not prove all edge cases or official parity.

No native provider has yet passed a recorded differential comparison with the official server. The oracle directory contains limited official observations, not a reusable full differential harness. T045 remains open. Linux execution is verified; macOS/Windows execution, packaging, native semantics and performance thresholds remain pending.

No faster-execution or lower-memory comparison is established. Native baseline work and narrow fixture replay must retain their workload and evidence limits; see [verification](../specs/verification.md).

# Parity status

Baseline `kotlin-lsp/v263.6379.0`; isolated official runtime observations exist in docs/research/oracle, but no native differential verification has run. Initial rows will be updated as implementation merges:

| Requirement | Native status | Official comparison | Blocking work |
| --- | --- | --- | --- |
| Lifecycle/framing | pending | unverified | T002, T005 |
| UTF-16 document synchronization | pending | unverified | T001, T005 |
| Complete Kotlin syntax/recovery | pending | unverified | T003, T007, T008 |
| Folding/document symbols | pending | unverified | T006, T008 |
| Scopes/imports/module resolution | pending | unverified | T009–T011 |
| Kotlin types/overloads/smart casts | pending | unverified | T012–T016 |
| Binary/Java/SDK/library symbols | pending | unverified | T017–T019 |
| Gradle/Maven/Android imports | pending | unverified | T004, T020–T023; existing-tool orchestration not implemented |
| JPS .idea/.iml compatibility | pending | source registered; runtime unverified | T048 |
| Completion/resolve | pending | unverified | T024 |
| Hover/navigation/references/highlights | pending | unverified | T025–T027 |
| Compiler/inspection/library diagnostics | pending | unverified | T028–T030 |
| Semantic tokens/inlay hints/signatures | pending | unverified | T031–T033 |
| Rename/move/extract/inline/intentions | pending | unverified | T034–T037 |
| Formatting/organize imports | pending | unverified | T038, T039 |
| Type hierarchy | pending | official advertises; standalone prepare returned null | T049 |
| Call hierarchy | pending | unverified | T040 |
| Official custom protocol compatibility | pending | unverified | T041 |
| Main Run/Debug code-lens descriptors | pending | unverified | T047; execution tracked separately |
| Run/debug external-tool interoperability | pending | unverified | T042; user-existing JDK permitted, none bundled |
| Incrementality/cancellation/performance | pending | unverified | T043, T044 |
| Oracle corpus/release packaging | pending | unverified | T045, T046 |

Ticket closure means its specified subset is delivered; it never implies broader feature parity. Later master-only additions are recorded in the official research report and require a future baseline update.

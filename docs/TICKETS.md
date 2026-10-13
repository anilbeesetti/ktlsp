# Dependency-ordered tickets

Canonical acceptance text is in linked GitHub issues when created. Review and merge requirements apply to every implementation ticket.

| ID | Ticket | Depends on |
| --- | --- | --- |
| [T001](https://github.com/anilbeesetti/ktlsp/issues/1) | UTF-16 document model and atomic versioned edits | none |
| [T002](https://github.com/anilbeesetti/ktlsp/issues/2) | Bounded LSP stdio frame reader and writer | none |
| [T003](https://github.com/anilbeesetti/ktlsp/issues/3) | Lossless bounded Kotlin lexer | none |
| [T004](https://github.com/anilbeesetti/ktlsp/issues/4) | Validated native explicit project-model manifest | none |
| [T005](https://github.com/anilbeesetti/ktlsp/issues/5) | Runnable stdio server lifecycle and document synchronization | T001,T002 |
| [T006](https://github.com/anilbeesetti/ktlsp/issues/6) | Syntax-only folding and delimiter diagnostics | T003,T005 |
| [T007](https://github.com/anilbeesetti/ktlsp/issues/7) | Evaluate and bound pure-Rust full Kotlin parser | [T003](https://github.com/anilbeesetti/ktlsp/issues/3) |
| [T008](https://github.com/anilbeesetti/ktlsp/issues/8) | Error-recovering Kotlin declarations and CST/HIR lowering | [T007](https://github.com/anilbeesetti/ktlsp/issues/7) |
| [T009](https://github.com/anilbeesetti/ktlsp/issues/9) | Lexical scopes and resolved source symbol identity | [T008](https://github.com/anilbeesetti/ktlsp/issues/8) |
| [T010](https://github.com/anilbeesetti/ktlsp/issues/10) | Packages/default/explicit/star/alias import resolution | [T009](https://github.com/anilbeesetti/ktlsp/issues/9) |
| [T011](https://github.com/anilbeesetti/ktlsp/issues/11) | Module visibility and dependency-aware source indexing | T004,T010 |
| [T012](https://github.com/anilbeesetti/ktlsp/issues/12) | Kotlin type representation and subtyping | [T009](https://github.com/anilbeesetti/ktlsp/issues/9) |
| [T013](https://github.com/anilbeesetti/ktlsp/issues/13) | Generic substitution and constraint inference | [T012](https://github.com/anilbeesetti/ktlsp/issues/12) |
| [T014](https://github.com/anilbeesetti/ktlsp/issues/14) | Kotlin call overload and receiver resolution | T010,T013 |
| [T015](https://github.com/anilbeesetti/ktlsp/issues/15) | CFG and flow-sensitive smart casts | T012,T014 |
| [T016](https://github.com/anilbeesetti/ktlsp/issues/16) | Kotlin conventions contracts and special declarations | T013,T015 |
| [T017](https://github.com/anilbeesetti/ktlsp/issues/17) | Bounded JVM classfile and Java generic signature reader | none |
| [T018](https://github.com/anilbeesetti/ktlsp/issues/18) | Kotlin metadata protobuf and name resolver | [T017](https://github.com/anilbeesetti/ktlsp/issues/17) |
| [T019](https://github.com/anilbeesetti/ktlsp/issues/19) | Dependency/SDK symbols sources and Java interoperability | T011,T014,T018 |
| [T020](https://github.com/anilbeesetti/ktlsp/issues/20) | Native dependency artifact resolution and cache | [T004](https://github.com/anilbeesetti/ktlsp/issues/4) |
| [T021](https://github.com/anilbeesetti/ktlsp/issues/21) | Official workspace.json import compatibility | [T004](https://github.com/anilbeesetti/ktlsp/issues/4) |
| [T022](https://github.com/anilbeesetti/ktlsp/issues/22) | Conservative declarative Maven project import | T020,T021 |
| [T023](https://github.com/anilbeesetti/ktlsp/issues/23) | Conservative Gradle and Android project import | T020,T021 |
| [T024](https://github.com/anilbeesetti/ktlsp/issues/24) | Semantic completion candidates and resolution | T014,T019 |
| [T025](https://github.com/anilbeesetti/ktlsp/issues/25) | Typed hover and KDoc rendering | T014,T019 |
| [T026](https://github.com/anilbeesetti/ktlsp/issues/26) | Definition type-definition and implementation navigation | T014,T019 |
| [T027](https://github.com/anilbeesetti/ktlsp/issues/27) | References highlights and workspace/document symbols | T011,T014,T019 |
| [T028](https://github.com/anilbeesetti/ktlsp/issues/28) | Native Kotlin semantic diagnostic codes and ranges | T016,T019 |
| [T029](https://github.com/anilbeesetti/ktlsp/issues/29) | Library-aware coroutine/serialization and AllOpen support | [T028](https://github.com/anilbeesetti/ktlsp/issues/28) |
| [T030](https://github.com/anilbeesetti/ktlsp/issues/30) | Quick fixes (master-only suppression follow-up) | [T028](https://github.com/anilbeesetti/ktlsp/issues/28) |
| [T031](https://github.com/anilbeesetti/ktlsp/issues/31) | Resolved semantic token provider | T027,T028 |
| [T032](https://github.com/anilbeesetti/ktlsp/issues/32) | Typed inlay hints and hint resolution | T013,T014 |
| [T033](https://github.com/anilbeesetti/ktlsp/issues/33) | Signature help overloads and active arguments | T014,T025 |
| [T034](https://github.com/anilbeesetti/ktlsp/issues/34) | Safe symbol rename and prepareRename | [T027](https://github.com/anilbeesetti/ktlsp/issues/27) |
| [T035](https://github.com/anilbeesetti/ktlsp/issues/35) | Move file/package refactoring | T034,T021 |
| [T036](https://github.com/anilbeesetti/ktlsp/issues/36) | Extract and inline local variable refactoring | T015,T030 |
| [T037](https://github.com/anilbeesetti/ktlsp/issues/37) | Baseline Kotlin intention catalog and applicability | T016,T030 |
| [T038](https://github.com/anilbeesetti/ktlsp/issues/38) | Native formatter with syntax preservation | [T007](https://github.com/anilbeesetti/ktlsp/issues/7) |
| [T039](https://github.com/anilbeesetti/ktlsp/issues/39) | Semantic organize imports and autoimport actions | T010,T019,T030 |
| [T040](https://github.com/anilbeesetti/ktlsp/issues/40) | Resolved call hierarchy incoming/outgoing | T014,T027 |
| [T041](https://github.com/anilbeesetti/ktlsp/issues/41) | Pinned official custom LSP and command compatibility | T021,T030,T035 |
| [T042](https://github.com/anilbeesetti/ktlsp/issues/42) | Specify Rust-only run/debug boundary and protocol | T019,T023,T041 |
| [T043](https://github.com/anilbeesetti/ktlsp/issues/43) | Incremental snapshots query invalidation and cancellation | T011,T028 |
| [T044](https://github.com/anilbeesetti/ktlsp/issues/44) | Latency memory and malformed-input performance budgets | T007,T043 |
| [T045](https://github.com/anilbeesetti/ktlsp/issues/45) | Isolated official oracle and differential protocol corpus | [T005](https://github.com/anilbeesetti/ktlsp/issues/5) |
| [T046](https://github.com/anilbeesetti/ktlsp/issues/46) | Native cross-platform packaging and parity release audit | All required baseline tickets T001–T045, T047, T048, T049; every parity row |
| [T047](https://github.com/anilbeesetti/ktlsp/issues/47) | Kotlin main Run/Debug code lenses and initialization gating | T014,T019,T041 |
| [T048](https://github.com/anilbeesetti/ktlsp/issues/48) | Registered-source JPS project model compatibility | T004,T021 |
| [T049](https://github.com/anilbeesetti/ktlsp/issues/51) | Runtime-advertised type hierarchy | T012,T019,T027 |

T041 acceptance explicitly includes decompile/JAR+jrt virtual docs, file templates, exportWorkspace, apply/resolveModCommand, highwatermark commands and client-extension initialization gates; wire names/payloads require pinned evidence. T047 includes `runMainCodeLens` gating and descriptors only, with JVM execution pending separately.

These tickets are integration-sized contracts. Semantic/intentions/import tickets must be expanded into smaller fixture-specific child issues before their implementation starts. T001–T005 are delivered at historical checkpoint `0d54973`; T006 is delivered in PR73. Children #58 (classfile structure), #59 (recorded replay), #60 (delivery hygiene), #61 (offline artifacts), #62 (parser safety) and #63 (exported-model subset), #69 (declaration annotations), #86 (revisioned parser results/corpus), #87 (parameter recovery), #66 (Signatures), #67 (nesting), #100 (bounded generic sibling recovery), #97 (Metadata header/d1) and #99 (prior docs checkpoint/PR101) are delivered, while parents #7/#17/#18/#20/#21/#44/#45/#46 remain open. [The development ledger](status/development.md) records exact checkpoint evidence and pending children, including documentation child #105 and pending C12/#103/C17/#104. T007 has a safety/adoption gate; delivered #62/#86 do not establish complete syntax or admit T008a while the final #83 audit retains C12–C17/B3–B5 gaps and B11 UNVERIFIED (T008/T008a HOLD). T042 and unrestricted dynamic build import have external-tool implementation/verification gaps under the clarified runtime boundary. Narrow child delivery leaves its parent open until the full parent acceptance is met.

Current policy: user-existing JDK/build tools are permitted for optional import/build/run/debug. T004 stays data-only; T022/T023/T042 must include small native external-tool interoperability subtickets. No Java is bundled and no semantic engine is offloaded. See specs/runtime-boundary.md.

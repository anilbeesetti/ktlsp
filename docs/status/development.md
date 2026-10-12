# Development ownership and evidence ledger

Snapshot: 2026-10-12, starting checkpoint `0d54973ce66f59362829360261fde6e46b4fc420`. Delivery hygiene child [T046a/#60](https://github.com/anilbeesetti/ktlsp/issues/60) owns this ledger. [Parity status](parity.md) records delivered behavior; this ledger records assignments and evidence needed to update it. T001–T005 are delivered; every semantic provider remains pending. Completed [research](../research/official.md), [oracle provenance](../research/oracle/README.md) and specifications remain the baseline.

## Admission and shared ownership

The coordinator reports 101 collaboration slots. That is reported capacity, not demonstrated simultaneous project-chat or build throughput. The initial six implementation chats plus this steward were observed active in isolated app worktrees at `0d54973`, configured gpt-6.1-sol High. The coordinator subsequently admitted T044 baseline preparation: eight project lanes including delivery, with two independent review slots reserved per owner (16 total). Actual memory/CPU/build contention and parallel-chat behavior still require observation; reduce admission when resources demand it. Architect reviews plans, repeated errors and completed long work only.

Coordinator chat: `01a1300d-2ba0-7832-97d0-91d070e2d297`. It alone grants exclusive merge windows. Each owner works on a `codex/` branch in its own worktree; no edits in another checkout. Root Cargo.toml/external dependencies require coordination. Cargo.lock integration is serialized at the merge gate and must preserve other packages. Shared exports are additive, integrated and rechecked. The steward exclusively owns PLAN.md, TICKETS.md, HANDOFF.md, status/parity.md, this ledger, specs/architecture.md and specs/verification.md. Ticket owners report evidence instead of editing those files.

Native product/build/core analysis/default tests remain Rust-only; T004 stays data-only. Optional user-existing external JDK/build/run/debug tools are permitted under [runtime-boundary.md](../specs/runtime-boundary.md); no bundled Java or JVM semantic offload. This wave runs no Gradle/Maven or fresh JVM oracle. Recorded-oracle replay is native data processing and is distinct from isolated official execution.

## Admitted contracts

All entries below are assigned work, not delivered functionality. A parent remains open when a narrow child lands; only full canonical acceptance permits parent closure. Owned paths are scope reservations; owner-confirmed changes and dependencies are recorded as reports arrive.

| Lane / accepted issue | Owner chat | Owned paths | Prerequisites | Declared subset and residual limits |
| --- | --- | --- | --- | --- |
| [T006/#6](https://github.com/anilbeesetti/ktlsp/issues/6) | `01a13010-c295-7ae2-8794-67408851b739` | `crates/ktlsp-server/**`; syntax path dependency reference | T003,T005 delivered | Token-aware folding and lexical/delimiter diagnostics over stdio; no document symbols or semantic/compiler diagnostics. lineFoldingOnly may intentionally differ from captured official fields. |
| [T007 safety/#62](https://github.com/anilbeesetti/ktlsp/issues/62), parent #7 | `01a13010-cac9-7672-bcee-1ea17b923747` | `crates/ktlsp-parser/**`; own approved lock changes | T003 delivered; dependency/notice audit | Enforce bounded parsing and lossless validation with kill/reap worker isolation. Coordinator approved exact ktrs-parser/ktrs-syntax =0.5.1 and expected ktrs-lexer 0.5.1/text-size 1.1.1 subject to graph/checksum/notice audit; no product server wiring or complete syntax claim. |
| [T017a/#58](https://github.com/anilbeesetti/ktlsp/issues/58), parent #17 | `01a13010-d29c-71a3-b325-053c9b3729f1` | `crates/ktlsp-binary/**` | No semantic prerequisite | Bounded classfile envelope/constant-pool/member reader; generic signatures, Kotlin metadata and library semantics remain separate. |
| [T020a/#61](https://github.com/anilbeesetti/ktlsp/issues/61), parent #20 | `01a13010-d9e6-7f02-9054-c3afb6ebea18` | `crates/ktlsp-artifacts/**`; own std-only lock entry | T004 delivered | Bounded exact-coordinate offline Maven-layout byte lookup on trusted quiescent roots; checksum unverified. No integration/downloads/cache writes/lockfiles/transitives/dynamic snapshots/Gradle layout/archive verification. |
| [T021a/#63](https://github.com/anilbeesetti/ktlsp/issues/63), parent #21 | `01a13010-e164-7cc0-babd-fac11c564256` | `crates/ktlsp-import/**`; own lock entry | T004 delivered; existing serde/serde_json/project references | Pinned unversioned workspace fixture subset, typed content/dependency-order sidecar and validated v1 projection. Unsupported SDK/settings/facets/resources/exclusions/scoped-exported libraries/build metadata block atomically; no flattening or server wiring. |
| [T045a/#59](https://github.com/anilbeesetti/ktlsp/issues/59), parent #45 | `01a13010-e929-7911-adbf-fb7c5a308c08` | `tools/oracle/**`, `docs/research/oracle/comparisons/**`; standalone lockfile | T005 delivered; committed official transcripts/provenance | Replay named native-supported observations; no fresh JVM execution or untested semantic-provider verification. |
| [T044 preparation/#64](https://github.com/anilbeesetti/ktlsp/issues/64), parent #44 | `01a13013-ec25-7c30-8b30-97875167f403` | `tools/bench/**`, standalone workspace/lockfile with existing native path references | Baseline `0d54973`; quiet measurement window; T007/T043 still unmet for parent | Native latency/memory baseline only, no comparative faster/lower-memory claim or parent closure. |
| [T046a/#60](https://github.com/anilbeesetti/ktlsp/issues/60), parent #46 | `01a13011-7393-7130-a497-479d1382bfe0` | Shared documentation listed above | Checkpoint and canonical issues | Coordination/evidence hygiene only; no source/Cargo edits or parity release. |

## Delivery evidence gate

For every lane, record accepted issue, exact reviewed head, separate functionality and adversarial review findings/rechecks, pinned command results/counts, hosted check URLs, integration base, coordinator grant, merge SHA and remaining limits. Until received, each item is pending; active chat status or a local green check is not delivery evidence. Native-tested, partial and oracle-verified apply only to named contracts/fixtures, as defined in [verification](../specs/verification.md).

| Lane | Reviewed head / reviews | Pinned native checks / hosted CI | Merge SHA | Native / official status |
| --- | --- | --- | --- | --- |
| T006 | pending | pending | pending | pending / unverified |
| T007 safety child | pending | pending | pending | pending / unverified |
| T017a | pending | pending | pending | pending / unverified |
| T020a | pending | pending | pending | pending / unverified |
| T021a | pending | pending | pending | pending / unverified |
| T045a | pending | pending | pending | pending / recorded comparison pending |
| T044 baseline child | pending | pending; quiet window required | pending | pending / no comparative evidence |
| T046a | pending | pending | pending | documentation only / no provider comparison |

All owners run pinned format, locked clippy and locked workspace tests, plus relevant bounded crash/subprocess checks. Recheck changed integration behavior, wait for hosted CI on the reviewed head, obtain coordinator merge admission, then merge with expected SHA. The coordinator verifies integrated main; the steward reconciles evidence in a later documentation update. A commit cannot contain its own final hash/check URLs, so the initial ledger deliberately leaves its own delivery evidence pending until a later reviewed update.

## Proposed next child queue (not admitted)

1. T007 follow-up complete syntax/recovery corpus, then T008 fixture-specific declarations/CST-HIR: start only after enforceable parser safety and source preservation land; safety alone is insufficient. T009 source scopes/identity follows accepted declarations, then T010 imports/T011 module-aware indexing. No blocked semantic parent starts prematurely.
2. T017 follow-up descriptors/generic signatures/attributes, then T018 metadata version/bit-encoding/protobuf/name resolver fixtures: require validated bounded classfile access and notices before binary semantic indexing (T019 additionally needs source/module/type prerequisites).
3. T021 model-extension child for unrepresentable root/scope/settings/SDK semantics; extend representable validated data before broadening import fixtures. Pair with T020 follow-up bounded cache/checksum/source-artifact contracts; no guessed resolution. T022/T023 optional existing-tool interoperability needs separate accepted native orchestration contracts and resolved-model provenance.
4. T045 follow-up provider comparisons once T006 and replay merge, preserving meaningful ranges/diagnostics and reporting intentional differences; separately coordinate any fresh isolated official capture. Recorded replay never certifies untested providers.
5. T044 budgets/incremental workloads after T007/T043 prerequisites and native baseline evidence. Comparative workloads must have equivalent supported behavior and documented hardware/profile/cache/method before speed or memory claims.

The coordinator dispatches accepted children and merge windows. These proposals do not reopen completed research or grant implementation/merge admission.

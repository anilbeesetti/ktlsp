# Verification and review contract

## Evidence levels

`pending`: no delivered implementation; `partial`: a defined subset is native-tested; `native-tested`: full declared subset passes local/integration tests; `oracle-verified`: matches the pinned official release corpus for the named fixtures; `blocked`: cannot yet meet the required contract. Record native status and official comparison separately: replaying recorded observations does not establish fresh official execution or untested provider behavior. These are separate from ticket closure. Closing a lexer ticket does not close language-parity requirements.

Each PR records issue, exact commit, code/behavior/edge cases reviewed, tests executed, limitations and reviewer findings/fixes. Reviewers must inspect code independently and exercise adversarial inputs; an implementer's statement that tests passed is insufficient evidence.

## Required regressions by subsystem

Text: Unicode astral characters, combining marks, CRLF, final empty line, invalid line/column/UTF-8 boundaries, surrogate splits, empty/full/sequential edits, stale and duplicate versions, rollback on later invalid edit, rangeLength validation.

Transport: fragmented reads, back-to-back frames, multibyte byte length, extra headers, case-insensitive Content-Length, duplicate/missing/non-numeric/overflow lengths, oversized headers/body, truncated EOF, closed pipes; all failures controlled and no protocol contamination on stdout.

Lexer/parser: nested/unclosed comments, escaped and raw strings, string templates, backticks, Unicode names, malformed numbers/characters, arbitrary UTF-8, deep delimiters and repeated type-argument ambiguity. Validate forward progress and exact token-span partition; run deterministic generated-input crash cases. Do not report unsupported syntax as compiler errors.

Explicit project-model loader (T004, data-only): malformed/oversized JSON, missing/duplicate identifiers, unknown schema versions, dependency cycles and missing references, relative paths, source-root overlap, generated/test roots, missing artifacts, filesystem permissions and symlinks. No build-tool execution.

Semantic providers: shadowing, duplicate names, aliases/star/default imports, module visibility, overloads, extension/member precedence, generics/variance/nullability, smart casts, incomplete code, cross-file edits and stale versions. Unresolved/ambiguous symbols must not drive rename/refactoring.

## Differential corpus

Fixtures use one directory per Kotlin/project case plus a sequence of JSON-RPC messages, expected normalized responses and a metadata file specifying baseline release, binary checksum, capabilities/settings/project model, observation date and normalization policy. Normalize only nondeterministic IDs/URIs/ordering fields proven unordered; never normalize semantic errors, diagnostic severities, text edits or completion ranking away. Native-only fixtures must say that no official oracle was run.

PR70 provides an [optional recorded-observation replay tool](../../tools/oracle/README.md), outside the default native test/CI/product graph. Its six tool tests are counted separately from workspace tests. The parser worker_protocol assert-based harness runs with workspace tests; its 19 fault cases are recorded separately from the ordinary Rust test count, and two ignored helper fixtures are invoked by active subprocess parents. The replay tool applies no semantic normalization, accepts sequential recorded interactions and cleans up only the direct child; process-tree supervision and a full corpus remain pending. Published baseline evidence is pinned to historical native revision `0d54973`, not current server behavior. PR73 separately compares/replays the actual committed folding observation: fold lines/kind and shutdown/exit match, but character/collapsedText fields for line-only clients and capability/notification differences remain. No fresh JVM ran in this wave; matching those rows does not certify complete folding or lifecycle parity.

Checkpoint counts belong to their exact reviewed integrations, not an aggregate of historical runs. Local macOS arm64 and hosted Linux execution verify the declared native subsets; Windows execution and cross-platform release packaging remain pending. The [development ledger](../status/development.md) records per-delivery heads, merge SHAs, check URLs and limits. Documentation updates must compare status claims against actual capabilities and recorded evidence, check local links, and search for stale claims across all owned status files.

## CI and merge

Run `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, and `cargo test --locked --workspace` using pinned Rust 1.99.0 (declared MSRV 1.90). Add relevant subprocess stdio integration/crash checks. Rebase or merge current main into the ticket branch after dependencies land, and review/test any integration changes before merge. Require the coordinator's exclusive merge-window grant and merge with the expected reviewed head SHA; serialize shared Cargo.lock integration. No force-push of main, no bypassing repository protection, no merging after unresolved P0/P1/P2 findings. Required hosted checks must pass on that head; if hosted CI is unavailable, disclose it and do not describe local checks as hosted CI. Record exact commands, test counts, check URLs, reviewed head, merge SHA and residual limits in delivery evidence.

## Performance evidence

Native baseline measurements are not comparative parity evidence. Publish corpus, supported behavior, hardware/OS, toolchain/profile, process/cache state, measurement method and reproducible results. Claim faster execution or lower memory only after measuring equivalent supported workloads against the pinned official release under a documented comparable method. Coordinate a quiet measurement window rather than timing under concurrent builds. T044 remains pending until its full budgets and incremental workload requirements are verified.

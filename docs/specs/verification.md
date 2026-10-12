# Verification and review contract

## Evidence levels

`pending`: no implementation; `partial`: a defined subset is native-tested; `native-tested`: full declared subset passes local/integration tests; `oracle-verified`: matches the pinned official release corpus; `blocked`: cannot yet meet the required contract. These are separate from ticket closure. Closing a lexer ticket does not close language-parity requirements.

Each PR records issue, exact commit, code/behavior/edge cases reviewed, tests executed, limitations and reviewer findings/fixes. Reviewers must inspect code independently and exercise adversarial inputs; an implementer's statement that tests passed is insufficient evidence.

## Required regressions by subsystem

Text: Unicode astral characters, combining marks, CRLF, final empty line, invalid line/column/UTF-8 boundaries, surrogate splits, empty/full/sequential edits, stale and duplicate versions, rollback on later invalid edit, rangeLength validation.

Transport: fragmented reads, back-to-back frames, multibyte byte length, extra headers, case-insensitive Content-Length, duplicate/missing/non-numeric/overflow lengths, oversized headers/body, truncated EOF, closed pipes; all failures controlled and no protocol contamination on stdout.

Lexer/parser: nested/unclosed comments, escaped and raw strings, string templates, backticks, Unicode names, malformed numbers/characters, arbitrary UTF-8, deep delimiters and repeated type-argument ambiguity. Validate forward progress and exact token-span partition; run deterministic generated-input crash cases. Do not report unsupported syntax as compiler errors.

Project: malformed/oversized JSON, missing/duplicate identifiers, unknown schema versions, dependency cycles and missing references, relative paths, source-root overlap, generated/test roots, missing artifacts, filesystem permissions and symlinks. No build-tool execution.

Semantic providers: shadowing, duplicate names, aliases/star/default imports, module visibility, overloads, extension/member precedence, generics/variance/nullability, smart casts, incomplete code, cross-file edits and stale versions. Unresolved/ambiguous symbols must not drive rename/refactoring.

## Differential corpus

Fixtures use one directory per Kotlin/project case plus a sequence of JSON-RPC messages, expected normalized responses and a metadata file specifying baseline release, binary checksum, capabilities/settings/project model, observation date and normalization policy. Normalize only nondeterministic IDs/URIs/ordering fields proven unordered; never normalize semantic errors, diagnostic severities, text edits or completion ranking away. Native-only fixtures must say that no official oracle was run.

## CI and merge

Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` using the pinned toolchain. Add subprocess stdio integration/crash tests when the binary exists. Rebase or merge current main into the ticket branch after dependencies land, and review/test any integration changes before merge. No force-push of main, no bypassing repository protection, no merging after unresolved P0/P1/P2 findings. Required hosted checks must pass; if hosted CI is unavailable, disclose it and do not describe local checks as hosted CI.

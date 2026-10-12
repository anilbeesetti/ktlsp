# Native server syntax providers (T006)

`ktlsp` supports bounded stdio lifecycle, open-document synchronization and `textDocument/foldingRange`. Initialization advertises UTF-16, incremental sync and folding only. `textDocument/publishDiagnostics` reports **lexical/structural** checks, not Kotlin compiler diagnostics; no pull diagnostic provider or document-symbol provider is advertised.

Folding recognizes matched multiline `{}`, `()`, `[]`, block-comment tokens and raw-string tokens. Strings/templates, characters, backtick identifiers and comments remain opaque to delimiter matching. The complete opaque raw-string/comment token may fold; nested template expressions/comment interiors do not create outer-file folds. Single-line ranges are omitted. Unterminated multiline comments/raw strings can fold to their token end while their lexical diagnostic remains visible. Imports, line-comment groups, declarations and semantic structure are outside this declared subset.

On a mismatched closer, report it and discard/diagnose all pending openers. This conservative recovery cannot create crossing delimiter folds; complete valid pairs encountered later can still fold. Unmatched closers and remaining openers produce structural diagnostics. Accepted open/change notifications publish replacement diagnostics (including empty arrays); close clears them. Rejected/stale/invalid edits publish nothing and preserve the current document. Include document versions only when `publishDiagnostics.versionSupport` is true; close clearing omits version.

Missing/closed-document folding requests return invalid params (`-32602`), without reading disk. Malformed request params/client folding capabilities also return invalid params. Analysis exhaustion leaves an accepted document version intact, replaces stale diagnostics with one explicit `analysis-limit` warning, and fails folding with RequestFailed (`-32803`). Subsequent edits can recover normally.

## Bounds and complexity

Existing lexer limits apply: 8 MiB document, 1,000,000 tokens, 256 lexical diagnostics, 128 nested scanner contexts. Structural nesting is capped at 1,024 open delimiters. At most 256 lexical/structural diagnostics are published; further structural diagnostics replace the last entry with an explicit suppression marker. Lexer exhaustion instead yields the analysis-limit warning.

Retain at most `min(client rangeLimit, 10,000)` multiline fold candidates, including zero. Selection is the first candidates completed during source-order scanning; results are then sorted by start line/character/end line. Nested inner folds can exhaust a small client budget before outer folds. `lineFoldingOnly: true` omits both character fields. No collapsed-text support is claimed.

Each query scans the bounded lexer token partition with an iterative stack. Recovery consumes each stack entry once. Only retained output endpoints (at most 20,512) are sorted/deduplicated and converted in one UTF-16 sweep; no per-token scan from the document or line start. Source/token work is linear; bounded output sorting/index lookup is O(k log k). There is no cached analysis per open document and no new external dependency. Synchronous analysis/cancellation and complete syntax/semantic parity remain later work.

## Official observation comparison

The native test reads the committed `docs/research/oracle/lifecycle-folding/Main.kt` and its actual captured response for pinned `kotlin-lsp/v263.6379.0`. For `fun main() { ... }`, native character-aware output matches captured start/end lines 0/2, UTF-16 characters 11/1 and `kind: region`. Official also returned `collapsedText: "{...}"` and character fields despite captured `lineFoldingOnly: true`; native intentionally omits these fields for that client profile. Tests assert the exact official response and explicit inequality for the line-only native response; they do not normalize away those differences. This one recorded comparison is not full folding parity or a fresh official replay. T045 owns further isolated replay/corpus tooling.

Native tests also cover real fragmented stdio with Java variables removed and empty PATH, CRLF/lone CR/non-BMP ranges, lexical errors, malformed nesting, stale/change/close behavior, scanner/token/depth/output limits, many folds, one long Unicode line and deterministic generated arbitrary UTF-8.

## Local performance observation

2026-10-12, macOS arm64 Apple M4 / 16 GiB, pinned Rust 1.99.0 release build. `/usr/bin/time -l target/release/deps/providers-7d4529ca788e2bde --exact many_folds_and_long_unicode_line_have_bounded_outputs --nocapture` measured 0.11 s wall time, 63,520,768 bytes maximum RSS and 62,390,752 bytes peak footprint for the complete test process (including generated source, retained JSON results and test harness). Within that run, 50,000 sequential multiline pairs (200,000 source bytes, result capped at 10,000) took 30.36 ms for open plus folding; a roughly 1.2 MB single Unicode line with 200,000 astral string characters and 200,000 single-line pairs, then one multiline pair, took 82.19 ms for change plus folding. An earlier run reported 45.93/84.13 ms. These are observations on one host, not budgets, isolated per-query memory, stable benchmarks or comparisons with the official server. Real native stdio also passes 30,000-fold/limit/recovery coverage.

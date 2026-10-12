# Proposed integration — awaiting coordinator grant

Dependency #86 must merge before production integration. Request exclusive handoff
for `crates/ktlsp-parser/Cargo.toml`, its affected declaration/resource tests and
ticket-local docs, plus Cargo.lock. No root manifest or syntax crate change needed.

Proposed direct dependency:
`ktrs-parser = { version = "=0.5.1", path = "../../vendor/ktrs-parser-0.5.1" }`

Expected lock delta: remove registry source/checksum from ktrs-parser 0.5.1 only;
retain existing ktrs-lexer/syntax 0.5.1 and text-size 1.1.1. No new package or
external dependency; no patch to other crates. Existing supervised worker stays
the sole product invocation. The vendor's standalone workspace/lock is only for
isolated source-local tests; actual production tests use root locked workspace.

Replace #86's three parameter-list defect assertions with exact restoration
expectations, preserve its separate unclosed generic-list defect, and add
production original-source LF/CRLF/lone CR/Unicode/backtick/multiple-recovery
controls, diagnostics/boundaries and resource/healthy-followup cases. Recheck
both reviewers after integration; run pinned fmt/clippy/workspace tests and exact
head hosted CI before requesting the coordinator's merge window. #83/#7/T008
remain gated on coordinator's joint recovery assessment, not this repair alone.

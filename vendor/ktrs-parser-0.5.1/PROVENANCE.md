# T007b2 source-local parser repair

Child #87 of #83/#7; production development follows merged #86 and explicit coordinator handoff.

Sources copied from the crates.io `ktrs-parser` 0.5.1 archive, SHA256
`ec8d43d9549490689477385c7678bbd22ecbc445973ab364e8ecb69dd4882bcc`.
`upstream-sha256.json` inventories original Rust source bytes. Upstream repository:
https://github.com/Hexay/ktrs (research pin a703f88d6d4ac9d983a17df66ddf6ac403546c00).
The registry archive is authoritative for this fork. License/NOTICE files retained
from the previously audited project notices; the published archive omitted them.

Only Rust `src/` is copied, including upstream unit-test source, without upstream
examples/integration fixtures/workspace or dev dependencies. Minimal standalone
manifest pins existing `ktrs-lexer`/`ktrs-syntax` 0.5.1; their sole transitive normal
dependency is `text-size` 1.1.1. No external dependency addition. Upstream lib unit
tests depend on expect-test and are not run by this standalone harness; no
mimalloc or facade/native parser dependencies are added. Ticket tests are original.

The product parser selects this fork as a path dependency. The root workspace
excludes the vendor workspace, keeping upstream dev dependencies out of default
tests. Production regressions live in crates/ktlsp-parser and traverse the actual
supervised worker through parse_snapshot/tree_for. Standalone tests alone do not
establish delivery; integrated review/checks/CI remain required. Direct upstream
parse_file requires LF; CRLF/lone CR production tests exercise the existing
normalized-to-original mapping through the supervised worker. Original source is never
modified by this repair. Full Kotlin grammar and semantic parity remain pending.

The patch checks ordinary function parameter boundaries, probes existing modifier
and annotation grammar with rollback, and reports a missing ')' without consuming
a following declaration. Nested type/default parsing remains with existing grammar.
Constructor, function-type and context parameter loops keep their original context.
EOF diagnostics retain upstream behavior. A scoped speculative-probe flag avoids
recursive replay when annotation arguments contain anonymous functions. Remaining
work is superlinear in nesting; existing production resource gates remain required.

Known bounded repair limits: a missing default (`x: Int =` followed by named fun),
deeply malformed parenthesized types (`x: (@A`), malformed annotation arguments,
and unclosed class generic lists can still consume later declarations. These are
upstream grammar/recovery gaps, not accepted recovery or full grammar parity.
Missing simple types and open generic argument types improve recovery but retain
upstream diagnostic deduplication (e.g. only `Type expected` at one offset).

Run the bounded source-local checks with:
`cargo test --offline --locked --manifest-path vendor/ktrs-parser-0.5.1/Cargo.toml --test recovery`

`worker-probe/` preserves the isolated production-source experiment without code
duplication: its manifest references existing production worker/library/tests and
patches ktrs-parser only in its standalone graph. Run:
`cargo test --offline --locked --manifest-path vendor/ktrs-parser-0.5.1/worker-probe/Cargo.toml --tests`
Neither standalone workspace is run by root workspace tests. Equivalent production
regressions now live in the parser crate and must pass integrated review/checks/CI
before #87 can close.
